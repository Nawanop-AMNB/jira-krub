use super::{Entry, EntryId, EntryState, Intent, IssueKey, StartTime};
use chrono::NaiveDate;

fn entry(id: &str, state: EntryState) -> Entry {
    Entry {
        id: EntryId::new(id.into()),
        issue_key: IssueKey::parse("A-1").unwrap(),
        date: NaiveDate::from_ymd_opt(2026, 9, 16).unwrap(),
        start: StartTime::NINE,
        seconds: 3600,
        title: "t".into(),
        detail: String::new(),
        state,
    }
}

fn failed(intent: Intent) -> EntryState {
    EntryState::Failed { error: "boom".into(), intent }
}

fn at(id: &str, start: &str, seconds: u64) -> Entry {
    let mut e = entry(id, EntryState::Staged);
    e.start = StartTime::parse(start).unwrap();
    e.seconds = seconds;
    e
}

#[test]
fn intent_for_every_state() {
    let update = Intent::Update { worklog_id: "5".into() };
    let delete = Intent::Delete { worklog_id: "6".into() };
    assert_eq!(entry("a", EntryState::Staged).intent(), Some(Intent::Create));
    assert_eq!(entry("a", EntryState::Pushed { worklog_id: "1".into() }).intent(), None);
    assert_eq!(entry("a", EntryState::Modified { worklog_id: "5".into() }).intent(), Some(update.clone()));
    assert_eq!(entry("a", EntryState::Deleted { worklog_id: "6".into() }).intent(), Some(delete.clone()));
    assert_eq!(entry("a", failed(Intent::Create)).intent(), Some(Intent::Create));
    assert_eq!(entry("a", failed(update.clone())).intent(), Some(update));
    assert_eq!(entry("a", failed(delete.clone())).intent(), Some(delete));
}

#[test]
fn worklog_id_for_every_state() {
    assert_eq!(entry("a", EntryState::Staged).worklog_id(), None);
    assert_eq!(entry("a", EntryState::Pushed { worklog_id: "1".into() }).worklog_id(), Some("1"));
    assert_eq!(entry("a", EntryState::Modified { worklog_id: "2".into() }).worklog_id(), Some("2"));
    assert_eq!(entry("a", EntryState::Deleted { worklog_id: "3".into() }).worklog_id(), Some("3"));
    assert_eq!(entry("a", failed(Intent::Create)).worklog_id(), None);
    assert_eq!(entry("a", failed(Intent::Update { worklog_id: "4".into() })).worklog_id(), Some("4"));
    assert_eq!(entry("a", failed(Intent::Delete { worklog_id: "5".into() })).worklog_id(), Some("5"));
}

#[test]
fn is_deleted_includes_a_failed_delete_only() {
    assert!(entry("a", EntryState::Deleted { worklog_id: "1".into() }).is_deleted());
    assert!(entry("a", failed(Intent::Delete { worklog_id: "1".into() })).is_deleted());
    assert!(!entry("a", failed(Intent::Update { worklog_id: "1".into() })).is_deleted());
    assert!(!entry("a", failed(Intent::Create)).is_deleted());
    assert!(!entry("a", EntryState::Staged).is_deleted());
}

#[test]
fn is_modified_includes_a_failed_update_only() {
    assert!(entry("a", EntryState::Modified { worklog_id: "1".into() }).is_modified());
    assert!(entry("a", failed(Intent::Update { worklog_id: "1".into() })).is_modified());
    assert!(!entry("a", failed(Intent::Delete { worklog_id: "1".into() })).is_modified());
    assert!(!entry("a", failed(Intent::Create)).is_modified());
    assert!(!entry("a", EntryState::Pushed { worklog_id: "1".into() }).is_modified());
}

#[test]
fn end_is_start_plus_seconds() {
    assert_eq!(at("a", "09:00", 5400).end(), StartTime::parse("10:30").unwrap());
}

#[test]
fn overlaps_when_same_day_ranges_intersect() {
    let a = at("a", "09:00", 3600);
    let b = at("b", "09:30", 3600);
    assert!(a.overlaps(&b));
    assert!(b.overlaps(&a));
}

#[test]
fn no_overlap_on_different_days() {
    let a = at("a", "09:00", 3600);
    let mut b = at("b", "09:30", 3600);
    b.date = NaiveDate::from_ymd_opt(2026, 9, 17).unwrap();
    assert!(!a.overlaps(&b));
}

#[test]
fn no_overlap_when_adjacent() {
    let a = at("a", "09:00", 3600);
    let b = at("b", "10:00", 3600);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}

#[test]
fn no_overlap_with_itself() {
    let a = at("a", "09:00", 3600);
    assert!(!a.overlaps(&a.clone()));
}

#[test]
fn no_overlap_when_either_side_is_deleted() {
    let a = at("a", "09:00", 3600);
    let mut b = at("b", "09:30", 3600);
    b.state = EntryState::Deleted { worklog_id: "1".into() };
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
    b.state = failed(Intent::Delete { worklog_id: "1".into() });
    assert!(!a.overlaps(&b));
}

#[test]
fn comment_paragraphs_trims_and_skips_blank_parts() {
    let mut e = entry("a", EntryState::Staged);
    e.title = "  fix expiry  ".into();
    e.detail = "\n details \t".into();
    assert_eq!(e.comment_paragraphs(), vec!["fix expiry".to_string(), "details".to_string()]);
    e.title = "   ".into();
    assert_eq!(e.comment_paragraphs(), vec!["details".to_string()]);
    e.detail = String::new();
    assert!(e.comment_paragraphs().is_empty());
}
