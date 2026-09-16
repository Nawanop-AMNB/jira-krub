use crate::domain::{SiteUrl, StartTime, WorkCalendar};
use crate::domain::calendar::MON_TO_FRI;
use chrono::NaiveDate;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub site: SiteUrl,
    pub email: String,
    pub api_token: String,
}

/// Settings that apply to every year.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSettings {
    pub hours_per_day: u32,
    /// Mon..Sun
    pub workdays: [bool; 7],
    pub default_start: StartTime,
    pub quick_stage_seconds: u64,
    pub lookback_weeks: u32,
    /// Staging a Jira search result also adds it to the watchlist.
    pub auto_watch: bool,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self { hours_per_day: 8, workdays: MON_TO_FRI, default_start: StartTime::NINE, quick_stage_seconds: 3600, lookback_weeks: 3, auto_watch: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holiday {
    pub date: NaiveDate,
    pub name: String,
}

/// Settings for one calendar year.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct YearSettings {
    /// Overrides `GlobalSettings::hours_per_day` for this year.
    pub hours_per_day: Option<u32>,
    pub holidays: Vec<Holiday>,
}

impl YearSettings {
    pub fn is_empty(&self) -> bool {
        self.hours_per_day.is_none() && self.holidays.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub credentials: Credentials,
    pub global: GlobalSettings,
    pub years: BTreeMap<i32, YearSettings>,
    /// Advanced: override the "work for me" JQL (file only, no UI).
    pub jql: Option<String>,
}

pub const DEFAULT_JQL: &str =
    "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";

impl Config {
    pub fn with_defaults(credentials: Credentials) -> Self {
        Self { credentials, global: GlobalSettings::default(), years: BTreeMap::new(), jql: None }
    }
    pub fn jql(&self) -> &str {
        self.jql.as_deref().unwrap_or(DEFAULT_JQL)
    }
    pub fn year(&self, y: i32) -> YearSettings {
        self.years.get(&y).cloned().unwrap_or_default()
    }
    /// The calendar the UI and summaries run on.
    pub fn calendar(&self) -> WorkCalendar {
        let mut cal = WorkCalendar::simple(self.global.hours_per_day as u64 * 3600);
        cal.workdays = self.global.workdays;
        for (year, ys) in &self.years {
            if let Some(h) = ys.hours_per_day {
                cal.year_target.insert(*year, h as u64 * 3600);
            }
            for hol in &ys.holidays {
                cal.holidays.insert(hol.date, hol.name.clone());
            }
        }
        cal
    }
}
