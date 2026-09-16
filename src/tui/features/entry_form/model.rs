use crate::domain::{Issue, IssueKey, StartTime};
use crate::tui::widgets::TextInput;
use chrono::NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Issue,
    Date,
    Start,
    Duration,
    Title,
    Detail,
}

impl Field {
    pub const ORDER: [Field; 6] = [Field::Issue, Field::Date, Field::Start, Field::Duration, Field::Title, Field::Detail];
    pub fn next(self) -> Self {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }
    pub fn prev(self) -> Self {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

pub const MAX_SUGGESTIONS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub issue: TextInput,
    pub date: NaiveDate,
    pub start: TextInput,
    pub duration: TextInput,
    pub title: TextInput,
    pub detail: TextInput,
    pub focus: Field,
    pub error: Option<String>,
    /// Issues to suggest from (local: watchlist + mine + history).
    pub candidates: Vec<Issue>,
    pub suggestion_sel: usize,
}

impl Model {
    pub fn new_entry(date: NaiveDate, issue: Option<&Issue>, start: StartTime, candidates: Vec<Issue>) -> Self {
        let focus = if issue.is_some() { Field::Duration } else { Field::Issue };
        Self {
            issue: TextInput::with(issue.map(|i| i.key.as_str().to_string()).unwrap_or_default()),
            date,
            start: TextInput::with(start.to_string()),
            duration: TextInput::default(),
            title: TextInput::default(),
            detail: TextInput::default(),
            focus,
            error: None,
            candidates,
            suggestion_sel: 0,
        }
    }

    pub fn field_mut(&mut self, f: Field) -> Option<&mut TextInput> {
        match f {
            Field::Issue => Some(&mut self.issue),
            Field::Start => Some(&mut self.start),
            Field::Duration => Some(&mut self.duration),
            Field::Title => Some(&mut self.title),
            Field::Detail => Some(&mut self.detail),
            Field::Date => None,
        }
    }

    /// Candidates matching the typed issue text, unless it already equals a key.
    pub fn suggestions(&self) -> Vec<&Issue> {
        let q = self.issue.text().trim();
        if q.is_empty() {
            return self.candidates.iter().take(MAX_SUGGESTIONS).collect();
        }
        let exact = self.candidates.iter().any(|i| i.key.as_str().eq_ignore_ascii_case(q));
        if exact {
            return Vec::new();
        }
        self.candidates.iter().filter(|i| i.matches(q)).take(MAX_SUGGESTIONS).collect()
    }

    /// Resolve the issue field: chosen suggestion, exact candidate, or a key-shaped string.
    pub fn resolve_issue(&self) -> Option<IssueKey> {
        let q = self.issue.text().trim();
        if let Some(i) = self.candidates.iter().find(|i| i.key.as_str().eq_ignore_ascii_case(q)) {
            return Some(i.key.clone());
        }
        let sugg = self.suggestions();
        if !sugg.is_empty() && !q.is_empty() {
            return sugg.get(self.suggestion_sel.min(sugg.len() - 1)).map(|i| i.key.clone());
        }
        IssueKey::parse(q).ok()
    }

    pub fn issue_summary(&self) -> Option<&str> {
        let key = self.resolve_issue()?;
        self.candidates.iter().find(|i| i.key == key).map(|i| i.summary.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    FocusNext,
    FocusPrev,
    Focus(Field),
    Char(char),
    Paste(String),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    DateShift(i64),
    StartStep(i32),
    SuggestNext,
    SuggestPrev,
    /// Enter on the issue field: accept suggestion and move on.
    AcceptIssue,
    Save,
    Cancel,
}
