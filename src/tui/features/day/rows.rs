//! Derived row lists for both panes.

use super::model::Model;
use crate::domain::{Entry, Head, Issue, IssueKey, Nested, RemoteWorklog, StartTime, StatusCategory, duration, nest};
use crate::tui::app::App;
use chrono::Timelike;
use std::collections::HashSet;

/// At most this many children render per jira-section group before a
/// `… +N more` row takes over; expanding it lifts the cap for that group.
pub const JIRA_CHILD_CAP: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Watch,
    Mine,
    History,
    Jira,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Watch => "watchlist",
            Section::Mine => "mine",
            Section::History => "logged recently",
            Section::Jira => "jira",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// A plain ticket, or a sub-task's parent — selectable and stageable.
    Normal,
    /// A sub-task, indented under its parent's row or a `Context` head.
    Child,
    /// `… +N more`: expands its group in place. Never stages, never watches.
    More,
}

#[derive(Debug, Clone)]
pub struct TicketRow {
    pub issue: Issue,
    pub section: Section,
    pub watched: bool,
    pub kind: RowKind,
    /// True on the first selectable row of a nested group (the `Normal` head,
    /// or the first `Child` when the head is a `Context` not shown as a row
    /// of its own). Used to count the jira section's top-level groups.
    pub group_start: bool,
    /// Set on a `group_start` row whose real parent isn't in the list: the
    /// dim, unselectable line to draw just above it.
    pub context: Option<crate::domain::ParentRef>,
    /// On a `Normal` row that heads a group: the number of children in it,
    /// before any cap (for the `▸N` marker). Zero otherwise.
    pub child_count: usize,
    /// On a `More` row: how many further children are hidden.
    pub hidden_count: usize,
}

/// One nested group flattened into selectable rows (plus, when the head is a
/// `Context`, the extra dim line carried on the first child's `context`).
/// `cap` limits how many children show before a `More` row takes over;
/// `None` means show them all.
fn flatten_group(section: Section, group: Nested, watched: &dyn Fn(&IssueKey) -> bool, cap: Option<usize>) -> Vec<TicketRow> {
    let (group_key, group_summary) = match &group.head {
        Head::Issue(i) => (i.key.clone(), i.summary.clone()),
        Head::Context(p) => (p.key.clone(), p.summary.clone()),
    };
    let mut out = Vec::new();
    let mut context = match group.head {
        Head::Issue(issue) => {
            let child_count = group.children.len();
            out.push(TicketRow {
                watched: watched(&issue.key),
                issue,
                section,
                kind: RowKind::Normal,
                group_start: true,
                context: None,
                child_count,
                hidden_count: 0,
            });
            None
        }
        Head::Context(p) => Some(p),
    };

    let total = group.children.len();
    let shown = cap.filter(|&n| total > n).unwrap_or(total);
    for child in group.children.into_iter().take(shown) {
        out.push(TicketRow {
            watched: watched(&child.key),
            issue: child,
            section,
            kind: RowKind::Child,
            group_start: context.is_some(),
            context: context.take(),
            child_count: 0,
            hidden_count: 0,
        });
    }
    if total > shown {
        out.push(TicketRow {
            watched: false,
            issue: Issue::new(group_key, group_summary, ""),
            section,
            kind: RowKind::More,
            group_start: false,
            context: None,
            child_count: 0,
            hidden_count: total - shown,
        });
    }
    out
}

/// Adds each issue (if not already `seen`) followed by its own open
/// (non-Done) sub-tasks (if not already `seen`) to a flat combined list —
/// shared by the jira section's search results and the mine section's
/// assigned cards, both of which can carry their own `subtasks` payload.
fn expand_with_open_subtasks(issues: &[Issue], seen: &mut HashSet<IssueKey>) -> Vec<Issue> {
    let mut combined = Vec::new();
    for issue in issues {
        if seen.insert(issue.key.clone()) {
            combined.push(issue.clone());
        }
        for child in &issue.subtasks {
            if child.status_category != StatusCategory::Done && seen.insert(child.key.clone()) {
                combined.push(child.clone());
            }
        }
    }
    combined
}

/// [`expand_with_open_subtasks`], then the local filter. Only rows that
/// survive the filter count as listed, so a card hidden here can still show
/// up as a Jira search hit (Jira's text search also matches descriptions).
fn expand_and_filter(issues: &[Issue], query: &str, seen: &mut HashSet<IssueKey>) -> Vec<Issue> {
    let mut local = seen.clone();
    let kept: Vec<Issue> = expand_with_open_subtasks(issues, &mut local).into_iter().filter(|i| i.matches(query)).collect();
    seen.extend(kept.iter().map(|i| i.key.clone()));
    kept
}

