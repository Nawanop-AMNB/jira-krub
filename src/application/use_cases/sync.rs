use crate::application::ports::{EMBED_LIMIT, IssueWithWorklogs, JiraGateway, Window};
use crate::domain::{Issue, IssueKey, RemoteWorklog};
use anyhow::Result;
use std::collections::BTreeSet;
use std::sync::{Condvar, Mutex};
use std::thread;

/// At most this many gateway requests run at once during the concurrent
/// legs of a sync.
const MAX_IN_FLIGHT: usize = 4;

/// Result of fetching one watchlist key: `None` when the issue is missing
/// (skipped, not an error).
type WatchlistFetch = Result<Option<(Issue, Vec<RemoteWorklog>)>>;

#[derive(Debug, Clone)]
pub struct SyncInput {
    pub jql: String,
    pub watchlist: Vec<IssueKey>,
    /// Days the UI wants worklogs for.
    pub window: Window,
}

#[derive(Debug, Clone)]
pub struct SyncOutput {
    pub account_id: String,
    pub display_name: String,
    /// Assigned, un-done (config JQL).
    pub mine: Vec<Issue>,
    /// Watchlist issues, freshly fetched (missing ones are skipped).
    pub watched: Vec<Issue>,
    /// Issues with my worklogs in the window that are neither mine nor watched.
    pub history: Vec<Issue>,
    /// My worklogs inside `window` across all three lists.
    pub worklogs: Vec<RemoteWorklog>,
    pub window: Window,
}

/// A partial result reported while a streamed sync is still running.
#[derive(Debug, Clone)]
pub enum SyncProgress {
    /// The assigned-to-me list is ready. Reported before any watchlist or
    /// history request is made, so the UI can show it immediately.
    Mine { account_id: String, display_name: String, mine: Vec<Issue> },
}

/// Cheapest complete fetch:
/// - mine + history come from JQL searches with the `worklog` field embedded
///   (one request each). An issue whose worklog count exceeds what Jira
///   embeds is re-fetched per issue, server-filtered to the window.
/// - watchlist issues are always fetched per issue with the window filter:
///   they are shared tickets, often old, with many other people's entries.
///
/// A plain, non-streaming entry point kept for tests that don't care about
/// progress; production code streams through [`run_with`] instead.
#[cfg(test)]
pub fn run(gateway: &dyn JiraGateway, input: &SyncInput) -> Result<SyncOutput> {
    run_with(gateway, input, &mut |_| {})
}

