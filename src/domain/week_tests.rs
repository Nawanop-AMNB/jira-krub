use super::{DayStatus, DaySummary, Entry, EntryId, EntryState, Intent, IssueKey, RemoteWorklog, StartTime, WorkCalendar};
use chrono::{Local, NaiveDate, TimeZone};

const TARGET: u64 = 8 * 3600;

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 16).unwrap() // Wednesday
}

fn entry(id: &str, seconds: u64, state: EntryState) -> Entry {
    Entry {
        id: EntryId::new(id.into()),
        issue_key: IssueKey::parse("A-1").unwrap(),
        date: day(),
        start: StartTime::NINE,
        seconds,
        title: "t".into(),
        detail: String::new(),
        state,
    }
}

/// Remote worklog on `day()` at 09:00 local time.
fn remote(id: &str, seconds: u64) -> RemoteWorklog {
    RemoteWorklog {
        id: id.into(),
        issue_key: IssueKey::parse("A-1").unwrap(),
        started: Local.with_ymd_and_hms(2026, 9, 16, 9, 0, 0).single().unwrap().fixed_offset(),
        seconds,
        comment: String::new(),
    }
}

fn summary(entries: &[Entry], remote: &[RemoteWorklog]) -> DaySummary {
    DaySummary::compute(day(), day(), &WorkCalendar::simple(TARGET), entries, remote)
}

#[test]
fn pushed_entry_not_yet_in_remote_counts_as_pushed() {
    let s = summary(&[entry("1", 1800, EntryState::Pushed { worklog_id: "r-new".into() })], &[]);
    assert_eq!(s.pushed_seconds, 1800);
    assert_eq!(s.staged_seconds, 0);
    assert_eq!(s.staged_count, 0);
}

#[test]
fn pushed_entry_already_in_remote_is_not_double_counted() {
    let s = summary(&[entry("1", 1800, EntryState::Pushed { worklog_id: "r1".into() })], &[remote("r1", 1800)]);
    assert_eq!(s.pushed_seconds, 1800);
    assert_eq!(s.staged_count, 0);
}

#[test]
fn modified_entry_replaces_remote_seconds_with_its_own() {
    let s = summary(&[entry("1", 900, EntryState::Modified { worklog_id: "r1".into() })], &[remote("r1", 3600)]);
    assert_eq!(s.pushed_seconds, 0);
    assert_eq!(s.staged_seconds, 900);
    assert_eq!(s.staged_count, 1);
    assert_eq!(s.total(), 900);
}

#[test]
fn deleted_entry_removes_remote_seconds_and_counts_as_staged_with_no_time() {
    let s = summary(
        &[entry("1", 3600, EntryState::Deleted { worklog_id: "r1".into() })],
        &[remote("r1", 3600), remote("r2", 1800)],
    );
    assert_eq!(s.pushed_seconds, 1800);
    assert_eq!(s.staged_seconds, 0);
    assert_eq!(s.staged_count, 1);
}

#[test]
fn failed_create_counts_as_staged() {
    let state = EntryState::Failed { error: "boom".into(), intent: Intent::Create };
    let s = summary(&[entry("1", 2700, state)], &[]);
    assert_eq!(s.pushed_seconds, 0);
    assert_eq!(s.staged_seconds, 2700);
    assert_eq!(s.staged_count, 1);
}

#[test]
fn weekend_status_wins_over_totals() {
    let saturday = NaiveDate::from_ymd_opt(2026, 9, 19).unwrap();
    let mut full = entry("1", TARGET, EntryState::Staged);
    full.date = saturday;
    let s = DaySummary::compute(saturday, day(), &WorkCalendar::simple(TARGET), &[full], &[]);
    assert_eq!(s.total(), TARGET);
    assert_eq!(s.status, DayStatus::Off);
}

#[test]
fn status_is_full_only_when_pushed_equals_target() {
    // half staged + half pushed is NOT full: green means Jira has it
    let s = summary(&[entry("1", TARGET / 2, EntryState::Staged)], &[remote("r1", TARGET / 2)]);
    assert_eq!(s.total(), TARGET);
    assert_eq!(s.status, DayStatus::Short);
    assert_eq!(s.remaining(), TARGET / 2);
    // all pushed → full
    let s = summary(&[], &[remote("r1", TARGET)]);
    assert_eq!(s.status, DayStatus::Full);
    assert_eq!(s.remaining(), 0);
}

#[test]
fn staged_only_day_is_still_empty() {
    let s = summary(&[entry("1", TARGET, EntryState::Staged)], &[]);
    assert_eq!(s.staged_seconds, TARGET);
    assert_eq!(s.status, DayStatus::TodayEmpty);
    assert_eq!(s.remaining(), TARGET);
}
