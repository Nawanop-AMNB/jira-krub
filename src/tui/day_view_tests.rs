//! Day view behaviour the spec spells out (R2, R4, R5b): leaving a cell saves
//! it, staging a search result watches the ticket, and what the prepare pane
//! and ticket pane actually list.

use super::action::Action;
use super::app::{App, Screen};
use super::features::day;
use super::features::day::rows::{prepare_rows, staged_total, ticket_rows};
use super::test_support::{Harness, harness, script_with_issues};
use crate::application::test_support::{issue, key, remote};
use crate::domain::{Entry, EntryId, EntryState, Issue, StartTime};
use chrono::{Datelike, NaiveDate};
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

fn day_action(app: &mut App, a: day::Action) {
    app.dispatch(Action::Day(a));
}

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

fn row_ids(app: &App) -> Vec<String> {
    let m = day_model(app);
    prepare_rows(app, m).rows.iter().map(|r| r.identity()).collect()
}

/// Stable identity of the prepare row the cursor is on.
fn selected_row(app: &App) -> String {
    let m = day_model(app);
    prepare_rows(app, m).rows[m.prepare_sel].identity()
}

fn ticket_keys(app: &App) -> Vec<String> {
    let m = day_model(app);
    ticket_rows(app, m).iter().map(|r| r.issue.key.to_string()).collect()
}

/// Put `issues` in the day view's "jira" search-results section.
fn with_search_results(app: &mut App, issues: Vec<Issue>) {
    match &mut app.screen {
        Screen::Day(m) => m.s.results = issues,
        _ => panic!("not on the day screen"),
    }
}

// ---- R4: leaving a cell in any way saves it -------------------------------

#[test]
fn clicking_another_row_saves_the_open_cell_and_re_sorts() {
    let (mut h, date) = day_view("day-unfocus-commits");
    let first = stage(&mut h.app, "e1", "KAN-1", date, "09:30", 3600);
    stage(&mut h.app, "e2", "KAN-2", date, "09:00", 3600);
    // rows: e2 (09:00), e1 (09:30)
    day_action(&mut h.app, day::Action::SelectPrepare(1));

    press(&mut h.app, KeyCode::Char('s'));
    typed(&mut h.app, "0800");
    // Click the other row instead of pressing Enter.
    day_action(&mut h.app, day::Action::SelectPrepare(0));

    assert_eq!(
        h.app.ledger.entry(&first).unwrap().start,
        StartTime::parse("08:00").unwrap(),
        "leaving the cell saves it — only Esc reverts"
    );
    assert_eq!(row_ids(&h.app), vec!["e:e1".to_string(), "e:e2".to_string()], "the list re-sorts once the walk ends");
    assert_eq!(selected_row(&h.app), "e:e2", "the cursor follows the clicked row to its new place");
    assert!(day_model(&h.app).edit.is_none());
}

#[test]
fn changing_the_day_saves_the_open_cell() {
    let (mut h, date) = day_view("day-daychange-commits");
    let id = stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    day_action(&mut h.app, day::Action::SelectPrepare(0));

    press(&mut h.app, KeyCode::Char('d'));
    typed(&mut h.app, "2h");
    day_action(&mut h.app, day::Action::NextDay); // the ◀ ▶ in the title bar

    assert_eq!(h.app.ledger.entry(&id).unwrap().seconds, 7200, "the duration is kept when the day changes");
    assert!(day_model(&h.app).edit.is_none());
}

#[test]
fn unparseable_text_on_unfocus_keeps_the_old_value_and_reports_it() {
    let (mut h, date) = day_view("day-unfocus-bad-value");
    let id = stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    day_action(&mut h.app, day::Action::SelectPrepare(0));

    press(&mut h.app, KeyCode::Char('d'));
    typed(&mut h.app, "1.5h"); // decimals are not Jira grammar
    day_action(&mut h.app, day::Action::FocusTickets);

    assert_eq!(h.app.ledger.entry(&id).unwrap().seconds, 3600, "the old value survives");
    assert!(h.app.status.is_some(), "the status line explains the dropped edit");
    assert!(day_model(&h.app).edit.is_none());
}

// ---- R2 / R16: staging a search result watches it -------------------------

#[test]
fn staging_a_jira_search_result_adds_it_to_the_watchlist() {
    let (mut h, _) = day_view("day-autowatch-search");
    with_search_results(&mut h.app, vec![issue("STW-140")]);
    let idx = ticket_keys(&h.app).iter().position(|k| k == "STW-140").expect("the jira section row");

    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::QuickStage);

    assert!(h.app.ledger.is_watched(&key("STW-140")), "otherwise the row vanishes on the next sync");
    assert_eq!(h.app.ledger.entries().len(), 1);
}

#[test]
fn staging_one_of_my_own_tickets_does_not_touch_the_watchlist() {
    let (mut h, _) = day_view("day-no-autowatch-mine");
    day_action(&mut h.app, day::Action::SelectTicket(0));
    day_action(&mut h.app, day::Action::QuickStage);
    assert!(h.app.ledger.watchlist().is_empty(), "assigned issues are already in the list");
}

// ---- R2: watchlist rows come first ---------------------------------------

#[test]
fn watched_tickets_are_listed_above_my_assigned_ones() {
    let (mut h, _) = day_view("day-watchlist-first");
    // KAN-2 is second in "mine"; watching it must lift it to the top.
    h.app.ledger.watch(&key("KAN-2"));
    assert_eq!(ticket_keys(&h.app), vec!["KAN-2".to_string(), "KAN-1".to_string()]);
}

// ---- R4: the prepare pane -------------------------------------------------

#[test]
fn a_worklog_already_adopted_locally_is_not_listed_twice() {
    let (mut h, date) = day_view("day-no-duplicate-row");
    let id = stage(&mut h.app, "e1", "KAN-1", date, "09:00", 3600);
    h.app.ledger.mark_pushed(&id, "w-1".into());
    h.app.remote.worklogs.push(remote("w-1", "KAN-1", (date.year(), date.month(), date.day()), 3600));

    assert_eq!(row_ids(&h.app), vec!["w:w-1".to_string()], "the local row and the Jira row are the same worklog");
}

#[test]
fn a_row_marked_for_deletion_stops_counting_towards_the_staged_total() {
    let (mut h, date) = day_view("day-staged-total-delete");
    stage(&mut h.app, "keep", "KAN-1", date, "09:00", 3600);
    h.app.ledger.add(Entry {
        id: EntryId::new("drop".into()),
        issue_key: key("KAN-2"),
        date,
        start: StartTime::parse("10:00").unwrap(),
        seconds: 7200,
        title: "t".into(),
        state: EntryState::Deleted { worklog_id: "w-9".into() },
    });

    let m = day_model(&h.app);
    let rows = prepare_rows(&h.app, m);
    assert_eq!(staged_total(&rows), 3600, "time that is about to be removed is not time you are about to log");
}
