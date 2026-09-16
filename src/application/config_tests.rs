//! `Config::calendar()`, `Config::year()` and `Config::jql()` (R16).

use super::config::{Config, Credentials, DEFAULT_JQL, GlobalSettings, Holiday, YearSettings};
use crate::domain::{SiteUrl, StartTime};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn holiday(y: i32, m: u32, day: u32, name: &str) -> Holiday {
    Holiday { date: d(y, m, day), name: name.to_string() }
}

fn config() -> Config {
    Config::with_defaults(Credentials {
        site: SiteUrl::parse("https://acme.atlassian.net").unwrap(),
        email: "me@example.com".into(),
        api_token: "tok".into(),
    })
}

#[test]
fn calendar_copies_the_configured_workdays() {
    let mut c = config();
    c.global.workdays = [true, false, true, false, true, false, true];
    assert_eq!(c.calendar().workdays, [true, false, true, false, true, false, true]);
}

#[test]
fn calendar_turns_global_hours_into_the_default_target_in_seconds() {
    let mut c = config();
    c.global.hours_per_day = 7;
    assert_eq!(c.calendar().default_target, 7 * 3600);
}

#[test]
fn calendar_merges_the_holidays_of_every_configured_year() {
    let mut c = config();
    c.years.insert(2025, YearSettings { hours_per_day: None, holidays: vec![holiday(2025, 12, 25, "Christmas")] });
    c.years.insert(
        2026,
        YearSettings { hours_per_day: None, holidays: vec![holiday(2026, 1, 1, "New Year"), holiday(2026, 4, 13, "Songkran")] },
    );
    let cal = c.calendar();
    assert_eq!(cal.holidays.len(), 3, "one map across all years");
    assert_eq!(cal.holiday(d(2025, 12, 25)), Some("Christmas"));
    assert_eq!(cal.holiday(d(2026, 1, 1)), Some("New Year"));
    assert_eq!(cal.holiday(d(2026, 4, 13)), Some("Songkran"));
}

#[test]
fn calendar_turns_a_year_hours_override_into_seconds() {
    let mut c = config();
    c.years.insert(2026, YearSettings { hours_per_day: Some(6), holidays: Vec::new() });
    assert_eq!(c.calendar().year_target.get(&2026).copied(), Some(6 * 3600));
}

#[test]
fn calendar_records_no_year_target_for_a_year_that_only_has_holidays() {
    let mut c = config();
    c.years.insert(2026, YearSettings { hours_per_day: None, holidays: vec![holiday(2026, 1, 1, "New Year")] });
    let cal = c.calendar();
    assert!(cal.year_target.is_empty(), "no override → the global default applies");
    assert_eq!(cal.target_for(d(2026, 1, 2)), 8 * 3600);
}

#[test]
fn year_returns_the_defaults_for_an_unconfigured_year() {
    let c = config();
    assert_eq!(c.year(1999), YearSettings::default());
}

#[test]
fn year_returns_the_stored_settings_for_a_configured_year() {
    let mut c = config();
    let ys = YearSettings { hours_per_day: Some(6), holidays: vec![holiday(2026, 1, 1, "New Year")] };
    c.years.insert(2026, ys.clone());
    assert_eq!(c.year(2026), ys);
}

#[test]
fn jql_falls_back_to_the_work_for_me_default() {
    assert_eq!(config().jql(), DEFAULT_JQL);
}

#[test]
fn jql_uses_the_file_override_when_present() {
    let mut c = config();
    c.jql = Some("project = KAN".into());
    assert_eq!(c.jql(), "project = KAN");
}

#[test]
fn year_settings_is_empty_only_without_hours_and_holidays() {
    assert!(YearSettings::default().is_empty());
    assert!(!YearSettings { hours_per_day: Some(6), holidays: Vec::new() }.is_empty());
    assert!(!YearSettings { hours_per_day: None, holidays: vec![holiday(2026, 1, 1, "New Year")] }.is_empty());
}

#[test]
fn with_defaults_starts_from_the_global_defaults_and_no_years() {
    let c = config();
    assert_eq!(c.global, GlobalSettings::default());
    assert_eq!(c.global.default_start, StartTime::NINE);
    assert!(c.years.is_empty());
}
