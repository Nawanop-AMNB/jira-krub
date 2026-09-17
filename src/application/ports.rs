use super::{Config, Credentials};
use crate::domain::{Issue, IssueKey, Ledger, RemoteWorklog};
use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Me {
    pub account_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct NewWorklog {
    pub issue_key: IssueKey,
    pub started: DateTime<Local>,
    pub seconds: u64,
    pub comment_paragraphs: Vec<String>,
}

/// Inclusive range of local calendar days worklogs are wanted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

impl Window {
    pub fn contains(&self, d: NaiveDate) -> bool {
        d >= self.from && d <= self.to
    }
    pub fn covers(&self, other: &Window) -> bool {
        self.from <= other.from && self.to >= other.to
    }
}

/// An issue from a search that asked for the `worklog` field. Jira embeds at
/// most the first `EMBED_LIMIT` worklogs (all authors, oldest first) plus
/// `total`; `my_worklogs` is that embedded page filtered to one author.
#[derive(Debug, Clone)]
pub struct IssueWithWorklogs {
    pub issue: Issue,
    pub my_worklogs: Vec<RemoteWorklog>,
    pub total: u64,
}

/// How many worklogs Jira embeds per issue in a search response.
pub const EMBED_LIMIT: u64 = 20;

/// Jira Cloud as the app needs it. Blocking; callers run it off the UI thread.
pub trait JiraGateway: Send + Sync {
    fn myself(&self) -> Result<Me>;
    fn search_issues(&self, jql: &str, max: usize) -> Result<Vec<Issue>>;
    /// Same search, plus each issue's embedded worklog page for `account_id`.
    fn search_issues_with_worklogs(&self, jql: &str, max: usize, account_id: &str) -> Result<Vec<IssueWithWorklogs>>;
    fn get_issue(&self, key: &IssueKey) -> Result<Issue>;
    /// Worklogs on one issue by `account_id`, optionally only those started
    /// inside `window` (server-side filter, cheap even for years-old issues).
    fn my_worklogs(&self, key: &IssueKey, account_id: &str, window: Option<&Window>) -> Result<Vec<RemoteWorklog>>;
    fn add_worklog(&self, req: &NewWorklog) -> Result<RemoteWorklog>;
    fn update_worklog(&self, worklog_id: &str, req: &NewWorklog) -> Result<RemoteWorklog>;
    fn delete_worklog(&self, issue_key: &IssueKey, worklog_id: &str) -> Result<()>;
}

/// Builds a gateway from credentials (needed before any config is saved).
pub type JiraGatewayFactory = Arc<dyn Fn(&Credentials) -> Result<Arc<dyn JiraGateway>> + Send + Sync>;

/// Opens a URL in the user's default browser. A port so the TUI stays
/// testable; the real implementation lives in `infrastructure`.
pub trait UrlOpener: Send + Sync {
    fn open(&self, url: &str) -> Result<()>;
}

pub trait StateStore: Send + Sync {
    fn load(&self) -> Result<Ledger>;
    fn save(&self, ledger: &Ledger) -> Result<()>;
}

pub trait ConfigStore: Send + Sync {
    fn load(&self) -> Result<Option<Config>>;
    fn save(&self, config: &Config) -> Result<()>;
    /// Human-readable location, for the UI.
    fn location(&self) -> String;
}

/// Classified gateway failure so the UI can route (setup vs offline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayErrorKind {
    Unauthorized,
    NotFound,
    Network,
    Other,
}

/// Errors from gateways carry this so callers can classify without parsing text.
#[derive(Debug, Clone)]
pub struct GatewayError {
    pub kind: GatewayErrorKind,
    pub message: String,
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for GatewayError {}

pub fn classify(err: &anyhow::Error) -> GatewayErrorKind {
    err.downcast_ref::<GatewayError>().map(|e| e.kind.clone()).unwrap_or(GatewayErrorKind::Other)
}
