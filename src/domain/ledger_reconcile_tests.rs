//! `Ledger::reconcile`: Jira is the source of truth for `Pushed` rows on
//! issues the sync actually covered; everything else is left alone.

use super::{Entry, EntryId, EntryState, Intent, IssueKey, Ledger, RemoteWorklog, StartTime};
use chrono::{Local, NaiveDate, TimeZone};

fn key(s: &str) -> IssueKey {
    IssueKey::parse(s).unwrap()
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn id(s: &str) -> EntryId {
    EntryId::new(s.to_string())
}

fn entry(eid: &str, issue: &str, state: EntryState) -> Entry {
    Entry {
        id: id(eid),
        issue_key: key(issue),
        date: d("2026-09-16"),
        start: StartTime::NINE,
        seconds: 3600,
        title: "local text".into(),
        state,
    }
}

/// A worklog Jira reports at 13:30 local time on 18 Sep 2026.
fn remote_at_1330(worklog_id: &str, issue: &str) -> RemoteWorklog {
    RemoteWorklog {
        id: worklog_id.into(),
        issue_key: key(issue),
        started: Local.with_ymd_and_hms(2026, 9, 18, 13, 30, 0).unwrap().fixed_offset(),
        seconds: 5400,
        comment: "changed in jira".into(),
    }
}

#[test]
fn a_pushed_row_takes_the_remote_date_start_seconds_and_comment() {
    let mut l = Ledger::default();
    l.add(entry("e1", "KAN-1", EntryState::Pushed { worklog_id: "w1".into() }));
    l.reconcile(&[remote_at_1330("w1", "KAN-1")], &[key("KAN-1")]);
    let e = l.entry(&id("e1")).expect("row kept");
    assert_eq!(e.date, d("2026-09-18"));
    assert_eq!(e.start, StartTime::from_hm(13, 30).unwrap());
    assert_eq!(e.seconds, 5400);
    assert_eq!(e.title, "changed in jira");
}

#[test]
fn a_pushed_row_on_an_uncovered_issue_survives_even_when_remote_has_nothing() {
    let mut l = Ledger::default();
    l.add(entry("e1", "OPS-7", EntryState::Pushed { worklog_id: "w9".into() }));
    // The sync only fetched KAN-1, so OPS-7 rows must not be judged by it.
    l.reconcile(&[], &[key("KAN-1")]);
    let e = l.entry(&id("e1")).expect("uncovered row kept");
    assert_eq!((e.seconds, e.title.as_str(), e.date), (3600, "local text", d("2026-09-16")));
}

#[test]
fn a_modified_row_is_left_alone() {
    let mut l = Ledger::default();
    l.add(entry("e1", "KAN-1", EntryState::Modified { worklog_id: "w1".into() }));
    l.reconcile(&[remote_at_1330("w1", "KAN-1")], &[key("KAN-1")]);
    let e = l.entry(&id("e1")).expect("row kept");
    assert_eq!((e.seconds, e.title.as_str()), (3600, "local text"));
    assert!(e.is_modified());
}

#[test]
fn a_row_marked_for_deletion_is_left_alone() {
    let mut l = Ledger::default();
    l.add(entry("e1", "KAN-1", EntryState::Deleted { worklog_id: "w1".into() }));
    l.reconcile(&[remote_at_1330("w1", "KAN-1")], &[key("KAN-1")]);
    let e = l.entry(&id("e1")).expect("row kept");
    assert_eq!(e.seconds, 3600);
    assert!(e.is_deleted());
}

#[test]
fn a_failed_row_is_left_alone() {
    let mut l = Ledger::default();
    let state = EntryState::Failed { error: "boom".into(), intent: Intent::Update { worklog_id: "w1".into() } };
    l.add(entry("e1", "KAN-1", state));
    l.reconcile(&[remote_at_1330("w1", "KAN-1")], &[key("KAN-1")]);
    let e = l.entry(&id("e1")).expect("row kept");
    assert_eq!((e.seconds, e.title.as_str()), (3600, "local text"));
    assert!(e.is_failed());
}

#[test]
fn a_staged_row_is_left_alone() {
    let mut l = Ledger::default();
    l.add(entry("e1", "KAN-1", EntryState::Staged));
    l.reconcile(&[], &[key("KAN-1")]);
    let e = l.entry(&id("e1")).expect("staged rows are never dropped");
    assert_eq!((e.seconds, e.title.as_str()), (3600, "local text"));
}
