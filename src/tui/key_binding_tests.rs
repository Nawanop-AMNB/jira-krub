//! Key bindings that the spec calls out by name (R4, R9a): which keys move
//! the day, which keys never do, what letters mean inside the search box, and
//! what Esc and Space do in EDIT mode.
//!
//! These drive `on_key` rather than dispatching actions, so the mapping in
//! each feature's `keys()` is what is under test.

use super::action::Action;
use super::app::{App, Screen};
use super::features::{day, settings, week};
use super::test_support::{harness, script_with_issues};
use crate::application::test_support::issue;
use chrono::{Days, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

fn day_model(app: &App) -> &day::Model {
    match &app.screen {
        Screen::Day(m) => m,
        _ => panic!("not on the day screen"),
    }
}

fn week_model(app: &App) -> &week::Model {
    match &app.screen {
        Screen::Week(m) => m,
        _ => panic!("not on the week screen"),
    }
}

fn settings_model(app: &App) -> &settings::Model {
    match &app.screen {
        Screen::Settings(m) => m,
        _ => panic!("not on the settings screen"),
    }
}

fn dbg_of<T: std::fmt::Debug>(v: &T) -> String {
    format!("{v:?}")
}

fn day_view(name: &str) -> (super::test_support::Harness, NaiveDate) {
    let mut h = harness(name, script_with_issues(vec![issue("KAN-1"), issue("KAN-2")]));
    let date = h.app.today;
    h.app.go_day(date);
    (h, date)
}

// ---- R4: arrows switch pane, only [ and ] move the day --------------------

#[test]
fn arrow_keys_switch_pane_and_never_change_the_day() {
    let (mut h, date) = day_view("keys-arrows-keep-day");
    press(&mut h.app, KeyCode::Right);
    assert_eq!(dbg_of(&day_model(&h.app).pane), "Prepare");
    assert_eq!(day_model(&h.app).date, date, "a stray → must not move entries to another date");

    press(&mut h.app, KeyCode::Left);
    assert_eq!(dbg_of(&day_model(&h.app).pane), "Tickets");
    assert_eq!(day_model(&h.app).date, date, "a stray ← must not move entries to another date");
}

#[test]
fn bracket_keys_step_the_day_one_at_a_time() {
    let (mut h, date) = day_view("keys-brackets-step-day");
    press(&mut h.app, KeyCode::Char(']'));
    assert_eq!(day_model(&h.app).date, date + Days::new(1));
    press(&mut h.app, KeyCode::Char('['));
    press(&mut h.app, KeyCode::Char('['));
    assert_eq!(day_model(&h.app).date, date - Days::new(1));
}

// ---- R9a: the search box swallows letters --------------------------------

#[test]
fn letters_typed_in_the_search_box_are_never_hotkeys() {
    let (mut h, date) = day_view("keys-search-swallows-letters");
    press(&mut h.app, KeyCode::Char('/'));
    assert!(day_model(&h.app).search_focused);

    // Every one of these is a hotkey outside the box: quit, push, watch,
    // prev/next day, edit cells.
    typed(&mut h.app, "qpwsdn");

    assert_eq!(day_model(&h.app).search.text(), "qpwsdn");
    assert!(!h.app.quit, "typing must never quit");
    assert!(h.app.overlay.is_none(), "typing must never open the push confirm");
    assert_eq!(day_model(&h.app).date, date, "typing must never change the day");
    assert!(day_model(&h.app).edit.is_none(), "typing must never open a cell editor");
}

#[test]
fn escape_in_a_filled_search_box_clears_it_before_leaving_it() {
    let (mut h, _) = day_view("keys-search-escape");
    press(&mut h.app, KeyCode::Char('/'));
    typed(&mut h.app, "kan");
    press(&mut h.app, KeyCode::Esc);
    assert_eq!(day_model(&h.app).search.text(), "");
    assert!(day_model(&h.app).search_focused, "the first Esc only clears the text");
    press(&mut h.app, KeyCode::Esc);
    assert!(!day_model(&h.app).search_focused, "the second Esc leaves the box");
}

// ---- R1: the week screen opens the day the cursor is on -------------------

#[test]
fn enter_on_the_week_screen_opens_the_selected_day() {
    let mut h = harness("keys-week-open-day", script_with_issues(Vec::new()));
    press(&mut h.app, KeyCode::Char('t'));
    let selected = week_model(&h.app).selected_date();
    press(&mut h.app, KeyCode::Enter);
    assert_eq!(day_model(&h.app).date, selected, "Enter opens the highlighted row, not its neighbour");
}

#[test]
fn j_and_k_move_the_week_cursor_down_and_up() {
    let mut h = harness("keys-week-cursor", script_with_issues(Vec::new()));
    h.app.dispatch(Action::Week(week::Action::Select(2)));
    press(&mut h.app, KeyCode::Char('j'));
    assert_eq!(week_model(&h.app).selected, 3, "j goes down the week");
    press(&mut h.app, KeyCode::Char('k'));
    press(&mut h.app, KeyCode::Char('k'));
    assert_eq!(week_model(&h.app).selected, 1, "k goes back up");
}

#[test]
fn the_week_cursor_wraps_around_both_ends() {
    let mut h = harness("keys-week-wrap", script_with_issues(Vec::new()));
    h.app.dispatch(Action::Week(week::Action::Select(0)));
    press(&mut h.app, KeyCode::Char('k'));
    assert_eq!(week_model(&h.app).selected, 6, "Mon ↑ wraps to Sun");
    press(&mut h.app, KeyCode::Char('j'));
    assert_eq!(week_model(&h.app).selected, 0);
}

// ---- R9a: Esc reverts an open field, Space toggles a chip ------------------

#[test]
fn escape_in_an_open_settings_field_reverts_the_typed_value() {
    let mut h = harness("keys-settings-esc-reverts", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    let before = settings_model(&h.app).hours.text().to_string();

    press(&mut h.app, KeyCode::Enter); // open the target-hours field
    assert!(settings_model(&h.app).editing);
    typed(&mut h.app, "5");
    assert_ne!(settings_model(&h.app).hours.text(), before);

    press(&mut h.app, KeyCode::Esc);
    assert_eq!(settings_model(&h.app).hours.text(), before, "Esc is the only way back to the old value");
    assert!(!settings_model(&h.app).editing);
}

#[test]
fn space_toggles_the_workday_chip_under_the_cursor() {
    let mut h = harness("keys-settings-chip-space", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    press(&mut h.app, KeyCode::Down); // Hours → Workdays
    press(&mut h.app, KeyCode::Enter); // open the chip row
    assert!(settings_model(&h.app).editing);
    let before = settings_model(&h.app).workdays;

    press(&mut h.app, KeyCode::Char(' '));
    let after = settings_model(&h.app).workdays;
    assert_eq!(after[0], !before[0], "Space toggles the chip the cursor is on");
    assert_eq!(after[1..], before[1..], "and only that one");
}
