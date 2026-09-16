use crate::application::ports::{EMBED_LIMIT, IssueWithWorklogs, JiraGateway, Window};
use crate::domain::{Issue, IssueKey, RemoteWorklog};
use anyhow::Result;
use std::collections::BTreeSet;

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

/// Cheapest complete fetch:
/// - mine + history come from JQL searches with the `worklog` field embedded
///   (one request each). An issue whose worklog count exceeds what Jira
///   embeds is re-fetched per issue, server-filtered to the window.
/// - watchlist issues are always fetched per issue with the window filter:
///   they are shared tickets, often old, with many other people's entries.
pub fn run(gateway: &dyn JiraGateway, input: &SyncInput) -> Result<SyncOutput> {
    let me = gateway.myself()?;
    let w = &input.window;

    let mine_raw = gateway.search_issues_with_worklogs(&input.jql, 200, &me.account_id)?;

    let mut watched = Vec::new();
    let mut worklogs = Vec::new();
    for key in &input.watchlist {
        if let Ok(issue) = gateway.get_issue(key) {
            worklogs.extend(gateway.my_worklogs(key, &me.account_id, Some(w))?);
            watched.push(issue);
        }
    }

    let history_jql = format!(
        "worklogAuthor = currentUser() AND worklogDate >= {} AND worklogDate <= {} ORDER BY updated DESC",
        w.from.format("%Y-%m-%d"),
        w.to.format("%Y-%m-%d")
    );
    let known: BTreeSet<IssueKey> = mine_raw.iter().map(|i| i.issue.key.clone()).chain(watched.iter().map(|i| i.key.clone())).collect();
    let history_raw: Vec<IssueWithWorklogs> = gateway
        .search_issues_with_worklogs(&history_jql, 200, &me.account_id)?
        .into_iter()
        .filter(|i| !known.contains(&i.issue.key))
        .collect();

    let mut mine = Vec::with_capacity(mine_raw.len());
    for i in mine_raw {
        worklogs.extend(embedded_or_refetch(gateway, &me.account_id, w, &i)?);
        mine.push(i.issue);
    }
    let mut history = Vec::with_capacity(history_raw.len());
    for i in history_raw {
        worklogs.extend(embedded_or_refetch(gateway, &me.account_id, w, &i)?);
        history.push(i.issue);
    }

    Ok(SyncOutput { account_id: me.account_id, display_name: me.display_name, mine, watched, history, worklogs, window: *w })
}

/// Use the embedded page when it is the whole list; otherwise ask the server
/// for just the window.
fn embedded_or_refetch(gateway: &dyn JiraGateway, account_id: &str, w: &Window, i: &IssueWithWorklogs) -> Result<Vec<RemoteWorklog>> {
    if i.total > EMBED_LIMIT {
        return gateway.my_worklogs(&i.issue.key, account_id, Some(w));
    }
    Ok(i.my_worklogs.iter().filter(|x| w.contains(x.local_date())).cloned().collect())
}
