use crate::domain::{Issue, IssueKey, StartTime};
use crate::tui::widgets::TextInput;
use chrono::NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Issue,
    Date,
    Start,
    Duration,
    /// Description: the worklog comment, multi-line.
    Title,
    /// The Save button: end of the walk.
    Save,
}

impl Field {
    pub const ORDER: [Field; 6] = [Field::Issue, Field::Date, Field::Start, Field::Duration, Field::Title, Field::Save];
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
    pub focus: Field,
    pub error: Option<String>,
    /// Issues to suggest from (local: watchlist + mine + history).
    pub candidates: Vec<Issue>,
    pub suggestion_sel: usize,
    /// EDIT mode: the focused field is open for typing.
    pub editing: bool,
    /// Value to restore on Esc (text, or date for the Date field).
    pub backup: String,
    pub backup_date: NaiveDate,
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
            focus,
            error: None,
            candidates,
            suggestion_sel: 0,
            // A walk form starts with its first field open.
            editing: true,
            backup: String::new(),
            backup_date: date,
        }
    }

    pub fn field_mut(&mut self, f: Field) -> Option<&mut TextInput> {
        match f {
            Field::Issue => Some(&mut self.issue),
            Field::Start => Some(&mut self.start),
            Field::Duration => Some(&mut self.duration),
            Field::Title => Some(&mut self.title),
            Field::Date | Field::Save => None,
        }
    }

    /// Snapshot the focused field so Esc can restore it.
    pub fn snapshot(&mut self) {
        self.backup_date = self.date;
        let f = self.focus;
        self.backup = self.field_mut(f).map(|t| t.text().to_string()).unwrap_or_default();
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
    /// Ctrl+Enter / Shift+Enter / Ctrl+J in the description.
    Newline,
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
    /// Enter in NAV: open the focused field (or Save on the button).
    Open,
    /// Commit the open field; +1 walks to the next field, -1 back.
    Commit(i32),
    /// Esc in EDIT: restore the field, back to NAV.
    Revert,
    Save,
    Cancel,
}
