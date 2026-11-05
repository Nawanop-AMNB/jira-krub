//! Days off: the Year-tab wording, and the `x` quick day-off modal on the
//! week summary (add, edit, move, remove; all persisted straight to disk).

use super::action::Action;
use super::app::App;
use super::features::settings;
use super::test_support::{CONFIG_TOML, Harness, go_worklog, harness, harness_with_config, render, render_styles, script_with_issues};
use super::theme;
use crate::application::Config;
use crate::domain::Week;
use chrono::NaiveDate;
use ratatui::style::Modifier;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// A Wednesday in the past, so the row's status never depends on today.
fn selected() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, 18).unwrap()
}

fn press(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn ctrl(app: &mut App, c: char) {
    app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
}

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

/// Focus the date field and replace its text.
fn set_date(app: &mut App, text: &str) {
    press(app, KeyCode::Up);
    for _ in 0..12 {
        press(app, KeyCode::Backspace);
    }
    typed(app, text);
}

/// Move to the Save button (from Date or Name) and press it.
fn submit(app: &mut App) {
    press(app, KeyCode::Down);
    press(app, KeyCode::Down);
    press(app, KeyCode::Enter);
}

/// Clear the name field (focused when the modal opens) and type a new one.
fn set_name(app: &mut App, text: &str) {
    for _ in 0..30 {
        press(app, KeyCode::Backspace);
    }
    typed(app, text);
}

fn week_on(h: &mut Harness, date: NaiveDate) {
    go_worklog(&mut h.app);
    h.app.go_week(Week::containing(date), date);
}

fn open(name: &str) -> Harness {
    open_with_config(name, CONFIG_TOML)
}

fn open_with_config(name: &str, toml: &str) -> Harness {
    let mut h = harness_with_config(name, script_with_issues(Vec::new()), toml);
    week_on(&mut h, selected());
    h
}

fn with_day_off(toml: &str, date: &str, name: &str) -> String {
    let year = &date[..4];
    format!("{toml}\n[year.{year}]\nholidays = [{{ date = \"{date}\", name = \"{name}\" }}]\n")
}

fn text(app: &mut App) -> String {
    render(app).join("\n")
}

/// The status line's text. Asserted on the model, not the frame: the week's
/// hint bar is long enough to push the status off a 120-column footer.
fn status(app: &App) -> String {
    app.status.as_ref().map(|s| s.text.clone()).unwrap_or_default()
}

fn modal_open(app: &App) -> bool {
    app.overlay.is_some()
}

fn days_off(cfg: &Config) -> Vec<(NaiveDate, String)> {
    cfg.years.values().flat_map(|y| y.holidays.iter().map(|h| (h.date, h.name.clone()))).collect()
}

fn memory(h: &Harness) -> Vec<(NaiveDate, String)> {
    days_off(h.app.config.as_ref().expect("config"))
}

fn on_disk(h: &Harness) -> Vec<(NaiveDate, String)> {
    days_off(&h.app.deps.config_store.load().expect("loads").expect("config file exists"))
}

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn settings_year_tab_says_days_off() {
    let mut h = harness("dayoff-settings-wording", script_with_issues(Vec::new()));
    h.app.dispatch(Action::OpenSettings);
    h.app.dispatch(Action::Settings(settings::Action::SwitchTab));
    let frame = text(&mut h.app);
    assert!(frame.contains("Days off"), "{frame}");
    assert!(frame.contains("+ add day off"), "{frame}");
    assert!(!frame.contains("Public holidays"), "{frame}");
    assert!(!frame.contains("add holiday"), "{frame}");
}

#[test]
fn x_opens_day_off_modal_for_selected_day() {
    let mut h = open("dayoff-open");
    press(&mut h.app, KeyCode::Char('x'));
    assert!(modal_open(&h.app));
    let frame = text(&mut h.app);
    assert!(frame.contains("┌ day off "), "title: {frame}");
    assert!(!frame.contains("edit day off"), "{frame}");
    assert!(frame.contains("2026-03-18"), "{frame}");
    assert!(frame.contains("Enter press"), "{frame}");
    assert!(!frame.contains("Ctrl-D"), "no remove hint when adding: {frame}");
}

#[test]
fn x_hint_is_listed_on_week_view() {
    let mut h = open("dayoff-hint");
    let frame = text(&mut h.app);
    let footer = frame.lines().last().unwrap_or_default();
    assert!(footer.contains("x day off"), "{footer}");
}

#[test]
fn enter_twice_saves_day_off_and_persists() {
    let mut h = open("dayoff-save");
    let global_before = h.app.config.as_ref().unwrap().global.clone();
    press(&mut h.app, KeyCode::Char('x'));
    typed(&mut h.app, "sick leave");
    press(&mut h.app, KeyCode::Enter); // name -> Save
    press(&mut h.app, KeyCode::Enter); // press Save

    assert!(!modal_open(&h.app));
    let want = vec![(selected(), "sick leave".to_string())];
    assert_eq!(memory(&h), want);
    assert_eq!(on_disk(&h), want);
    assert_eq!(h.app.config.as_ref().unwrap().global, global_before, "global settings untouched");
    let frame = text(&mut h.app);
    assert!(frame.contains("off · sick leave"), "{frame}");
    assert_eq!(status(&h.app), "day off saved");
}

#[test]
fn blank_name_saves_as_day_off() {
    let mut h = open("dayoff-blank-name");
    press(&mut h.app, KeyCode::Char('x'));
    typed(&mut h.app, "   ");
    submit(&mut h.app);
    assert_eq!(memory(&h), vec![(selected(), "day off".to_string())]);
    assert_eq!(on_disk(&h), vec![(selected(), "day off".to_string())]);
}

#[test]
fn typed_short_date_uses_selected_year() {
    let mut h = open("dayoff-short-date");
    press(&mut h.app, KeyCode::Char('x'));
    set_date(&mut h.app, "11-6");
    submit(&mut h.app);
    assert!(!modal_open(&h.app));
    assert_eq!(memory(&h), vec![(ymd(2026, 11, 6), "day off".to_string())]);
    assert_eq!(on_disk(&h), memory(&h));
}

#[test]
fn bad_date_keeps_modal_open_with_error() {
    let mut h = open("dayoff-bad-date");
    let before = h.app.config.clone();
    press(&mut h.app, KeyCode::Char('x'));
    set_date(&mut h.app, "13-45");
    submit(&mut h.app);
    assert!(modal_open(&h.app));
    let frame = text(&mut h.app);
    assert!(frame.contains('✗'), "an error line is shown: {frame}");
    assert_eq!(h.app.config, before);
    assert!(on_disk(&h).is_empty());
}

#[test]
fn esc_cancels_without_change() {
    let mut h = open("dayoff-esc");
    let before = h.app.config.clone();
    press(&mut h.app, KeyCode::Char('x'));
    assert!(modal_open(&h.app));
    typed(&mut h.app, "nope");
    press(&mut h.app, KeyCode::Esc);
    assert!(!modal_open(&h.app));
    assert_eq!(h.app.config, before);
    assert!(on_disk(&h).is_empty());
}

#[test]
fn x_on_existing_day_off_edits_it() {
    let toml = with_day_off(CONFIG_TOML, "2026-03-18", "9d's day");
    let mut h = open_with_config("dayoff-edit", &toml);
    press(&mut h.app, KeyCode::Char('x'));
    let frame = text(&mut h.app);
    assert!(frame.contains("edit day off"), "{frame}");
    assert!(frame.contains("9d's day"), "{frame}");
    assert!(!frame.contains("Ctrl-D"), "{frame}");

    set_name(&mut h.app, "trip");
    submit(&mut h.app);
    assert!(!modal_open(&h.app));
    assert_eq!(memory(&h), vec![(selected(), "trip".to_string())]);
    assert_eq!(on_disk(&h), memory(&h));
}

#[test]
fn editing_date_moves_the_day_off() {
    let toml = with_day_off(CONFIG_TOML, "2026-03-18", "trip");
    let mut h = open_with_config("dayoff-move", &toml);
    press(&mut h.app, KeyCode::Char('x'));
    set_date(&mut h.app, "2026-03-19");
    submit(&mut h.app);
    assert!(!modal_open(&h.app));
    assert_eq!(memory(&h), vec![(ymd(2026, 3, 19), "trip".to_string())]);
    assert_eq!(on_disk(&h), memory(&h));
}

#[test]
fn keyboard_delete_removes_day_off_and_clears_the_row() {
    let toml = with_day_off(CONFIG_TOML, "2026-03-18", "trip");
    let mut h = open_with_config("dayoff-remove", &toml);
    assert!(text(&mut h.app).contains("off · trip"), "precondition: the row names the day off");
    press(&mut h.app, KeyCode::Char('x'));
    press(&mut h.app, KeyCode::Down); // Save
    press(&mut h.app, KeyCode::Left); // Delete
    press(&mut h.app, KeyCode::Enter);
    assert!(!modal_open(&h.app));
    assert!(memory(&h).is_empty());
    assert!(on_disk(&h).is_empty());
    assert!(h.app.config.as_ref().unwrap().years.is_empty(), "the emptied year entry is dropped");
    let frame = text(&mut h.app);
    let row = frame.lines().find(|l| l.contains("Wed 18")).expect("Wed row");
    assert!(!row.contains("off · "), "{row}");
    assert_eq!(status(&h.app), "day off removed");
}

#[test]
fn x_without_config_opens_nothing() {
    let mut h = open("dayoff-no-config");
    h.app.config = None;
    press(&mut h.app, KeyCode::Char('x'));
    assert!(h.app.overlay.is_none());
    assert!(status(&h.app).contains("not connected"));
}

#[test]
fn tab_and_shift_tab_move_between_fields() {
    let mut h = open("dayoff-tab");
    press(&mut h.app, KeyCode::Char('x'));
    h.app.on_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)); // name -> date
    for _ in 0..12 {
        press(&mut h.app, KeyCode::Backspace);
    }
    typed(&mut h.app, "4-1");
    press(&mut h.app, KeyCode::Tab); // date -> name
    typed(&mut h.app, "fair");
    submit(&mut h.app);
    assert_eq!(memory(&h), vec![(ymd(2026, 4, 1), "fair".to_string())]);
}

