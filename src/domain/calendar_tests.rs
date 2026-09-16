//! `WorkCalendar`: which days need hours and how many (R16).

use super::calendar::{MON_TO_FRI, WorkCalendar};
use chrono::NaiveDate;

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Mon 14 Sep 2026 … Sun 20 Sep 2026.
const MON: &str = "2026-09-14";
const WED: &str = "2026-09-16";
const FRI: &str = "2026-09-18";
const SAT: &str = "2026-09-19";
const SUN: &str = "2026-09-20";

#[test]
fn simple_enables_monday_to_friday_and_nothing_else() {
    let c = WorkCalendar::simple(8 * 3600);
    assert_eq!(c.workdays, MON_TO_FRI);
    for day in [MON, WED, FRI] {
        assert!(c.is_workday(d(day)), "{day} is a workday");
    }
    for day in [SAT, SUN] {
        assert!(!c.is_workday(d(day)), "{day} is not a workday");
    }
}

#[test]
fn simple_starts_with_no_holidays_and_no_year_overrides() {
    let c = WorkCalendar::simple(8 * 3600);
    assert!(c.holidays.is_empty());
    assert!(c.year_target.is_empty());
    assert_eq!(c.default_target, 8 * 3600);
    assert_eq!(c.holiday(d(MON)), None);
}

#[test]
fn a_holiday_makes_an_enabled_weekday_a_day_off() {
    let mut c = WorkCalendar::simple(8 * 3600);
    assert!(c.is_workday(d(WED)));
    c.holidays.insert(d(WED), "Company day".into());
    assert!(!c.is_workday(d(WED)));
}

#[test]
fn off_reason_prefers_the_holiday_name_over_the_weekday_name() {
    let mut c = WorkCalendar::simple(8 * 3600);
    c.holidays.insert(d(SAT), "New Year".into());
    assert_eq!(c.off_reason(d(SAT)).as_deref(), Some("New Year"), "holiday wins over Saturday");
}

#[test]
fn target_for_is_zero_on_an_off_day() {
    let mut c = WorkCalendar::simple(8 * 3600);
    c.holidays.insert(d(WED), "Company day".into());
    assert_eq!(c.target_for(d(SAT)), 0, "weekend");
    assert_eq!(c.target_for(d(WED)), 0, "holiday");
}

#[test]
fn a_year_override_wins_over_the_default_target() {
    let mut c = WorkCalendar::simple(8 * 3600);
    c.year_target.insert(2026, 7 * 3600);
    assert_eq!(c.target_for(d(MON)), 7 * 3600);
}

#[test]
fn a_year_without_an_override_keeps_the_default_target() {
    let mut c = WorkCalendar::simple(8 * 3600);
    c.year_target.insert(2026, 7 * 3600);
    assert_eq!(c.target_for(d("2027-09-13")), 8 * 3600, "2027 has no override");
}

#[test]
fn target_between_covers_both_ends_and_skips_off_days() {
    let mut c = WorkCalendar::simple(8 * 3600);
    c.holidays.insert(d(WED), "Company day".into());
    // Mon–Sun: 5 weekdays − 1 holiday = 4 × 8h; both ends included.
    assert_eq!(c.target_between(d(MON), d(SUN)), 4 * 8 * 3600);
    assert_eq!(c.target_between(d(MON), d(FRI)), 4 * 8 * 3600, "Fri is the last counted day");
}

#[test]
fn target_between_counts_one_day_when_from_equals_to() {
    let c = WorkCalendar::simple(8 * 3600);
    assert_eq!(c.target_between(d(MON), d(MON)), 8 * 3600);
    assert_eq!(c.target_between(d(SAT), d(SAT)), 0);
}

#[test]
fn target_between_is_zero_when_from_is_after_to() {
    let c = WorkCalendar::simple(8 * 3600);
    assert_eq!(c.target_between(d(FRI), d(MON)), 0);
}
