use crate::application::config::{GlobalSettings, Holiday, YearSettings};
use crate::application::{Config, ConfigStore, Credentials};
use crate::domain::{SiteUrl, StartTime, duration};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

// ---- file shape -----------------------------------------------------------
//
// [connection]  base_url, email, api_token
// [global]      hours_per_day, workdays, default_start_time, quick_stage,
//               lookback_weeks, auto_watch, jql
// [year.2026]   hours_per_day, holidays = [{ date, name }]
//
// Pre-0.2 files had base_url / email / api_token / hours_per_day / jql /
// default_start_time at the top level; those still load.

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConnectionFile {
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api_token: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct GlobalFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hours_per_day: Option<u32>,
    /// e.g. ["mon","tue","wed","thu","fri"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workdays: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_start_time: Option<String>,
    /// Jira duration grammar, e.g. "1h"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quick_stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lookback_weeks: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auto_watch: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    jql: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HolidayFile {
    date: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct YearFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hours_per_day: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    holidays: Vec<HolidayFile>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    connection: Option<ConnectionFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    global: Option<GlobalFile>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    year: BTreeMap<String, YearFile>,
    // ---- legacy top-level keys (read only) ----
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hours_per_day: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    jql: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_start_time: Option<String>,
}

const DAY_NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

fn parse_workdays(names: &[String]) -> [bool; 7] {
    let mut out = [false; 7];
    for n in names {
        if let Some(i) = DAY_NAMES.iter().position(|d| d.eq_ignore_ascii_case(n.trim())) {
            out[i] = true;
        }
    }
    out
}

fn workday_names(w: &[bool; 7]) -> Vec<String> {
    DAY_NAMES.iter().zip(w).filter(|(_, on)| **on).map(|(n, _)| n.to_string()).collect()
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

        // connection: new section, else legacy top-level
        let conn = f.connection.unwrap_or_default();
        let base_url = if conn.base_url.is_empty() { f.base_url.clone().unwrap_or_default() } else { conn.base_url };
        let email = if conn.email.is_empty() { f.email.clone().unwrap_or_default() } else { conn.email };
        let file_token = conn.api_token.or(f.api_token);
        let token = std::env::var("JIRA_API_TOKEN").ok().filter(|t| !t.is_empty()).or(file_token);
        let Some(api_token) = token.filter(|t| !t.is_empty()) else {
            return Ok(None); // no token → needs the Connect screen
        };
        let site = SiteUrl::parse(&base_url)?;

        let g = f.global.unwrap_or_default();
        let defaults = GlobalSettings::default();
        let start_raw = g.default_start_time.or(f.default_start_time);
        let global = GlobalSettings {
            hours_per_day: g.hours_per_day.or(f.hours_per_day).unwrap_or(defaults.hours_per_day),
            workdays: g.workdays.as_deref().map(parse_workdays).unwrap_or(defaults.workdays),
            default_start: match start_raw.as_deref() {
                Some(s) => StartTime::parse(s).context("default_start_time")?,
                None => defaults.default_start,
            },
            quick_stage_seconds: match g.quick_stage.as_deref() {
                Some(s) => duration::parse(s).context("quick_stage")?,
                None => defaults.quick_stage_seconds,
            },
            lookback_weeks: g.lookback_weeks.unwrap_or(defaults.lookback_weeks),
            auto_watch: g.auto_watch.unwrap_or(defaults.auto_watch),
        };

        let mut years = BTreeMap::new();
        for (y, yf) in f.year {
            let Ok(year) = y.parse::<i32>() else { continue };
            let mut holidays = Vec::new();
            for h in yf.holidays {
                let date = NaiveDate::parse_from_str(h.date.trim(), "%Y-%m-%d").with_context(|| format!("holiday date '{}' in year {year}", h.date))?;
                holidays.push(Holiday { date, name: h.name });
            }
            holidays.sort_by_key(|h| h.date);
            years.insert(year, YearSettings { hours_per_day: yf.hours_per_day, holidays });
        }

        Ok(Some(Config {
            credentials: Credentials { site, email, api_token },
            global,
            years,
            jql: g.jql.or(f.jql).filter(|j| !j.trim().is_empty()),
        }))
    }

    fn save(&self, c: &Config) -> Result<()> {
        let f = ConfigFile {
            connection: Some(ConnectionFile {
                base_url: c.credentials.site.as_str().to_string(),
                email: c.credentials.email.clone(),
                api_token: Some(c.credentials.api_token.clone()),
            }),
            global: Some(GlobalFile {
                hours_per_day: Some(c.global.hours_per_day),
                workdays: Some(workday_names(&c.global.workdays)),
                default_start_time: Some(c.global.default_start.to_string()),
                quick_stage: Some(duration::format(c.global.quick_stage_seconds).replace(' ', "")),
                lookback_weeks: Some(c.global.lookback_weeks),
                auto_watch: Some(c.global.auto_watch),
                jql: c.jql.clone(),
            }),
            year: c
                .years
                .iter()
                .filter(|(_, ys)| !ys.is_empty())
                .map(|(y, ys)| {
                    (
                        y.to_string(),
                        YearFile {
                            hours_per_day: ys.hours_per_day,
                            holidays: ys.holidays.iter().map(|h| HolidayFile { date: h.date.format("%Y-%m-%d").to_string(), name: h.name.clone() }).collect(),
                        },
                    )
                })
                .collect(),
            ..Default::default()
        };
        let body = toml::to_string_pretty(&f)?;
        super::paths::write_atomic(&self.path, body.as_bytes(), Some(0o600))
    }

    fn location(&self) -> String {
        self.path.display().to_string()
    }
}