#[test]
fn saving_onto_a_date_with_a_day_off_replaces_it_and_keeps_order() {
    let toml = with_day_off(CONFIG_TOML, "2026-03-19", "later");
    let mut h = open_with_config("dayoff-replace", &toml);
    press(&mut h.app, KeyCode::Char('x'));
    typed(&mut h.app, "earlier");
    submit(&mut h.app);
    assert_eq!(memory(&h), vec![(selected(), "earlier".to_string()), (ymd(2026, 3, 19), "later".to_string())]);

    press(&mut h.app, KeyCode::Char('x'));
    set_date(&mut h.app, "3-19");
    submit(&mut h.app);
    assert_eq!(memory(&h), vec![(ymd(2026, 3, 19), "earlier".to_string())], "moved onto an occupied date replaces its name");
}

// ---- buttons ----

/// Rows of the modal box only (title border through bottom border), with
/// their absolute row index, so the footer never leaks into an assertion.
fn box_rows(app: &mut App) -> Vec<(usize, String)> {
    let rows = render(app);
    let top = rows.iter().position(|r| r.contains("┌ ") && r.contains("day off ")).expect("modal title row");
    let left = rows[top].chars().position(|c| c == '┌').expect("modal corner");
    let bottom = (top..rows.len()).find(|&i| rows[i].chars().nth(left) == Some('└')).expect("modal bottom border");
    (top..=bottom).map(|i| (i, rows[i].clone())).collect()
}

