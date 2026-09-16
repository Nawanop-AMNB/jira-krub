use super::push::{Counts, Op, Outcome, PushItem, counts, plan, push_one};
use crate::application::test_support::{FakeGateway, key};
use crate::domain::{Entry, EntryId, EntryState, Intent, StartTime};
use chrono::{Local, NaiveDate, TimeZone};

fn entry(id: &str, title: &str, state: EntryState) -> Entry {
    Entry {
        id: EntryId::new(id.into()),
        issue_key: key("A-1"),
        date: NaiveDate::from_ymd_opt(2026, 9, 16).unwrap(),
        start: StartTime::parse("13:30").unwrap(),
        seconds: 3600,
        title: title.into(),
        detail: String::new(),
        state,
    }
}

fn failed(intent: Intent) -> EntryState {
    EntryState::Failed { error: "boom".into(), intent }
}

fn plan_all(entries: &[Entry], allow_untitled: bool) -> (Vec<PushItem>, Vec<EntryId>) {
    let refs: Vec<&Entry> = entries.iter().collect();
    plan(&refs, allow_untitled)
}

fn ids(v: &[EntryId]) -> Vec<&str> {
    v.iter().map(EntryId::as_str).collect()
}

// ---- plan -----------------------------------------------------------------

#[test]
fn staged_entry_plans_a_create() {
    let (items, untitled) = plan_all(&[entry("1", "t", EntryState::Staged)], false);
    assert!(untitled.is_empty());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id.as_str(), "1");
    assert!(matches!(&items[0].op, Op::Create(r) if r.issue_key == key("A-1") && r.seconds == 3600));
}

#[test]
fn modified_entry_plans_an_update_with_its_worklog_id() {
    let (items, _) = plan_all(&[entry("1", "t", EntryState::Modified { worklog_id: "77".into() })], false);
    assert!(matches!(&items[0].op, Op::Update { worklog_id, request } if worklog_id == "77" && request.seconds == 3600));
}

#[test]
fn deleted_entry_plans_a_delete() {
    let (items, _) = plan_all(&[entry("1", "t", EntryState::Deleted { worklog_id: "77".into() })], false);
    assert!(matches!(&items[0].op, Op::Delete { issue_key, worklog_id } if *issue_key == key("A-1") && worklog_id == "77"));
}

#[test]
fn failed_entry_retries_its_intent() {
    let entries = [
        entry("c", "t", failed(Intent::Create)),
        entry("u", "t", failed(Intent::Update { worklog_id: "5".into() })),
        entry("d", "t", failed(Intent::Delete { worklog_id: "6".into() })),
    ];
    let (items, _) = plan_all(&entries, false);
    assert_eq!(items.len(), 3);
    assert!(matches!(&items[0].op, Op::Create(_)));
    assert!(matches!(&items[1].op, Op::Update { worklog_id, .. } if worklog_id == "5"));
    assert!(matches!(&items[2].op, Op::Delete { worklog_id, .. } if worklog_id == "6"));
}

#[test]
fn pushed_entry_is_skipped() {
    let (items, untitled) = plan_all(&[entry("1", "t", EntryState::Pushed { worklog_id: "77".into() })], false);
    assert!(items.is_empty());
    assert!(untitled.is_empty());
}

#[test]
fn untitled_entries_are_skipped_and_reported_when_not_allowed() {
    let entries = [entry("titled", "t", EntryState::Staged), entry("blank", "   ", EntryState::Staged)];
    let (items, untitled) = plan_all(&entries, false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id.as_str(), "titled");
    assert_eq!(ids(&untitled), vec!["blank"]);
}

#[test]
fn untitled_entries_are_included_when_allowed() {
    let (items, untitled) = plan_all(&[entry("blank", "", EntryState::Staged)], true);
    assert_eq!(items.len(), 1);
    assert!(untitled.is_empty());
}

#[test]
fn untitled_delete_is_never_skipped() {
    let (items, untitled) = plan_all(&[entry("blank", "", EntryState::Deleted { worklog_id: "9".into() })], false);
    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0].op, Op::Delete { .. }));
    assert!(untitled.is_empty());
}

#[test]
fn started_is_entry_date_and_start_in_local_timezone() {
    let (items, _) = plan_all(&[entry("1", "t", EntryState::Staged)], false);
    let Op::Create(r) = &items[0].op else { panic!("expected create") };
    let expected = Local.with_ymd_and_hms(2026, 9, 16, 13, 30, 0).single().unwrap();
    assert_eq!(r.started, expected);
}

#[test]
fn comment_paragraphs_are_title_then_detail() {
    let mut e = entry("1", "title", EntryState::Staged);
    e.detail = "detail".into();
    let (items, _) = plan_all(&[e], false);
    let Op::Create(r) = &items[0].op else { panic!("expected create") };
    assert_eq!(r.comment_paragraphs, vec!["title".to_string(), "detail".to_string()]);
}

#[test]
fn comment_paragraphs_are_empty_when_title_and_detail_are_blank() {
    let (items, _) = plan_all(&[entry("1", " ", EntryState::Staged)], true);
    let Op::Create(r) = &items[0].op else { panic!("expected create") };
    assert!(r.comment_paragraphs.is_empty());
}

// ---- counts ---------------------------------------------------------------

#[test]
fn counts_sum_seconds_for_create_and_update_only() {
    let mut deleted = entry("d", "t", EntryState::Deleted { worklog_id: "1".into() });
    deleted.seconds = 99_999;
    let mut modified = entry("m", "t", EntryState::Modified { worklog_id: "2".into() });
    modified.seconds = 1800;
    let entries = [entry("c", "t", EntryState::Staged), modified, deleted];
    let (items, _) = plan_all(&entries, false);
    assert_eq!(counts(&items), Counts { create: 1, update: 1, delete: 1, seconds: 3600 + 1800 });
}

// ---- push_one -------------------------------------------------------------

fn single_item(e: Entry) -> PushItem {
    plan_all(&[e], false).0.remove(0)
}

#[test]
fn push_one_create_calls_add_worklog_and_yields_saved() {
    let gw = FakeGateway::default();
    let item = single_item(entry("1", "t", EntryState::Staged));
    let out = push_one(&gw, &item).unwrap();
    assert!(matches!(&out, Outcome::Saved(w) if w.id == "created" && w.seconds == 3600));
    let calls = gw.calls();
    assert_eq!(calls.added.len(), 1);
    assert!(calls.updated.is_empty());
    assert!(calls.deleted.is_empty());
}

#[test]
fn push_one_update_calls_update_worklog_with_the_id() {
    let gw = FakeGateway::default();
    let item = single_item(entry("1", "t", EntryState::Modified { worklog_id: "77".into() }));
    let out = push_one(&gw, &item).unwrap();
    assert!(matches!(&out, Outcome::Saved(w) if w.id == "77"));
    let calls = gw.calls();
    assert_eq!(calls.updated.len(), 1);
    assert_eq!(calls.updated[0].0, "77");
    assert!(calls.added.is_empty());
}

#[test]
fn push_one_delete_calls_delete_worklog_and_yields_deleted() {
    let gw = FakeGateway::default();
    let item = single_item(entry("1", "t", EntryState::Deleted { worklog_id: "77".into() }));
    let out = push_one(&gw, &item).unwrap();
    assert!(matches!(&out, Outcome::Deleted { worklog_id } if worklog_id == "77"));
    assert_eq!(gw.calls().deleted, vec![(key("A-1"), "77".to_string())]);
}
