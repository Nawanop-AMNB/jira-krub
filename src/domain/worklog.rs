use super::IssueKey;
use chrono::{DateTime, FixedOffset, Local, NaiveDate};

/// A worklog as Jira holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorklog {
    pub id: String,
    pub issue_key: IssueKey,
    pub started: DateTime<FixedOffset>,
    pub seconds: u64,
    pub comment: String,
}

impl RemoteWorklog {
    /// Calendar day in the local timezone.
    pub fn local_date(&self) -> NaiveDate {
        self.started.with_timezone(&Local).date_naive()
    }
}