fn box_text(app: &mut App) -> String {
    box_rows(app).into_iter().map(|(_, r)| r).collect::<Vec<_>>().join("\n")
}

/// Column and row of the first cell of `needle` inside the modal box.
fn find_in_box(app: &mut App, needle: &str) -> (u16, u16) {
    try_find_in_box(app, needle).unwrap_or_else(|| panic!("{needle:?} not in box: {:?}", box_rows(app)))
}

fn try_find_in_box(app: &mut App, needle: &str) -> Option<(u16, u16)> {
    box_rows(app).iter().find_map(|(y, r)| r.find(needle).map(|byte| (r[..byte].chars().count() as u16, *y as u16)))
}

fn click(app: &mut App, column: u16, row: u16) {
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers: KeyModifiers::NONE });
}

fn click_on(app: &mut App, needle: &str) {
    let (x, y) = find_in_box(app, needle);
    click(app, x, y);
}

const TOML_9D: &str = "9d's day";

fn open_edit(name: &str) -> Harness {
    let toml = with_day_off(CONFIG_TOML, "2026-03-18", TOML_9D);
    let mut h = open_with_config(name, &toml);
    press(&mut h.app, KeyCode::Char('x'));
    assert!(modal_open(&h.app));
    h
}

fn unchanged(h: &Harness) {
    let want = vec![(selected(), TOML_9D.to_string())];
    assert_eq!(memory(h), want);
    assert_eq!(on_disk(h), want);
}

