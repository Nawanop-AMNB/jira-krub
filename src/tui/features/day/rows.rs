//! Derived row lists for both panes.

use super::model::Model;
use crate::domain::{Entry, Issue, IssueKey, RemoteWorklog, StartTime, duration};
use crate::tui::app::App;
use chrono::Timelike;
use std::collections::HashSet;

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

#[derive(Debug, Clone)]
pub struct TicketRow {
    pub issue: Issue,
    pub section: Section,
    pub watched: bool,
}

/// Selectable ticket rows in display order (headers are drawn separately).
pub fn ticket_rows(app: &App, m: &Model) -> Vec<TicketRow> {
    let q = m.query();
    let mut out = Vec::new();
    let mut seen: HashSet<IssueKey> = HashSet::new();

    for key in app.ledger.watchlist() {
        let issue = app.remote.issue(key).cloned().unwrap_or_else(|| Issue {
            key: key.clone(),
            summary: if app.remote.offline { "(offline)".into() } else { "…".into() },
            status: String::new(),
        });
        if issue.matches(&q) {
            seen.insert(issue.key.clone());
            out.push(TicketRow { issue, section: Section::Watch, watched: true });
        }
    }
    for i in &app.remote.mine {
        if !app.ledger.is_watched(&i.key) && i.matches(&q) && seen.insert(i.key.clone()) {
            out.push(TicketRow { issue: i.clone(), section: Section::Mine, watched: false });
        }
    }
    for i in &app.remote.history {
        if !app.ledger.is_watched(&i.key) && i.matches(&q) && seen.insert(i.key.clone()) {
            out.push(TicketRow { issue: i.clone(), section: Section::History, watched: false });
        }
    }
    for i in &m.s.results {
        if seen.insert(i.key.clone()) {
            out.push(TicketRow { issue: i.clone(), section: Section::Jira, watched: app.ledger.is_watched(&i.key) });
        }
    }
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
    let mut pending: Vec<PrepareRow> = Vec::new();
    let mut jira: Vec<PrepareRow> = Vec::new();
    let mut known_ids: HashSet<String> = HashSet::new();
    for e in app.ledger.entries_on(m.date) {
        if let Some(id) = e.worklog_id() {
            known_ids.insert(id.to_string());
        }
        if e.needs_push() {
            pending.push(PrepareRow::Local(e.clone()));
        } else {
            jira.push(PrepareRow::Local(e.clone()));
        }
    }
    for w in app.remote.worklogs.iter().filter(|w| w.local_date() == m.date) {
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
