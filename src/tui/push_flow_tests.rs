//! Batch push (R6) and what a finished sync does to the ledger (R7), driven
//! through the real `App` with a fake gateway.

use super::action::{Action, PushScope};
use super::app::{App, Screen};
use super::msg::{Msg, SyncError};
use super::test_support::{Harness, harness, harness_with_config, script_with_issues};
use crate::application::ports::{GatewayErrorKind, Window};
use crate::application::test_support::{Script, gateway_error, issue, key, remote};
use crate::application::use_cases::sync::SyncOutput;
use crate::domain::{Entry, EntryId, EntryState, IssueKey, StartTime, Week};
use chrono::{Datelike, Days, NaiveDate};
use std::time::Duration;

/// Drain the worker until the running push has reported `PushDone`.
fn settle_push(app: &mut App) {
    for _ in 0..200 {
        for m in app.worker.drain() {
            app.on_msg(m);
        }
        if app.push.is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("push did not finish within 1s");
}

fn add(app: &mut App, id: &str, issue_key: &str, date: NaiveDate, title: &str, state: EntryState) -> EntryId {
    let eid = EntryId::new(id.to_string());
    app.ledger.add(Entry {
        id: eid.clone(),
        issue_key: key(issue_key),
        date,
        start: StartTime::parse("09:00").unwrap(),
        seconds: 3600,
        title: title.to_string(),
        state,
    });
    eid
}

/// Answer the confirm prompt with `y` and run the push to completion.
fn confirm_and_push(app: &mut App) {
    assert!(app.overlay.is_some(), "a push always confirms first");
    app.dispatch(Action::ConfirmYes);
    settle_push(app);
}

fn pushed_keys(h: &Harness) -> Vec<String> {
    h.gateway.calls().added.iter().map(|w| w.issue_key.to_string()).collect()
}

// ---- R6: scope ------------------------------------------------------------

#[test]
fn pushing_a_day_sends_only_that_day() {
    let mut h = harness("push-day-scope", script_with_issues(vec![issue("KAN-1")]));
    let today = h.app.today;
    add(&mut h.app, "today", "KAN-1", today, "t", EntryState::Staged);
    add(&mut h.app, "yesterday", "KAN-2", today - Days::new(1), "t", EntryState::Staged);
    add(&mut h.app, "tomorrow", "KAN-3", today + Days::new(1), "t", EntryState::Staged);

    h.app.dispatch(Action::PushRequest(PushScope::Day(today)));
    confirm_and_push(&mut h.app);

    assert_eq!(pushed_keys(&h), vec!["KAN-1".to_string()], "neighbouring days must stay staged");
    assert!(h.app.ledger.entry(&EntryId::new("yesterday".into())).unwrap().needs_push());
    assert!(h.app.ledger.entry(&EntryId::new("tomorrow".into())).unwrap().needs_push());
}

#[test]
fn pushing_a_week_sends_every_day_in_it_and_nothing_outside() {
    let mut h = harness("push-week-scope", script_with_issues(vec![issue("KAN-1")]));
    let week = Week::containing(h.app.today);
    add(&mut h.app, "mon", "KAN-1", week.monday(), "t", EntryState::Staged);
    add(&mut h.app, "sun", "KAN-2", week.sunday(), "t", EntryState::Staged);
    add(&mut h.app, "next-mon", "KAN-3", week.sunday() + Days::new(1), "t", EntryState::Staged);

    h.app.dispatch(Action::PushRequest(PushScope::Week(week)));
    confirm_and_push(&mut h.app);

    let mut keys = pushed_keys(&h);
    keys.sort();
    assert_eq!(keys, vec!["KAN-1".to_string(), "KAN-2".to_string()]);
}

// ---- R6: title is optional ------------------------------------------------

#[test]
fn an_entry_without_a_description_is_still_pushed_without_a_comment() {
    let mut h = harness("push-untitled", script_with_issues(vec![issue("KAN-1")]));
    let today = h.app.today;
    let id = add(&mut h.app, "blank", "KAN-1", today, "   ", EntryState::Staged);

    h.app.dispatch(Action::PushRequest(PushScope::Day(today)));
    confirm_and_push(&mut h.app);

    assert_eq!(pushed_keys(&h), vec!["KAN-1".to_string()], "a blank title must not silently skip the entry");
    assert!(h.gateway.calls().added[0].comment_paragraphs.is_empty(), "no comment is sent");
    assert!(h.app.ledger.entry(&id).unwrap().is_pushed());
}

// ---- R6: failures ---------------------------------------------------------

#[test]
fn a_rejected_entry_becomes_failed_and_only_it_is_retried() {
    let script = Script {
        searches: [Ok(vec![issue("KAN-1")]), Ok(Vec::new())].into(),
        add_result: Some(Err(gateway_error(GatewayErrorKind::Other, "add worklog KAN-9: HTTP 404 issue does not exist"))),
        ..Script::default()
    };
    let mut h = harness("push-failure", script);
    let today = h.app.today;
    let bad = add(&mut h.app, "bad", "KAN-9", today, "t", EntryState::Staged);

    h.app.dispatch(Action::PushRequest(PushScope::Day(today)));
    confirm_and_push(&mut h.app);

    let e = h.app.ledger.entry(&bad).expect("the entry is kept so it can be retried");
    assert!(e.is_failed(), "state is {:?}", e.state);
    match &e.state {
        EntryState::Failed { error, .. } => assert!(error.contains("404"), "the HTTP status is visible: {error}"),
        other => panic!("expected failed, got {other:?}"),
    }
    assert!(h.app.push.is_none(), "the push is finished");

    // A second push re-sends exactly the one that failed.
    let before = h.gateway.calls().added.len();
    h.app.dispatch(Action::PushRequest(PushScope::Day(today)));
    confirm_and_push(&mut h.app);
    assert_eq!(h.gateway.calls().added.len(), before + 1);
}

// ---- R6 / R13: delete --------------------------------------------------------

#[test]
fn a_pushed_delete_drops_the_row_and_the_cached_worklog() {
    let mut h = harness("push-delete", script_with_issues(vec![issue("KAN-1")]));
    let today = h.app.today;
    let id = add(&mut h.app, "gone", "KAN-1", today, "t", EntryState::Deleted { worklog_id: "w-1".into() });
    h.app.remote.worklogs.push(remote("w-1", "KAN-1", (today.year(), today.month(), today.day()), 3600));

    h.app.dispatch(Action::PushRequest(PushScope::Day(today)));
    confirm_and_push(&mut h.app);

    assert_eq!(h.gateway.calls().deleted, vec![(key("KAN-1"), "w-1".to_string())]);
    assert!(h.app.ledger.entry(&id).is_none(), "the local row goes away");
    assert!(
        !h.app.remote.worklogs.iter().any(|w| w.id == "w-1"),
        "the cached Jira row goes too, or the day keeps counting deleted time"
    );
}

// ---- R7: what a finished sync does ---------------------------------------

fn sync_output(mine: Vec<crate::domain::Issue>, worklogs: Vec<crate::domain::RemoteWorklog>, window: Window) -> SyncOutput {
    SyncOutput {
        account_id: "acc-1".into(),
        display_name: "Me".into(),
        mine,
        watched: Vec::new(),
        history: Vec::new(),
        worklogs,
        window,
    }
}

#[test]
fn a_sync_drops_a_pushed_row_jira_no_longer_has() {
    let mut h = harness("sync-reconciles", script_with_issues(vec![issue("KAN-1")]));
    let today = h.app.today;
    let gone = add(&mut h.app, "gone", "KAN-1", today, "t", EntryState::Pushed { worklog_id: "w-gone".into() });
    let staged = add(&mut h.app, "staged", "KAN-1", today, "t", EntryState::Staged);
    let window = Window { from: today - Days::new(7), to: today + Days::new(7) };

    h.app.on_msg(Msg::Synced(Ok(sync_output(vec![issue("KAN-1")], Vec::new(), window))));

    assert!(h.app.ledger.entry(&gone).is_none(), "Jira is the source of truth for pushed rows");
    assert!(h.app.ledger.entry(&staged).is_some(), "sync never deletes pending entries");
}

#[test]
fn a_sync_copies_the_remote_values_onto_a_pushed_row() {
    let mut h = harness("sync-takes-remote", script_with_issues(vec![issue("KAN-1")]));
    let today = h.app.today;
    let id = add(&mut h.app, "e1", "KAN-1", today, "local text", EntryState::Pushed { worklog_id: "w-1".into() });
    let window = Window { from: today - Days::new(7), to: today + Days::new(7) };
    let mut w = remote("w-1", "KAN-1", (2026, 9, 16), 5400);
    w.comment = "changed in jira".into();

    h.app.on_msg(Msg::Synced(Ok(sync_output(vec![issue("KAN-1")], vec![w], window))));

    let e = h.app.ledger.entry(&id).expect("row kept");
    assert_eq!((e.seconds, e.title.as_str()), (5400, "changed in jira"));
}

#[test]
fn a_401_during_sync_reopens_the_connect_screen() {
    let mut h = harness("sync-401", script_with_issues(Vec::new()));
    h.app.on_msg(Msg::Synced(Err(SyncError { kind: GatewayErrorKind::Unauthorized, message: "HTTP 401".into() })));
    match &h.app.screen {
        Screen::Connect(_) => {}
        _ => panic!("a rejected token must send the user back to Connect"),
    }
}

#[test]
fn a_network_failure_during_sync_stays_on_the_main_screen_in_offline_mode() {
    let mut h = harness("sync-offline", script_with_issues(Vec::new()));
    h.app.on_msg(Msg::Synced(Err(SyncError { kind: GatewayErrorKind::Network, message: "connection refused".into() })));
    assert!(matches!(h.app.screen, Screen::Week(_)), "a dropped connection is not an auth problem");
    assert!(h.app.remote.offline, "the UI says offline and keeps the last state");
}

// ---- R7 / R16: the lookback setting widens the fetched window -------------

/// The `worklogDate >=` bound of the history JQL from the startup sync.
fn history_from(h: &Harness) -> NaiveDate {
    let calls = h.gateway.calls();
    let jql = calls.jql.iter().find(|j| j.contains("worklogAuthor")).expect("a history search ran");
    let after = jql.split("worklogDate >= ").nth(1).expect("a lower bound");
    NaiveDate::parse_from_str(&after[..10], "%Y-%m-%d").expect("an ISO date")
}

fn config_with_lookback(weeks: u32) -> String {
    super::test_support::CONFIG_TOML.replace("lookback_weeks = 3", &format!("lookback_weeks = {weeks}"))
}

#[test]
fn more_lookback_weeks_fetch_more_history() {
    let short = harness_with_config("lookback-1", script_with_issues(Vec::new()), &config_with_lookback(1));
    let long = harness_with_config("lookback-5", script_with_issues(Vec::new()), &config_with_lookback(5));
    let gap = (history_from(&short) - history_from(&long)).num_days();
    assert_eq!(gap, 4 * 7, "four extra lookback weeks must reach four weeks further back");
}

#[test]
fn the_sync_window_reaches_at_least_the_configured_lookback_back() {
    let h = harness_with_config("lookback-3", script_with_issues(Vec::new()), &config_with_lookback(3));
    let monday = Week::containing(h.app.today).monday();
    assert!(
        history_from(&h) <= monday - Days::new(3 * 7),
        "three weeks of history is the point of the setting: {} vs {monday}",
        history_from(&h)
    );
}

#[test]
fn the_watchlist_is_sent_with_the_sync() {
    let mut script = script_with_issues(Vec::new());
    script.issues = [("OPS-7".to_string(), issue("OPS-7"))].into();
    let mut h = harness("sync-watchlist", script);
    h.app.ledger.watch(&key("OPS-7"));
    h.app.remote.window = None;
    h.app.start_sync();
    super::test_support::settle(&mut h.app);
    let fetched: Vec<IssueKey> = h.gateway.calls().get_issue.clone();
    assert!(fetched.contains(&key("OPS-7")), "watched issues are fetched every sync: {fetched:?}");
}
