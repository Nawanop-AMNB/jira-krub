//! `state.json`: every entry state survives a restart, and a pre-0.3 file's
//! second paragraph (`detail`) is folded back into the description (R3).

use super::json_state::JsonStateStore;
use crate::application::StateStore;
use crate::domain::{Entry, EntryId, EntryState, Intent, IssueKey, Ledger, StartTime};
use chrono::NaiveDate;
use std::path::PathBuf;

/// Unique temp directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("jira-krub-state-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn entry(id: &str, state: EntryState) -> Entry {
    Entry {
        id: EntryId::new(id.into()),
        issue_key: IssueKey::parse("X-1").unwrap(),
        date: NaiveDate::from_ymd_opt(2026, 9, 16).unwrap(),
        start: StartTime::parse("09:00").unwrap(),
        seconds: 3600,
        title: "t".into(),
        state,
    }
}

/// Save one entry per state, load it back, and report each state as saved.
fn roundtrip_states(name: &str, states: Vec<(&str, EntryState)>) -> Vec<(String, EntryState)> {
    let dir = TempDir::new(name);
    let store = JsonStateStore::at(dir.file("state.json"));
    let mut l = Ledger::default();
    for (id, s) in &states {
        l.add(entry(id, s.clone()));
    }
    store.save(&l).unwrap();
    let back = store.load().unwrap();
    back.entries().iter().map(|e| (e.id.as_str().to_string(), e.state.clone())).collect()
}

#[test]
fn every_entry_state_survives_a_restart() {
    let states = vec![
        ("staged", EntryState::Staged),
        ("pushed", EntryState::Pushed { worklog_id: "w1".into() }),
        ("modified", EntryState::Modified { worklog_id: "w2".into() }),
        ("deleted", EntryState::Deleted { worklog_id: "w3".into() }),
        ("failed-create", EntryState::Failed { error: "boom".into(), intent: Intent::Create }),
        ("failed-update", EntryState::Failed { error: "boom".into(), intent: Intent::Update { worklog_id: "w4".into() } }),
        ("failed-delete", EntryState::Failed { error: "boom".into(), intent: Intent::Delete { worklog_id: "w5".into() } }),
    ];
    let got = roundtrip_states("all-states", states.clone());
    let want: Vec<(String, EntryState)> = states.into_iter().map(|(id, s)| (id.to_string(), s)).collect();
    assert_eq!(got, want, "a restart must not change what the next push will do");
}

#[test]
fn a_row_marked_for_deletion_is_still_a_delete_after_a_restart() {
    // Regression guard: persisting `deleted` as `modified` would turn the
    // next push into a PUT that silently keeps the worklog in Jira.
    let got = roundtrip_states("deleted-state", vec![("d", EntryState::Deleted { worklog_id: "w3".into() })]);
    assert_eq!(got, vec![("d".to_string(), EntryState::Deleted { worklog_id: "w3".into() })]);
}

#[test]
fn an_edited_row_is_still_pending_after_a_restart() {
    // Loading `modified` as `pushed` would drop the edit without a trace.
    let got = roundtrip_states("modified-state", vec![("m", EntryState::Modified { worklog_id: "w2".into() })]);
    assert_eq!(got, vec![("m".to_string(), EntryState::Modified { worklog_id: "w2".into() })]);
}

#[test]
fn a_legacy_detail_field_becomes_the_second_description_line() {
    let dir = TempDir::new("legacy-detail");
    let path = dir.file("state.json");
    std::fs::write(
        &path,
        r#"{
          "version": 1,
          "entries": [
            { "id": "a", "issue_key": "X-1", "date": "2026-09-16", "start": "09:00",
              "seconds": 3600, "title": "fix expiry", "detail": "bumped the timeout",
              "state": "staged" }
          ],
          "watchlist": []
        }"#,
    )
    .unwrap();
    let l = JsonStateStore::at(path).load().unwrap();
    let e = l.entries().first().expect("the legacy row loads");
    assert_eq!(e.title, "fix expiry\nbumped the timeout", "the old second paragraph is not lost");
    assert_eq!(e.comment_paragraphs(), vec!["fix expiry".to_string(), "bumped the timeout".to_string()]);
}

#[test]
fn a_legacy_row_without_detail_keeps_its_title_unchanged() {
    let dir = TempDir::new("legacy-no-detail");
    let path = dir.file("state.json");
    std::fs::write(
        &path,
        r#"{ "version": 1, "entries": [ { "id": "a", "issue_key": "X-1", "date": "2026-09-16",
             "start": "09:00", "seconds": 3600, "title": "fix expiry", "state": "staged" } ],
             "watchlist": [] }"#,
    )
    .unwrap();
    let l = JsonStateStore::at(path).load().unwrap();
    assert_eq!(l.entries()[0].title, "fix expiry");
}
