use crate::domain::{SiteUrl, StartTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub site: SiteUrl,
    pub email: String,
    pub api_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub credentials: Credentials,
    pub hours_per_day: u32,
    pub jql: Option<String>,
    pub default_start: StartTime,
}

pub const DEFAULT_JQL: &str =
    "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";

impl Config {
    pub fn with_defaults(credentials: Credentials) -> Self {
        Self { credentials, hours_per_day: 8, jql: None, default_start: StartTime::NINE }
    }
    pub fn target_seconds(&self) -> u64 {
        self.hours_per_day as u64 * 3600
    }
    pub fn weekly_target_seconds(&self) -> u64 {
        self.target_seconds() * 5
    }
    pub fn jql(&self) -> &str {
        self.jql.as_deref().unwrap_or(DEFAULT_JQL)
    }
}