/// Same fetch as [`run`], plus a `progress` callback invoked with
/// [`SyncProgress::Mine`] as soon as the assigned-to-me list is known —
/// before any watchlist or history request is made. The watchlist and
/// history requests then run concurrently (capped at `MAX_IN_FLIGHT`
/// in-flight requests), and so does the per-issue worklog refetch for the
/// mine and history lists.
pub fn run_with(gateway: &dyn JiraGateway, input: &SyncInput, progress: &mut dyn FnMut(SyncProgress)) -> Result<SyncOutput> {
    let me = gateway.myself()?;
    let w = &input.window;

    let mine_raw = gateway.search_issues_with_worklogs(&input.jql, 200, &me.account_id)?;
    progress(SyncProgress::Mine {
        account_id: me.account_id.clone(),
        display_name: me.display_name.clone(),
        mine: mine_raw.iter().map(|i| i.issue.clone()).collect(),
    });

    let history_jql = format!(
        "worklogAuthor = currentUser() AND worklogDate >= {} AND worklogDate <= {} ORDER BY updated DESC",
        w.from.format("%Y-%m-%d"),
        w.to.format("%Y-%m-%d")
    );

    let sem = Semaphore::new(MAX_IN_FLIGHT);

    // Watchlist keys and the history search are independent of each other
    // and of the mine list already in hand, so they run concurrently.
    let account_id = me.account_id.as_str();
    let (watchlist_results, history_result): (Vec<WatchlistFetch>, Result<Vec<IssueWithWorklogs>>) = thread::scope(|scope| {
        let watchlist_handles: Vec<_> = input
            .watchlist
            .iter()
            .map(|key| {
                let sem = &sem;
                scope.spawn(move || {
                    sem.acquire();
                    let result = fetch_watchlist_issue(gateway, key, account_id, w);
                    sem.release();
                    result
                })
            })
            .collect();
        let history_handle = {
            let sem = &sem;
            let history_jql = history_jql.as_str();
            scope.spawn(move || {
                sem.acquire();
                let result = gateway.search_issues_with_worklogs(history_jql, 200, account_id);
                sem.release();
                result
            })
        };
        let watchlist_results = watchlist_handles.into_iter().map(join_scoped).collect();
        let history_result = join_scoped(history_handle);
        (watchlist_results, history_result)
    });

    let mut watched = Vec::new();
    let mut watched_worklogs = Vec::new();
    for result in watchlist_results {
        if let Some((issue, worklogs)) = result? {
            watched_worklogs.extend(worklogs);
            watched.push(issue);
        }
    }

    let known: BTreeSet<IssueKey> = mine_raw.iter().map(|i| i.issue.key.clone()).chain(watched.iter().map(|i| i.key.clone())).collect();
    let history_raw: Vec<IssueWithWorklogs> = history_result?.into_iter().filter(|i| !known.contains(&i.issue.key)).collect();

    // Per-issue embedded-or-refetch for mine and history, also concurrent.
    let mine_len = mine_raw.len();
    let refetched: Vec<Result<Vec<RemoteWorklog>>> = {
        let items: Vec<&IssueWithWorklogs> = mine_raw.iter().chain(history_raw.iter()).collect();
        thread::scope(|scope| {
            let account_id = me.account_id.as_str();
            let handles: Vec<_> = items
                .into_iter()
                .map(|i| {
                    let sem = &sem;
                    scope.spawn(move || {
                        sem.acquire();
                        let result = embedded_or_refetch(gateway, account_id, w, i);
                        sem.release();
                        result
                    })
                })
                .collect();
            handles.into_iter().map(join_scoped).collect()
        })
    };
    let mut mine_refetched = refetched;
    let history_refetched = mine_refetched.split_off(mine_len);

    let mut mine = Vec::with_capacity(mine_raw.len());
    let mut mine_worklogs = Vec::new();
    for (i, result) in mine_raw.into_iter().zip(mine_refetched) {
        mine_worklogs.extend(result?);
        mine.push(i.issue);
    }
    let mut history = Vec::with_capacity(history_raw.len());
    let mut history_worklogs = Vec::new();
    for (i, result) in history_raw.into_iter().zip(history_refetched) {
        history_worklogs.extend(result?);
        history.push(i.issue);
    }

    let mut worklogs = mine_worklogs;
    worklogs.extend(watched_worklogs);
    worklogs.extend(history_worklogs);

    Ok(SyncOutput { account_id: me.account_id, display_name: me.display_name, mine, watched, history, worklogs, window: *w })
}

/// One watchlist key: fetch the issue, then (only if it exists) its
/// worklogs. A missing issue is skipped, not an error; a worklog fetch
/// failure is.
fn fetch_watchlist_issue(gateway: &dyn JiraGateway, key: &IssueKey, account_id: &str, w: &Window) -> WatchlistFetch {
    match gateway.get_issue(key) {
        Ok(issue) => {
            let worklogs = gateway.my_worklogs(key, account_id, Some(w))?;
            Ok(Some((issue, worklogs)))
        }
        Err(_) => Ok(None),
    }
}

/// Use the embedded page when it is the whole list; otherwise ask the server
/// for just the window.
fn embedded_or_refetch(gateway: &dyn JiraGateway, account_id: &str, w: &Window, i: &IssueWithWorklogs) -> Result<Vec<RemoteWorklog>> {
    if i.total > EMBED_LIMIT {
        return gateway.my_worklogs(&i.issue.key, account_id, Some(w));
    }
    Ok(i.my_worklogs.iter().filter(|x| w.contains(x.local_date())).cloned().collect())
}

/// Joins a scoped thread, propagating a panic rather than swallowing it.
fn join_scoped<T>(handle: thread::ScopedJoinHandle<'_, T>) -> T {
    match handle.join() {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// A simple counting semaphore bounding how many concurrent legs run at
/// once. std-only, no new dependency.
struct Semaphore {
    available: Mutex<usize>,
    cond: Condvar,
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        Self { available: Mutex::new(permits), cond: Condvar::new() }
    }

    fn acquire(&self) {
        let mut available = self.available.lock().unwrap_or_else(|e| e.into_inner());
        while *available == 0 {
            available = self.cond.wait(available).unwrap_or_else(|e| e.into_inner());
        }
        *available -= 1;
    }

    fn release(&self) {
        let mut available = self.available.lock().unwrap_or_else(|e| e.into_inner());
        *available += 1;
        drop(available);
        self.cond.notify_one();
    }
}