#[test]
fn edit_modal_shows_delete_save_and_cancel() {
    let mut h = open_edit("dayoff-btn-edit");
    let frame = box_text(&mut h.app);
    assert!(frame.contains("[ Delete ]"), "{frame}");
    assert!(frame.contains("[ Save ]"), "{frame}");
    assert!(frame.contains("[ Cancel ]"), "{frame}");
}

#[test]
fn add_modal_has_no_delete() {
    let mut h = open("dayoff-btn-add");
    press(&mut h.app, KeyCode::Char('x'));
    let frame = box_text(&mut h.app);
    assert!(frame.contains("[ Save ]"), "{frame}");
    assert!(frame.contains("[ Cancel ]"), "{frame}");
    assert!(!frame.contains("[ Delete ]"), "{frame}");
}

#[test]
fn delete_button_is_red() {
    let mut h = open_edit("dayoff-delete-style");
    let (dx, dy) = find_in_box(&mut h.app, "[ Delete ]");
    let (sx, sy) = find_in_box(&mut h.app, "[ Save ]");
    let styles = render_styles(&mut h.app);
    for i in 0.."[ Delete ]".len() as u16 {
        let s = styles[dy as usize][(dx + i) as usize];
        assert_eq!(s.fg, theme::bad().fg, "delete is red at {i}");
        assert_eq!(s.add_modifier, Modifier::empty(), "delete is not bold or dim at {i}");
    }
    let save = styles[sy as usize][sx as usize];
    let del = styles[dy as usize][dx as usize];
    assert_ne!(del, save, "Delete must look different from Save");
    assert_eq!(save.fg, theme::accent().fg, "Save keeps the accent");
}

fn assert_removed(h: &Harness) {
    assert!(!modal_open(&h.app));
    assert!(memory(h).is_empty());
    assert!(on_disk(h).is_empty());
    assert_eq!(status(&h.app), "day off removed");
}

#[test]
fn clicking_delete_removes_day_off() {
    let mut h = open_edit("dayoff-click-delete");
    click_on(&mut h.app, "[ Delete ]");
    assert_removed(&h);
}

#[test]
fn enter_on_delete_removes() {
    let mut h = open_edit("dayoff-enter-delete");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Left);
    press(&mut h.app, KeyCode::Enter);
    assert_removed(&h);
}

#[test]
fn clicking_save_saves() {
    let mut h = open_edit("dayoff-click-save");
    set_name(&mut h.app, "trip");
    click_on(&mut h.app, "[ Save ]");
    assert!(!modal_open(&h.app));
    assert_eq!(memory(&h), vec![(selected(), "trip".to_string())]);
    assert_eq!(on_disk(&h), memory(&h));
    assert_eq!(status(&h.app), "day off saved");
}

