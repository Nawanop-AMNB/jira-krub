use crate::domain::{SiteUrl, StartTime, WorkCalendar};
use crate::domain::calendar::MON_TO_FRI;
use chrono::{Datelike, NaiveDate};
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
    /// The day off recorded for `date`, if any.
    pub fn day_off(&self, date: NaiveDate) -> Option<&Holiday> {
        self.years.get(&date.year())?.holidays.iter().find(|h| h.date == date)
    }
    /// Record a day off on `date` (replacing the name if one is already
    /// there). `moved_from` is the date it used to be on: that entry is
    /// dropped, so editing a date moves the day off rather than copying it.
    pub fn set_day_off(&mut self, moved_from: Option<NaiveDate>, date: NaiveDate, name: String) {
        if let Some(old) = moved_from {
            self.remove_day_off(old);
        }
        let ys = self.years.entry(date.year()).or_default();
        ys.holidays.retain(|h| h.date != date);
        ys.holidays.push(Holiday { date, name });
        ys.holidays.sort_by_key(|h| h.date);
    }
    /// Forget the day off on `date`; a year left with no settings is dropped.
    pub fn remove_day_off(&mut self, date: NaiveDate) {
        let year = date.year();
        if let Some(ys) = self.years.get_mut(&year) {
            ys.holidays.retain(|h| h.date != date);
            if ys.is_empty() {
                self.years.remove(&year);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::with_defaults(Credentials {
            site: SiteUrl::parse("https://acme.atlassian.net").unwrap(),
            email: "me@example.com".into(),
            api_token: "tok".into(),
        })
    }
    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn set_day_off_creates_the_year_and_keeps_dates_sorted_one_per_date() {
        let mut c = cfg();
        c.set_day_off(None, d(2026, 5, 2), "b".into());
        c.set_day_off(None, d(2026, 1, 2), "a".into());
        c.set_day_off(None, d(2026, 5, 2), "b2".into());
        let names: Vec<_> = c.year(2026).holidays.iter().map(|h| (h.date, h.name.clone())).collect();
        assert_eq!(names, vec![(d(2026, 1, 2), "a".to_string()), (d(2026, 5, 2), "b2".to_string())]);
    }

    #[test]
    fn set_day_off_with_moved_from_moves_across_years() {
        let mut c = cfg();
        c.set_day_off(None, d(2026, 12, 31), "x".into());
        c.set_day_off(Some(d(2026, 12, 31)), d(2027, 1, 1), "x".into());
        assert!(c.day_off(d(2026, 12, 31)).is_none());
        assert_eq!(c.day_off(d(2027, 1, 1)).map(|h| h.name.as_str()), Some("x"));
        assert!(!c.years.contains_key(&2026), "emptied year is dropped");
    }

    #[test]
    fn remove_day_off_keeps_a_year_that_still_has_settings() {
        let mut c = cfg();
        c.years.insert(2026, YearSettings { hours_per_day: Some(7), holidays: Vec::new() });
        c.set_day_off(None, d(2026, 3, 3), "x".into());
        c.remove_day_off(d(2026, 3, 3));
        assert_eq!(c.year(2026).hours_per_day, Some(7));
        assert!(c.year(2026).holidays.is_empty());
    }
}
