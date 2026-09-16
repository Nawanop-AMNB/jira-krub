//! Reducer tests: a real `App` on temp files + a fake gateway, driven through
//! `dispatch` and `on_key` exactly as the event loop would.
//!
//! Feature models expose their focus as private-module enums (`day::Pane`,
//! `settings::GlobalField`, …). Tests compare their `Debug` text rather than
//! naming the types, which are not reachable from here.

use super::action::Action;
use super::app::{App, Overlay, Screen};
use super::features::day::rows::prepare_rows;
use super::features::{connect, day, entry_form, settings, week};
use super::test_support::{Harness, harness, script_with_issues};
use crate::application::test_support::{issue, key};
use crate::domain::{EntryId, StartTime};
use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ---- helpers --------------------------------------------------------------

fn dbg_of<T: std::fmt::Debug>(v: &T) -> String {
    format!("{v:?}")
}

fn press(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

fn ctrl_c(app: &mut App) {
    app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
}

fn day_model(app: &App) -> &day::Model {
    match &app.screen {
        Screen::Day(m) => m,
        _ => panic!("not on the day screen"),
    }
}

fn settings_model(app: &App) -> &settings::Model {
    match &app.screen {
        Screen::Settings(m) => m,
        _ => panic!("not on the settings screen"),
    }
}

fn form_model(app: &App) -> &entry_form::Model {
    match &app.overlay {
        Some(Overlay::Form(m)) => m,
        _ => panic!("no entry form open"),
    }
}

/// Prepare-pane rows as currently displayed, by stable identity.
fn row_ids(app: &App) -> Vec<String> {
    let m = day_model(app);
    prepare_rows(app, m).rows.iter().map(|r| r.identity()).collect()
}

fn year_hours(app: &App) -> String {
    settings_model(app).current_year().expect("a draft for the shown year").hours.text().to_string()
}

/// A day view on today with two of my tickets in the list.
fn day_view(name: &str) -> (Harness, NaiveDate) {
    let mut h = harness(name, script_with_issues(vec![issue("KAN-1"), issue("KAN-2")]));
    let date = h.app.today;
    h.app.go_day(date);
    (h, date)
}

fn stage(app: &mut App, id: &str, issue_key: &str, date: NaiveDate, start: &str, seconds: u64) -> EntryId {
    let eid = EntryId::new(id.to_string());
    app.ledger.quick_stage(eid.clone(), key(issue_key), date, StartTime::parse(start).unwrap(), seconds);
    eid
}

fn day_action(app: &mut App, a: day::Action) {
    app.dispatch(Action::Day(a));
}

fn settings_action(app: &mut App, a: settings::Action) {
    app.dispatch(Action::Settings(a));
}

fn form_action(app: &mut App, a: entry_form::Action) {
    app.dispatch(Action::Form(a));
}

// ---- startup (R7) ---------------------------------------------------------

#[test]
fn startup_syncs_through_the_gateway_and_fills_the_ticket_list() {
    let h = harness("startup-sync", script_with_issues(vec![issue("KAN-1")]));
    {
        let calls = h.gateway.calls();
        assert_eq!(calls.myself, 1);
        assert!(calls.jql.first().is_some_and(|j| j.contains("currentUser")), "{:?}", calls.jql);
    }
    assert_eq!(h.app.remote.mine.len(), 1);
    assert!(matches!(h.app.screen, Screen::Week(_)), "a saved config skips the connect screen");
}

// ---- day view: quick stage (R4) -------------------------------------------

#[test]
fn quick_stage_uses_the_configured_duration_and_default_start() {
    let (mut h, date) = day_view("quick-stage-values");
    day_action(&mut h.app, day::Action::QuickStage);
    let e = h.app.ledger.entries().first().expect("one staged entry");
    assert_eq!(e.issue_key, key("KAN-1"), "the selected ticket");
    assert_eq!(e.date, date);
    assert_eq!(e.seconds, 45 * 60, "quick_stage = \"45m\" in config");
    assert_eq!(e.start, StartTime::parse("13:00").unwrap(), "default_start_time in config");
}

#[test]
fn quick_stage_keeps_the_tickets_pane_and_points_the_cursor_at_the_new_row() {
    let (mut h, date) = day_view("quick-stage-cursor");
    // An earlier row, so the new 13:00 entry is not first by accident.
    stage(&mut h.app, "e0", "KAN-2", date, "09:00", 3600);
    day_action(&mut h.app, day::Action::QuickStage);
    let m = day_model(&h.app);
    assert_eq!(dbg_of(&m.pane), "Tickets", "focus stays put so several tickets can be staged in a row");
    assert_eq!(m.prepare_sel, 1, "cursor is on the newly staged 13:00 row");
}

// ---- day view: search box is the top row of the list (R9a) ----------------

#[test]
fn up_from_the_first_ticket_focuses_the_search_box() {
    let (mut h, _) = day_view("day-up-search");
    assert!(!day_model(&h.app).search_focused);
    press(&mut h.app, KeyCode::Up);
    assert!(day_model(&h.app).search_focused);
}

#[test]
fn down_from_the_search_box_walks_back_into_the_ticket_list() {
    let (mut h, _) = day_view("day-down-list");
    press(&mut h.app, KeyCode::Up);
    press(&mut h.app, KeyCode::Down);
    assert!(!day_model(&h.app).search_focused);
}

// ---- day view: inline cell edit (R5b) -------------------------------------

#[test]
fn a_cell_edit_commits_the_typed_duration_and_walks_to_the_description() {
    let (mut h, date) = day_view("cell-edit-commit");
    let id = stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    day_action(&mut h.app, day::Action::SelectPrepare(0));
    press(&mut h.app, KeyCode::Char('d'));
    typed(&mut h.app, "2h");
    press(&mut h.app, KeyCode::Enter);
    assert_eq!(h.app.ledger.entry(&id).unwrap().seconds, 7200);
    let edit = day_model(&h.app).edit.as_ref().expect("the walk continues");
    assert_eq!(dbg_of(&edit.cell), "Title");
}

#[test]
fn escape_reverts_a_cell_edit_and_leaves_edit_mode() {
    let (mut h, date) = day_view("cell-edit-cancel");
    let id = stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    day_action(&mut h.app, day::Action::SelectPrepare(0));
    press(&mut h.app, KeyCode::Char('d'));
    typed(&mut h.app, "2h");
    press(&mut h.app, KeyCode::Esc);
    assert_eq!(h.app.ledger.entry(&id).unwrap().seconds, 3600, "only Esc reverts");
    assert!(day_model(&h.app).edit.is_none());
}

// ---- day view: row order is stable while editing (R4) ---------------------

#[test]
fn a_walked_row_keeps_its_place_while_the_walk_is_open() {
    let (mut h, date) = day_view("row-freeze-hold");
    stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    let late = stage(&mut h.app, "e2", "KAN-2", date, "09:30", 3600);
    day_action(&mut h.app, day::Action::SelectPrepare(1));

    press(&mut h.app, KeyCode::Char('s'));
    typed(&mut h.app, "0800");
    press(&mut h.app, KeyCode::Enter);

    assert_eq!(h.app.ledger.entry(&late).unwrap().start, StartTime::parse("08:00").unwrap(), "the new start is saved");
    assert!(day_model(&h.app).frozen.is_some(), "the walk is still open");
    assert_eq!(row_ids(&h.app), vec!["e:e1".to_string(), "e:e2".to_string()], "order unchanged mid-walk");
    assert_eq!(day_model(&h.app).prepare_sel, 1, "the cursor is still on the row being walked");
}

#[test]
fn rows_resort_and_the_cursor_follows_once_the_walk_ends() {
    let (mut h, date) = day_view("row-freeze-release");
    stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    stage(&mut h.app, "e2", "KAN-2", date, "09:30", 3600);
    day_action(&mut h.app, day::Action::SelectPrepare(1));

    press(&mut h.app, KeyCode::Char('s'));
    typed(&mut h.app, "0800");
    press(&mut h.app, KeyCode::Enter); // start → duration
    press(&mut h.app, KeyCode::Enter); // duration → description
    press(&mut h.app, KeyCode::Enter); // description → walk ends

    assert!(day_model(&h.app).frozen.is_none());
    assert_eq!(row_ids(&h.app), vec!["e:e2".to_string(), "e:e1".to_string()], "08:00 sorts first");
    assert_eq!(day_model(&h.app).prepare_sel, 0, "the cursor followed the row");
}

// ---- day view: remove (R5b, R13) ------------------------------------------

#[test]
fn remove_deletes_a_staged_row_outright() {
    let (mut h, date) = day_view("remove-staged");
    stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    day_action(&mut h.app, day::Action::SelectPrepare(0));
    day_action(&mut h.app, day::Action::Remove);
    assert!(h.app.ledger.entries().is_empty());
}

#[test]
fn remove_toggles_a_pushed_row_between_marked_for_deletion_and_pushed() {
    let (mut h, date) = day_view("remove-pushed");
    let id = stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    h.app.ledger.mark_pushed(&id, "w-1".into());
    day_action(&mut h.app, day::Action::SelectPrepare(0));

    day_action(&mut h.app, day::Action::Remove);
    assert!(h.app.ledger.entry(&id).unwrap().is_deleted(), "nothing touches Jira until the push");

    day_action(&mut h.app, day::Action::Remove);
    assert!(h.app.ledger.entry(&id).unwrap().is_pushed(), "a second Backspace undoes the delete");
}

// ---- settings (R16) -------------------------------------------------------

#[test]
fn open_settings_switches_to_the_settings_screen() {
    let mut h = harness("settings-open", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    assert!(matches!(h.app.screen, Screen::Settings(_)));
    assert_eq!(dbg_of(&settings_model(&h.app).tab), "Global", "the Global tab opens first");
}

#[test]
fn the_year_arrows_step_the_year_and_keep_the_focused_row() {
    let mut h = harness("settings-year-step", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    settings_action(&mut h.app, settings::Action::SwitchTab);
    assert_eq!(dbg_of(&settings_model(&h.app).tab), "Year");

    let first_year = settings_model(&h.app).year;
    let focus = dbg_of(&settings_model(&h.app).y_focus);

    settings_action(&mut h.app, settings::Action::PrevYear);
    assert_eq!(settings_model(&h.app).year, first_year - 1);
    assert_eq!(dbg_of(&settings_model(&h.app).y_focus), focus, "repeated ←/→ keep stepping years");

    settings_action(&mut h.app, settings::Action::NextYear);
    assert_eq!(settings_model(&h.app).year, first_year);
}

#[test]
fn year_hours_revert_restores_the_field_and_commit_keeps_the_typed_value() {
    let mut h = harness("settings-year-hours", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    settings_action(&mut h.app, settings::Action::SwitchTab);

    settings_action(&mut h.app, settings::Action::Activate);
    assert!(settings_model(&h.app).editing, "Enter opens the field for typing");
    settings_action(&mut h.app, settings::Action::Char('7'));
    assert_eq!(year_hours(&h.app), "7");
    settings_action(&mut h.app, settings::Action::Revert);
    assert_eq!(year_hours(&h.app), "", "Esc restores the blank override");
    assert!(!settings_model(&h.app).editing);

    settings_action(&mut h.app, settings::Action::Activate);
    settings_action(&mut h.app, settings::Action::Char('6'));
    settings_action(&mut h.app, settings::Action::Commit);
    assert!(!settings_model(&h.app).editing);
    assert_eq!(year_hours(&h.app), "6");
}

#[test]
fn saving_writes_the_year_hours_override_into_the_config() {
    let mut h = harness("settings-year-save", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    settings_action(&mut h.app, settings::Action::SwitchTab);
    let year = settings_model(&h.app).year;
    settings_action(&mut h.app, settings::Action::Activate);
    settings_action(&mut h.app, settings::Action::Char('6'));
    settings_action(&mut h.app, settings::Action::Commit);

    settings_action(&mut h.app, settings::Action::Save);
    let cfg = h.app.config.as_ref().expect("still connected");
    assert_eq!(cfg.years[&year].hours_per_day, Some(6));
}

#[test]
fn toggling_a_workday_chip_and_saving_updates_the_config() {
    let mut h = harness("settings-workdays", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    settings_action(&mut h.app, settings::Action::Down); // Hours → Workdays
    assert_eq!(dbg_of(&settings_model(&h.app).g_focus), "Workdays");

    settings_action(&mut h.app, settings::Action::Activate); // open the chip row
    assert!(settings_model(&h.app).editing);
    settings_action(&mut h.app, settings::Action::Right); // Mon → Tue
    settings_action(&mut h.app, settings::Action::Right); // Tue → Wed
    settings_action(&mut h.app, settings::Action::Activate); // Wed off
    settings_action(&mut h.app, settings::Action::Commit);

    settings_action(&mut h.app, settings::Action::Save);
    assert!(!h.app.config.as_ref().unwrap().global.workdays[2], "Wednesday is no longer a workday");
}

#[test]
fn discarding_dirty_settings_leaves_the_config_unchanged() {
    let mut h = harness("settings-discard", script_with_issues(Vec::new()));
    let before = h.app.config.clone().expect("connected");
    h.app.dispatch(Action::OpenSettings);
    settings_action(&mut h.app, settings::Action::Down);
    settings_action(&mut h.app, settings::Action::Activate);
    settings_action(&mut h.app, settings::Action::Right);
    settings_action(&mut h.app, settings::Action::Right);
    settings_action(&mut h.app, settings::Action::Activate);
    settings_action(&mut h.app, settings::Action::Commit);

    settings_action(&mut h.app, settings::Action::Cancel);
    assert!(matches!(h.app.overlay, Some(Overlay::Confirm(_))), "edited fields ask before discarding");
    h.app.dispatch(Action::ConfirmYes);

    assert_eq!(h.app.config.as_ref(), Some(&before));
    assert!(matches!(h.app.screen, Screen::Week(_)));
}

// ---- settings → connect (R16) ---------------------------------------------

/// Walk from the Hours row down to `Jira connection…`.
fn focus_connection_row(app: &mut App) {
    for _ in 0..6 {
        settings_action(app, settings::Action::Down);
    }
    assert_eq!(dbg_of(&settings_model(app).g_focus), "Connection");
}

#[test]
fn the_connection_row_opens_connect_marked_as_coming_from_settings() {
    let mut h = harness("connect-from-settings", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    focus_connection_row(&mut h.app);
    settings_action(&mut h.app, settings::Action::Activate);
    match &h.app.screen {
        Screen::Connect(m) => assert!(m.from_settings),
        _ => panic!("expected the connect screen"),
    }
}

#[test]
fn quitting_connect_opened_from_settings_returns_to_settings() {
    let mut h = harness("connect-back-to-settings", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    focus_connection_row(&mut h.app);
    settings_action(&mut h.app, settings::Action::Activate);
    h.app.dispatch(Action::Connect(connect::Action::Quit));
    assert!(matches!(h.app.screen, Screen::Settings(_)), "Esc returns to Settings, not main");
}

// ---- entry form popup (R5) ------------------------------------------------

#[test]
fn add_entry_on_the_week_screen_opens_the_form_already_in_edit_mode() {
    let mut h = harness("form-open", script_with_issues(vec![issue("KAN-1")]));
    h.app.dispatch(Action::Week(week::Action::AddEntry));
    let m = form_model(&h.app);
    assert!(m.editing, "a walk form opens with its first field open");
    assert_eq!(dbg_of(&m.focus), "Issue");
}

#[test]
fn commit_walks_the_form_from_issue_to_date_to_start() {
    let mut h = harness("form-walk", script_with_issues(vec![issue("KAN-1")]));
    h.app.dispatch(Action::Week(week::Action::AddEntry));
    form_action(&mut h.app, entry_form::Action::Commit(1));
    assert_eq!(dbg_of(&form_model(&h.app).focus), "Date");
    form_action(&mut h.app, entry_form::Action::Commit(1));
    assert_eq!(dbg_of(&form_model(&h.app).focus), "Start");
}

#[test]
fn revert_restores_the_form_field_from_its_backup() {
    let mut h = harness("form-revert", script_with_issues(vec![issue("KAN-1")]));
    h.app.dispatch(Action::Week(week::Action::AddEntry));
    form_action(&mut h.app, entry_form::Action::Commit(1));
    form_action(&mut h.app, entry_form::Action::Commit(1));
    assert_eq!(form_model(&h.app).start.text(), "13:00");

    form_action(&mut h.app, entry_form::Action::Char('8'));
    assert_ne!(form_model(&h.app).start.text(), "13:00");
    form_action(&mut h.app, entry_form::Action::Revert);
    assert_eq!(form_model(&h.app).start.text(), "13:00");
    assert!(!form_model(&h.app).editing);
}

#[test]
fn saving_the_form_stages_an_entry_with_the_typed_duration() {
    let mut h = harness("form-save", script_with_issues(vec![issue("KAN-1")]));
    let date = h.app.today;
    h.app.dispatch(Action::Week(week::Action::AddEntry));
    for c in "KAN-1".chars() {
        form_action(&mut h.app, entry_form::Action::Char(c));
    }
    for _ in 0..3 {
        form_action(&mut h.app, entry_form::Action::Commit(1));
    }
    assert_eq!(dbg_of(&form_model(&h.app).focus), "Duration");
    for c in "2h".chars() {
        form_action(&mut h.app, entry_form::Action::Char(c));
    }
    form_action(&mut h.app, entry_form::Action::Save);

    assert!(h.app.overlay.is_none(), "the popup closes on save");
    let e = h.app.ledger.entries().first().expect("one staged entry");
    assert_eq!(e.issue_key, key("KAN-1"));
    assert_eq!(e.seconds, 7200);
    assert_eq!(e.date, date);
}

// ---- Ctrl+C (R9b) ---------------------------------------------------------

#[test]
fn one_ctrl_c_warns_without_quitting() {
    let mut h = harness("ctrl-c-once", script_with_issues(Vec::new()));
    ctrl_c(&mut h.app);
    assert!(!h.app.quit);
    assert!(h.app.status.is_some(), "the warning is shown");
}

#[test]
fn a_second_ctrl_c_inside_the_window_quits() {
    let mut h = harness("ctrl-c-twice", script_with_issues(Vec::new()));
    ctrl_c(&mut h.app);
    ctrl_c(&mut h.app);
    assert!(h.app.quit);
}