#[test]
fn clicking_cancel_cancels() {
    let mut h = open_edit("dayoff-click-cancel");
    typed(&mut h.app, "nope");
    click_on(&mut h.app, "[ Cancel ]");
    assert!(!modal_open(&h.app));
    unchanged(&h);
}

// ---- keyboard focus: fields and buttons ----

/// Styles of the cells `needle` occupies inside the modal box.
fn cell_styles(app: &mut App, needle: &str) -> Vec<ratatui::style::Style> {
    let (x, y) = find_in_box(app, needle);
    let styles = render_styles(app);
    (0..needle.chars().count() as u16).map(|i| styles[y as usize][(x + i) as usize]).collect()
}

fn is_reversed(app: &mut App, needle: &str) -> bool {
    cell_styles(app, needle).iter().all(|s| s.add_modifier.contains(Modifier::REVERSED))
}

/// Which target has focus, read off the screen: a field's label turns bold,
/// a button is drawn reversed.
fn focused(app: &mut App) -> &'static str {
    let mut on = Vec::new();
    for field in ["date", "name"] {
        if cell_styles(app, field)[0].add_modifier.contains(Modifier::BOLD) {
            on.push(field);
        }
    }
    for button in ["Delete", "Save", "Cancel"] {
        let label = format!("[ {button} ]");
        if try_find_in_box(app, &label).is_some() && is_reversed(app, &label) {
            on.push(button);
        }
    }
    assert_eq!(on.len(), 1, "exactly one target is focused: {on:?}");
    on[0]
}

fn backtab(app: &mut App) {
    app.on_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
}

#[test]
fn opens_with_name_focused_and_typing_goes_in() {
    let mut h = open("dayoff-focus-open");
    press(&mut h.app, KeyCode::Char('x'));
    assert_eq!(focused(&mut h.app), "name");
    typed(&mut h.app, "trip");
    let name_row = box_rows(&mut h.app).into_iter().map(|(_, r)| r).find(|r| r.contains("name")).expect("name row");
    assert!(name_row.contains("trip"), "{name_row}");
    let date_row = box_rows(&mut h.app).into_iter().map(|(_, r)| r).find(|r| r.contains("date")).expect("date row");
    assert!(!date_row.contains("trip"), "{date_row}");
}

#[test]
fn down_from_name_focuses_save() {
    let mut h = open("dayoff-down-save");
    press(&mut h.app, KeyCode::Char('x'));
    press(&mut h.app, KeyCode::Down);
    assert!(is_reversed(&mut h.app, "[ Save ]"));
    assert!(!is_reversed(&mut h.app, "[ Cancel ]"));
}

#[test]
fn left_right_move_between_buttons_clamped() {
    let mut h = open_edit("dayoff-lr-edit");
    press(&mut h.app, KeyCode::Down);
    assert_eq!(focused(&mut h.app), "Save");
    press(&mut h.app, KeyCode::Right);
    assert_eq!(focused(&mut h.app), "Cancel");
    press(&mut h.app, KeyCode::Right);
    assert_eq!(focused(&mut h.app), "Cancel", "no wrap past Cancel");
    press(&mut h.app, KeyCode::Left);
    press(&mut h.app, KeyCode::Left);
    assert_eq!(focused(&mut h.app), "Delete");
    press(&mut h.app, KeyCode::Left);
    assert_eq!(focused(&mut h.app), "Delete", "no wrap past Delete");

    let mut h = open("dayoff-lr-add");
    press(&mut h.app, KeyCode::Char('x'));
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Left);
    assert_eq!(focused(&mut h.app), "Save", "no Delete to the left in add mode");
}

#[test]
fn left_right_in_field_move_cursor() {
    let mut h = open("dayoff-lr-cursor");
    press(&mut h.app, KeyCode::Char('x'));
    typed(&mut h.app, "ab");
    press(&mut h.app, KeyCode::Left);
    typed(&mut h.app, "X");
    assert!(box_text(&mut h.app).contains("aXb"));
    assert_eq!(focused(&mut h.app), "name", "Left/Right never leave the field");
}

