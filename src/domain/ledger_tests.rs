use super::{Entry, EntryId, EntryState, Intent, IssueKey, Ledger, RemoteWorklog, StartTime};
use chrono::{Local, NaiveDate, TimeZone};

fn key(s: &str) -> IssueKey {
    IssueKey::parse(s).unwrap()
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn id(s: &str) -> EntryId {
    EntryId::new(s.into())
}

fn entry(i: &str, date: NaiveDate, state: EntryState) -> Entry {
    Entry {
        id: id(i),
        issue_key: key("A-1"),
        date,
        start: StartTime::NINE,
        seconds: 3600,
        title: "t".into(),
        state,
    }
}

fn failed(intent: Intent) -> EntryState {
    EntryState::Failed { error: "boom".into(), intent }
}

/// A remote worklog started at 13:30 local time on 2026-09-16.
fn remote(worklog_id: &str) -> RemoteWorklog {
    RemoteWorklog {
        id: worklog_id.into(),
        issue_key: key("OPS-7"),
        started: Local.with_ymd_and_hms(2026, 9, 16, 13, 30, 0).single().unwrap().fixed_offset(),
        seconds: 5400,
        comment: "standup".into(),
    }
}

#[test]
fn adopt_remote_creates_a_pushed_entry_from_the_worklog() {
    let mut l = Ledger::default();
    let got = l.adopt_remote(id("local-1"), &remote("r1"));
    assert_eq!(got, id("local-1"));
    let e = l.entry(&id("local-1")).unwrap();
    assert_eq!(e.issue_key, key("OPS-7"));
    assert_eq!(e.date, d(2026, 9, 16));
    assert_eq!(e.start, StartTime::parse("13:30").unwrap());
    assert_eq!(e.seconds, 5400);
    assert_eq!(e.title, "standup");
    assert_eq!(e.state, EntryState::Pushed { worklog_id: "r1".into() });
    assert_eq!(l.staged_count(), 0);
}

#[test]
fn adopt_remote_returns_the_existing_id_when_already_adopted() {
    let mut l = Ledger::default();
    l.adopt_remote(id("first"), &remote("r1"));
    l.edit(&id("first"), |e| e.seconds = 60);
    assert_eq!(l.adopt_remote(id("second"), &remote("r1")), id("first"));
    assert_eq!(l.entries().len(), 1);
    assert!(l.entry(&id("second")).is_none());
}

#[test]
fn staged_in_range_is_inclusive_on_both_ends() {
    let l = Ledger::new(
        vec![
            entry("before", d(2026, 9, 13), EntryState::Staged),
            entry("from", d(2026, 9, 14), EntryState::Staged),
            entry("mid", d(2026, 9, 16), EntryState::Staged),
            entry("to", d(2026, 9, 20), EntryState::Staged),
            entry("after", d(2026, 9, 21), EntryState::Staged),
        ],
        vec![],
    );
    let ids: Vec<&str> = l.staged_in(d(2026, 9, 14), d(2026, 9, 20)).iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["from", "mid", "to"]);
}

#[test]
fn staged_count_counts_everything_but_pushed() {
    let day = d(2026, 9, 16);
    let l = Ledger::new(
        vec![
            entry("pushed", day, EntryState::Pushed { worklog_id: "1".into() }),
            entry("staged", day, EntryState::Staged),
            entry("modified", day, EntryState::Modified { worklog_id: "2".into() }),
            entry("deleted", day, EntryState::Deleted { worklog_id: "3".into() }),
            entry("failed", day, failed(Intent::Create)),
        ],
        vec![],
    );
    assert_eq!(l.staged_count(), 4);
}

#[test]
fn edit_keeps_a_staged_entry_staged() {
    let mut l = Ledger::new(vec![entry("s", d(2026, 9, 16), EntryState::Staged)], vec![]);
    assert!(l.edit(&id("s"), |e| e.seconds = 900));
    let e = l.entry(&id("s")).unwrap();
    assert_eq!(e.seconds, 900);
    assert_eq!(e.state, EntryState::Staged);
}

#[test]
fn edit_keeps_a_failed_entry_failed_with_its_intent() {
    let state = failed(Intent::Update { worklog_id: "7".into() });
    let mut l = Ledger::new(vec![entry("f", d(2026, 9, 16), state.clone())], vec![]);
    assert!(l.edit(&id("f"), |e| e.title = "renamed".into()));
    let e = l.entry(&id("f")).unwrap();
    assert_eq!(e.title, "renamed");
    assert_eq!(e.state, state);
}
