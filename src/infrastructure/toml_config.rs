use crate::application::{Config, ConfigStore, Credentials};
use crate::domain::{SiteUrl, StartTime};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct ConfigFile {
    base_url: String,
    email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hours_per_day: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    jql: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_start_time: Option<String>,
}

pub struct TomlConfigStore {
    path: PathBuf,
}

impl TomlConfigStore {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn default_location() -> Result<Self> {
        Ok(Self::at(super::paths::config_file()?))
    }
}

impl ConfigStore for TomlConfigStore {
    fn load(&self) -> Result<Option<Config>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&self.path).with_context(|| format!("read {}", self.path.display()))?;
        let f: ConfigFile = toml::from_str(&raw).with_context(|| format!("parse {}", self.path.display()))?;
        let token = std::env::var("JIRA_API_TOKEN").ok().filter(|t| !t.is_empty()).or(f.api_token);
        let Some(api_token) = token.filter(|t| !t.is_empty()) else {
            // Config without a token → treat as "needs setup".
            return Ok(None);
        };
        let site = SiteUrl::parse(&f.base_url)?;
        let default_start = match f.default_start_time.as_deref() {
            Some(s) => StartTime::parse(s).context("default_start_time")?,
            None => StartTime::NINE,
        };
        Ok(Some(Config {
            credentials: Credentials { site, email: f.email, api_token },
            hours_per_day: f.hours_per_day.unwrap_or(8),
            jql: f.jql.filter(|j| !j.trim().is_empty()),
            default_start,
        }))
    }

    fn save(&self, c: &Config) -> Result<()> {
        let f = ConfigFile {
            base_url: c.credentials.site.as_str().to_string(),
            email: c.credentials.email.clone(),
            api_token: Some(c.credentials.api_token.clone()),
            hours_per_day: Some(c.hours_per_day),
            jql: c.jql.clone(),
            default_start_time: Some(c.default_start.to_string()),
        };
        let body = toml::to_string_pretty(&f)?;
        super::paths::write_atomic(&self.path, body.as_bytes(), Some(0o600))
    }

    fn location(&self) -> String {
        self.path.display().to_string()
    }
}
