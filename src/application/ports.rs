use super::{Config, Credentials};
use crate::domain::{Issue, IssueKey, Ledger, RemoteWorklog};
use anyhow::Result;
use chrono::{DateTime, Local};
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

/// Jira Cloud as the app needs it. Blocking; callers run it off the UI thread.
pub trait JiraGateway: Send + Sync {
    fn myself(&self) -> Result<Me>;
    fn search_issues(&self, jql: &str, max: usize) -> Result<Vec<Issue>>;
    fn get_issue(&self, key: &IssueKey) -> Result<Issue>;
    fn my_worklogs(&self, key: &IssueKey, account_id: &str) -> Result<Vec<RemoteWorklog>>;
    fn add_worklog(&self, req: &NewWorklog) -> Result<RemoteWorklog>;
    fn update_worklog(&self, worklog_id: &str, req: &NewWorklog) -> Result<RemoteWorklog>;
    fn delete_worklog(&self, issue_key: &IssueKey, worklog_id: &str) -> Result<()>;
}

/// Builds a gateway from credentials (needed before any config is saved).
pub type JiraGatewayFactory = Arc<dyn Fn(&Credentials) -> Result<Arc<dyn JiraGateway>> + Send + Sync>;

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
