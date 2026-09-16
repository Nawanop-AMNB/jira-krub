use super::{IssueKey, StartTime};
use chrono::NaiveDate;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryId(String);

impl EntryId {
    pub fn new(raw: String) -> Self {
        Self(raw)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What the next push must do for an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Create,
    Update { worklog_id: String },
    Delete { worklog_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryState {
    /// New, local only.
    Staged,
    /// In Jira and identical locally.
    Pushed { worklog_id: String },
    /// In Jira, but edited locally; needs an update.
    Modified { worklog_id: String },
    /// In Jira, marked for deletion.
    Deleted { worklog_id: String },
    /// Last push attempt failed; `intent` says what to retry.
    Failed { error: String, intent: Intent },
}

/// A unit of work the user intends to (or did) log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: EntryId,
    pub issue_key: IssueKey,
    pub date: NaiveDate,
    pub start: StartTime,
    pub seconds: u64,
    /// Worklog comment. One line per Jira paragraph.
    pub title: String,
    pub state: EntryState,
}

impl Entry {
    pub fn is_pushed(&self) -> bool {
        matches!(self.state, EntryState::Pushed { .. })
    }
    pub fn is_deleted(&self) -> bool {
        matches!(self.state, EntryState::Deleted { .. })
            || matches!(&self.state, EntryState::Failed { intent: Intent::Delete { .. }, .. })
    }
    pub fn is_modified(&self) -> bool {
        matches!(self.state, EntryState::Modified { .. })
            || matches!(&self.state, EntryState::Failed { intent: Intent::Update { .. }, .. })
    }
    pub fn is_failed(&self) -> bool {
        matches!(self.state, EntryState::Failed { .. })
    }
    /// Anything but `Pushed` still has to go to Jira.
    pub fn needs_push(&self) -> bool {
        !self.is_pushed()
    }
    /// Jira worklog id, if this entry exists in Jira (in any state).
    pub fn worklog_id(&self) -> Option<&str> {
        match &self.state {
            EntryState::Pushed { worklog_id } | EntryState::Modified { worklog_id } | EntryState::Deleted { worklog_id } => Some(worklog_id),
            EntryState::Failed { intent: Intent::Update { worklog_id } | Intent::Delete { worklog_id }, .. } => Some(worklog_id),
            EntryState::Staged | EntryState::Failed { intent: Intent::Create, .. } => None,
        }
    }
    /// What a push should do. `None` when nothing is pending.
    pub fn intent(&self) -> Option<Intent> {
        match &self.state {
            EntryState::Staged => Some(Intent::Create),
            EntryState::Pushed { .. } => None,
            EntryState::Modified { worklog_id } => Some(Intent::Update { worklog_id: worklog_id.clone() }),
            EntryState::Deleted { worklog_id } => Some(Intent::Delete { worklog_id: worklog_id.clone() }),
            EntryState::Failed { intent, .. } => Some(intent.clone()),
        }
    }
    pub fn has_title(&self) -> bool {
        !self.title.trim().is_empty()
    }
    /// Jira comment paragraphs: one per non-empty line of the description.
    pub fn comment_paragraphs(&self) -> Vec<String> {
        self.title.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
    }

    pub fn end(&self) -> StartTime {
        self.start.plus_seconds(self.seconds)
    }
    pub fn overlaps(&self, other: &Entry) -> bool {
        self.date == other.date
            && self.id != other.id
            && !self.is_deleted()
            && !other.is_deleted()
            && self.start < other.end()
            && other.start < self.end()
    }
}