fn group_key(head: &Head) -> IssueKey {
    match head {
        Head::Issue(i) => i.key.clone(),
        Head::Context(p) => p.key.clone(),
    }
}

/// The jira ("search results") section: a result with `subtasks` groups with
/// its own (non-done) children even though they are not separate hits; a
/// result that is itself a sub-task shares one head (the real parent if it is
/// also a result, else a `Context` row) with every other hit under the same
/// parent. Capped at [`JIRA_CHILD_CAP`] children per group unless expanded.
fn jira_rows(app: &App, m: &Model, seen: &mut HashSet<IssueKey>) -> Vec<TicketRow> {
    // A result already listed above (say, my own card) is not repeated, but
    // its sub-tasks still are: search is how unassigned ones are reached.
    // They then nest under a `Context` head.
    let combined = expand_with_open_subtasks(&m.s.results, seen);
    let watched = |k: &IssueKey| app.ledger.is_watched(k);
    nest(&combined)
        .into_iter()
        .flat_map(|group| {
            let cap = if m.s.expanded.contains(&group_key(&group.head)) { None } else { Some(JIRA_CHILD_CAP) };
            flatten_group(Section::Jira, group, &watched, cap)
        })
        .collect()
}

/// Selectable ticket rows in display order (headers are drawn separately).
/// Within watchlist/mine/logged-recently, rows are nested with the domain
/// helper; grouping never crosses a section. The jira section has its own
/// rules — see [`jira_rows`].
pub fn ticket_rows(app: &App, m: &Model) -> Vec<TicketRow> {
    let q = m.query();
    let mut seen: HashSet<IssueKey> = HashSet::new();

    // A watched card that is also assigned to me comes from `mine`, which
    // carries its `subtasks`; the per-key watchlist fetch does not.
    let watch_pool: Vec<Issue> = app
        .ledger
        .watchlist()
        .iter()
        .map(|key| {
            app.remote.mine.iter().find(|i| &i.key == key).or_else(|| app.remote.issue(key)).cloned().unwrap_or_else(|| {
                Issue::new(key.clone(), if app.remote.offline { "(offline)" } else { "…" }, "")
            })
        })
        .collect();
    let watch_issues = expand_and_filter(&watch_pool, &q, &mut seen);
    // Each assigned card contributes itself plus its own open sub-tasks —
    // the common case is a card assigned to me whose sub-task isn't, so the
    // sub-task never gets a search hit or a row of its own otherwise.
    let mine_pool: Vec<Issue> = app.remote.mine.iter().filter(|i| !app.ledger.is_watched(&i.key)).cloned().collect();
    let mine_issues = expand_and_filter(&mine_pool, &q, &mut seen);
    let mut history_issues = Vec::new();
    for i in &app.remote.history {
        if !app.ledger.is_watched(&i.key) && i.matches(&q) && seen.insert(i.key.clone()) {
            history_issues.push(i.clone());
        }
    }

    let watched = |k: &IssueKey| app.ledger.is_watched(k);
    let mut out = Vec::new();
    // Same cap as the jira section: a card with many open sub-tasks doesn't
    // flood the list, and the same `expanded` state (keyed by group/parent
    // key) lifts it in place.
    let cap = |head: &Head| if m.s.expanded.contains(&group_key(head)) { None } else { Some(JIRA_CHILD_CAP) };
    for group in nest(&watch_issues) {
        let cap = cap(&group.head);
        out.extend(flatten_group(Section::Watch, group, &watched, cap));
    }
    for group in nest(&mine_issues) {
        let cap = cap(&group.head);
        out.extend(flatten_group(Section::Mine, group, &watched, cap));
    }
    for group in nest(&history_issues) {
        out.extend(flatten_group(Section::History, group, &watched, None));
    }
    out.extend(jira_rows(app, m, &mut seen));
    out
}

/// One line in the prepare pane: a local entry (any state) or a worklog that
/// only exists in Jira so far.
#[derive(Debug, Clone)]
pub enum PrepareRow {
    Local(Entry),
    Remote(RemoteWorklog),
}