#[test]
fn enter_on_name_moves_to_save_without_saving() {
    let mut h = open("dayoff-enter-name");
    let before = h.app.config.clone();
    press(&mut h.app, KeyCode::Char('x'));
    typed(&mut h.app, "trip");
    press(&mut h.app, KeyCode::Enter);
    assert!(modal_open(&h.app));
    assert_eq!(focused(&mut h.app), "Save");
    assert_eq!(h.app.config, before);
    assert!(on_disk(&h).is_empty());
}

#[test]
fn enter_on_date_moves_to_name_without_saving() {
    let mut h = open("dayoff-enter-date");
    let before = h.app.config.clone();
    press(&mut h.app, KeyCode::Char('x'));
    press(&mut h.app, KeyCode::Up);
    assert_eq!(focused(&mut h.app), "date");
    press(&mut h.app, KeyCode::Enter);
    assert!(modal_open(&h.app));
    assert_eq!(focused(&mut h.app), "name");
    assert_eq!(h.app.config, before);
}

#[test]
fn enter_on_save_saves() {
    let mut h = open("dayoff-enter-save");
    press(&mut h.app, KeyCode::Char('x'));
    typed(&mut h.app, "trip");
    press(&mut h.app, KeyCode::Enter);
    assert!(modal_open(&h.app), "the first Enter only moves to Save");
    assert!(on_disk(&h).is_empty());
    press(&mut h.app, KeyCode::Enter);
    assert!(!modal_open(&h.app));
    assert_eq!(memory(&h), vec![(selected(), "trip".to_string())]);
    assert_eq!(on_disk(&h), memory(&h));
    assert_eq!(status(&h.app), "day off saved");
}

#[test]
fn enter_on_cancel_closes_unchanged() {
    let mut h = open_edit("dayoff-enter-cancel");
    typed(&mut h.app, "nope");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Right);
    assert_eq!(focused(&mut h.app), "Cancel");
    press(&mut h.app, KeyCode::Enter);
    assert!(!modal_open(&h.app));
    unchanged(&h);
}

#[test]
fn typing_on_a_button_is_ignored() {
    let mut h = open_edit("dayoff-type-button");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Char('q'));
    press(&mut h.app, KeyCode::Backspace);
    press(&mut h.app, KeyCode::Delete);
    press(&mut h.app, KeyCode::Home);
    press(&mut h.app, KeyCode::End);
    assert!(modal_open(&h.app));
    assert_eq!(focused(&mut h.app), "Save");
    let frame = box_text(&mut h.app);
    assert!(frame.contains(TOML_9D), "name text unchanged: {frame}");
    assert!(!frame.contains('q'), "{frame}");
    unchanged(&h);
}

#[test]
fn esc_closes_without_change_from_field_and_button() {
    let mut h = open_edit("dayoff-esc-field");
    typed(&mut h.app, "nope");
    press(&mut h.app, KeyCode::Esc);
    assert!(!modal_open(&h.app));
    unchanged(&h);

    let mut h = open_edit("dayoff-esc-button");
    typed(&mut h.app, "nope");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Esc);
    assert!(!modal_open(&h.app));
    unchanged(&h);
}

#[test]
fn ctrl_d_does_nothing() {
    let mut h = open_edit("dayoff-ctrl-d-edit");
    ctrl(&mut h.app, 'd');
    assert!(modal_open(&h.app));
    unchanged(&h);

    let mut h = open("dayoff-ctrl-d-add");
    let before = h.app.config.clone();
    press(&mut h.app, KeyCode::Char('x'));
    ctrl(&mut h.app, 'd');
    assert!(modal_open(&h.app));
    assert_eq!(h.app.config, before);
}

