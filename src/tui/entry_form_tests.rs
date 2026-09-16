//! The log-work popup (R5): what Save actually stages, and what it refuses.

use super::action::Action;
use super::app::{App, Overlay};
use super::features::{entry_form, week};
use super::test_support::{Harness, harness, script_with_issues};
use crate::application::test_support::{issue, key};
use crate::domain::StartTime;

fn form_action(app: &mut App, a: entry_form::Action) {
    app.dispatch(Action::Form(a));
}

fn form_model(app: &App) -> &entry_form::Model {
    match &app.overlay {
        Some(Overlay::Form(m)) => m,
        _ => panic!("no entry form open"),
    }
}

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        form_action(app, entry_form::Action::Char(c));
    }
}

/// Open the popup and walk to the duration field with `KAN-1` chosen.
fn form_on_duration(name: &str) -> Harness {
    let mut h = harness(name, script_with_issues(vec![issue("KAN-1")]));
    h.app.dispatch(Action::Week(week::Action::AddEntry));
    typed(&mut h.app, "KAN-1");
    for _ in 0..3 {
        form_action(&mut h.app, entry_form::Action::Commit(1));
    }
    assert_eq!(format!("{:?}", form_model(&h.app).focus), "Duration");
    h
}

#[test]
fn the_start_time_stepped_in_the_popup_is_the_one_that_is_staged() {
    let mut h = harness("form-start-kept", script_with_issues(vec![issue("KAN-1")]));
    h.app.dispatch(Action::Week(week::Action::AddEntry));
    typed(&mut h.app, "KAN-1");
    form_action(&mut h.app, entry_form::Action::Commit(1)); // Issue → Date
    form_action(&mut h.app, entry_form::Action::Commit(1)); // Date → Start
    assert_eq!(form_model(&h.app).start.text(), "13:00", "default_start_time from config");

    form_action(&mut h.app, entry_form::Action::StartStep(15));
    form_action(&mut h.app, entry_form::Action::Commit(1)); // Start → Duration
    typed(&mut h.app, "2h");
    form_action(&mut h.app, entry_form::Action::Save);

    let e = h.app.ledger.entries().first().expect("one staged entry");
    assert_eq!(e.start, StartTime::parse("13:15").unwrap(), "the typed/stepped start must win over the default");
    assert_eq!(e.issue_key, key("KAN-1"));
}

#[test]
fn a_duration_outside_jira_grammar_is_refused_instead_of_guessed() {
    let mut h = form_on_duration("form-bad-duration");
    typed(&mut h.app, "1.5h"); // decimals are not Jira grammar
    form_action(&mut h.app, entry_form::Action::Save);

    assert!(h.app.overlay.is_some(), "the popup stays open on a bad duration");
    let m = form_model(&h.app);
    assert_eq!(format!("{:?}", m.focus), "Duration", "the offending field keeps focus");
    assert!(m.editing, "and stays in edit so the value can be fixed");
    assert!(h.app.ledger.entries().is_empty(), "nothing is staged from an unparseable duration");
}

/// R5: "Invalid input turns the cell red with hint `e.g. 1h30m, 90m` and
/// stays in edit." On the Save path `save()` sets `error`, then re-opens the
/// offending field — and `open()` clears `error` again, so the only place the
/// popup can show the hint (`view.rs`, `if let Some(e) = &m.error`) is empty.
/// Walking with Enter is fine; only Save / Ctrl+S loses the message.
#[test]
fn a_refused_duration_explains_itself() {
    let mut h = form_on_duration("form-bad-duration-hint");
    typed(&mut h.app, "1.5h");
    form_action(&mut h.app, entry_form::Action::Save);
    let m = form_model(&h.app);
    assert!(m.error.as_deref().is_some_and(|e| e.contains("duration")), "error is {:?}", m.error);
}

#[test]
fn walking_past_a_bad_duration_with_enter_does_show_the_hint() {
    let mut h = form_on_duration("form-bad-duration-walk");
    typed(&mut h.app, "1.5h");
    form_action(&mut h.app, entry_form::Action::Commit(1));
    let m = form_model(&h.app);
    assert_eq!(format!("{:?}", m.focus), "Duration", "the walk does not move on");
    assert!(m.error.as_deref().is_some_and(|e| e.contains("1h30m")), "error is {:?}", m.error);
}

#[test]
fn an_empty_duration_is_refused() {
    let mut h = form_on_duration("form-empty-duration");
    form_action(&mut h.app, entry_form::Action::Save);
    assert!(h.app.overlay.is_some(), "the popup stays open");
    assert!(h.app.ledger.entries().is_empty());
}

#[test]
fn the_staged_duration_is_snapped_to_a_quarter_of_an_hour() {
    let mut h = form_on_duration("form-snap-duration");
    typed(&mut h.app, "1h23m");
    form_action(&mut h.app, entry_form::Action::Save);
    let e = h.app.ledger.entries().first().expect("one staged entry");
    assert_eq!(e.seconds, 5400, "83m rounds to the nearest 15 min");
}

#[test]
fn saving_without_a_matching_issue_is_refused() {
    let mut h = harness("form-no-issue", script_with_issues(vec![issue("KAN-1")]));
    h.app.dispatch(Action::Week(week::Action::AddEntry));
    typed(&mut h.app, "nothing-like-this");
    form_action(&mut h.app, entry_form::Action::Save);
    assert!(h.app.overlay.is_some(), "the popup stays open");
    assert_eq!(format!("{:?}", form_model(&h.app).focus), "Issue");
    assert!(h.app.ledger.entries().is_empty());
}