impl PrepareRow {
    pub fn key(&self) -> &IssueKey {
        match self {
            PrepareRow::Local(e) => &e.issue_key,
            PrepareRow::Remote(w) => &w.issue_key,
        }
    }
    pub fn start(&self) -> StartTime {
        match self {
            PrepareRow::Local(e) => e.start,
            PrepareRow::Remote(w) => {
                let l = w.started.with_timezone(&chrono::Local);
                StartTime::from_hm(l.hour() as u16, l.minute() as u16).unwrap_or(StartTime::NINE)
            }
        }
    }
    pub fn seconds(&self) -> u64 {
        match self {
            PrepareRow::Local(e) => e.seconds,
            PrepareRow::Remote(w) => w.seconds,
        }
    }
    pub fn title(&self) -> String {
        match self {
            PrepareRow::Local(e) => e.title.clone(),
            PrepareRow::Remote(w) => w.comment.clone(),
        }
    }
    pub fn is_deleted(&self) -> bool {
        matches!(self, PrepareRow::Local(e) if e.is_deleted())
    }
    /// Stable identity across adoption: the Jira worklog id when there is
    /// one, else the local entry id.
    pub fn identity(&self) -> String {
        match self {
            PrepareRow::Local(e) => e.worklog_id().map(|w| format!("w:{w}")).unwrap_or_else(|| format!("e:{}", e.id)),
            PrepareRow::Remote(w) => format!("w:{}", w.id),
        }
    }
    pub fn local(&self) -> Option<&Entry> {
        match self {
            PrepareRow::Local(e) => Some(e),
            PrepareRow::Remote(_) => None,
        }
    }
    fn order_key(&self) -> (StartTime, String) {
        let id = match self {
            PrepareRow::Local(e) => e.id.as_str().to_string(),
            PrepareRow::Remote(w) => w.id.clone(),
        };
        (self.start(), id)
    }
}

pub struct PrepareRows {
    /// Pending rows first, then rows already in Jira.
    pub rows: Vec<PrepareRow>,
    pub pending_len: usize,
}

impl PrepareRows {
    pub fn pending(&self) -> &[PrepareRow] {
        &self.rows[..self.pending_len]
    }
    pub fn in_jira(&self) -> &[PrepareRow] {
        &self.rows[self.pending_len..]
    }
    pub fn index_of_entry(&self, id: &crate::domain::EntryId) -> Option<usize> {
        self.rows.iter().position(|r| matches!(r, PrepareRow::Local(e) if &e.id == id))
    }
}

pub fn prepare_rows(app: &App, m: &Model) -> PrepareRows {
    let mut rows = prepare_rows_for(app, m.date);
    if let Some(frozen) = &m.frozen {
        // Keep the order the user saw when the walk started; rows that
        // appeared since go after, in their natural order.
        let n = rows.pending_len;
        let pos = |r: &PrepareRow| frozen.iter().position(|id| *id == r.identity()).unwrap_or(usize::MAX);
        rows.rows[..n].sort_by_key(pos);
    }
    rows
}

pub fn prepare_rows_for(app: &App, date: chrono::NaiveDate) -> PrepareRows {
    let mut pending: Vec<PrepareRow> = Vec::new();
    let mut jira: Vec<PrepareRow> = Vec::new();
    let mut known_ids: HashSet<String> = HashSet::new();
    for e in app.ledger.entries_on(date) {
        if let Some(id) = e.worklog_id() {
            known_ids.insert(id.to_string());
        }
        if e.needs_push() {
            pending.push(PrepareRow::Local(e.clone()));
        } else {
            jira.push(PrepareRow::Local(e.clone()));
        }
    }
    for w in app.remote.worklogs.iter().filter(|w| w.local_date() == date) {
        if !known_ids.contains(&w.id) {
            jira.push(PrepareRow::Remote(w.clone()));
        }
    }
    pending.sort_by_key(|r| r.order_key());
    jira.sort_by_key(|r| r.order_key());
    let pending_len = pending.len();
    pending.extend(jira);
    PrepareRows { rows: pending, pending_len }
}

/// Seconds that will exist after the push (pending, excluding deletes).
pub fn staged_total(rows: &PrepareRows) -> u64 {
    rows.pending().iter().filter(|r| !r.is_deleted()).map(|r| r.seconds()).sum()
}
/// Seconds currently in Jira and untouched.
pub fn pushed_total(rows: &PrepareRows) -> u64 {
    rows.in_jira().iter().map(|r| r.seconds()).sum()
}

pub fn fmt0(secs: u64) -> String {
    if secs == 0 { "0".into() } else { duration::format(secs) }
}