#[test]
fn up_from_buttons_returns_to_name_and_up_from_date_stays() {
    let mut h = open_edit("dayoff-up");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Up);
    assert_eq!(focused(&mut h.app), "name");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Right);
    press(&mut h.app, KeyCode::Up);
    assert_eq!(focused(&mut h.app), "name", "from Cancel too");
    press(&mut h.app, KeyCode::Up);
    assert_eq!(focused(&mut h.app), "date");
    press(&mut h.app, KeyCode::Up);
    assert_eq!(focused(&mut h.app), "date", "Up on Date stays put");
    press(&mut h.app, KeyCode::Down);
    assert_eq!(focused(&mut h.app), "name");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Down);
    assert_eq!(focused(&mut h.app), "Save", "Down on the button row stays put");
}

#[test]
fn tab_cycles_all_targets() {
    let mut h = open_edit("dayoff-tab-edit");
    for want in ["Delete", "Save", "Cancel", "date", "name"] {
        press(&mut h.app, KeyCode::Tab);
        assert_eq!(focused(&mut h.app), want);
    }
    for want in ["date", "Cancel", "Save", "Delete", "name"] {
        backtab(&mut h.app);
        assert_eq!(focused(&mut h.app), want);
    }

    let mut h = open("dayoff-tab-add");
    press(&mut h.app, KeyCode::Char('x'));
    for want in ["Save", "Cancel", "date", "name"] {
        press(&mut h.app, KeyCode::Tab);
        assert_eq!(focused(&mut h.app), want);
    }
    for want in ["date", "Cancel", "Save", "name"] {
        backtab(&mut h.app);
        assert_eq!(focused(&mut h.app), want);
    }
}

#[test]
fn focused_delete_is_red_reversed() {
    let mut h = open_edit("dayoff-delete-focused");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Left);
    assert_eq!(focused(&mut h.app), "Delete");
    for (i, s) in cell_styles(&mut h.app, "[ Delete ]").iter().enumerate() {
        assert_eq!(s.fg, theme::bad().fg, "red at {i}");
        assert!(s.add_modifier.contains(Modifier::REVERSED), "reversed at {i}");
    }
    for (i, s) in cell_styles(&mut h.app, "[ Save ]").iter().enumerate() {
        assert_eq!(s.fg, theme::accent().fg, "Save keeps the accent at {i}");
        assert!(!s.add_modifier.contains(Modifier::REVERSED), "Save not reversed at {i}");
    }
}

#[test]
fn focused_save_keeps_accent_colour_reversed() {
    let mut h = open("dayoff-save-focused");
    press(&mut h.app, KeyCode::Char('x'));
    press(&mut h.app, KeyCode::Down);
    for (i, s) in cell_styles(&mut h.app, "[ Save ]").iter().enumerate() {
        assert_eq!(s.fg, theme::accent().fg, "accent at {i}");
        assert!(s.add_modifier.contains(Modifier::REVERSED), "reversed at {i}");
    }
}

#[test]
fn bad_date_on_save_keeps_modal_with_error() {
    let mut h = open("dayoff-bad-date-save");
    let before = h.app.config.clone();
    press(&mut h.app, KeyCode::Char('x'));
    set_date(&mut h.app, "13-45");
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Down);
    assert_eq!(focused(&mut h.app), "Save");
    press(&mut h.app, KeyCode::Enter);
    assert!(modal_open(&h.app));
    assert!(text(&mut h.app).contains('✗'));
    assert_eq!(h.app.config, before);
    assert!(on_disk(&h).is_empty());
}

#[test]
fn footer_shows_move_press_close_hints() {
    let mut h = open_edit("dayoff-footer-edit");
    let mut add = open("dayoff-footer-add");
    press(&mut add.app, KeyCode::Char('x'));
    for app in [&mut h.app, &mut add.app] {
        let frame = text(app);
        let footer = frame.lines().last().unwrap_or_default();
        assert!(footer.contains("↑↓←→ move"), "{footer}");
        assert!(footer.contains("Enter press"), "{footer}");
        assert!(footer.contains("Esc close"), "{footer}");
        assert!(!footer.contains("Ctrl-D"), "{footer}");
        assert!(!footer.contains("Enter save"), "{footer}");
    }
}
