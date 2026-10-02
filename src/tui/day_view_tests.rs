//! Day view behaviour the spec spells out (R2, R4, R5b): leaving a cell saves
//! it, staging a search result watches the ticket, and what the prepare pane
//! and ticket pane actually list.

use super::action::Action;
use super::app::{App, Screen};
use super::features::day;
use super::features::day::rows::{RowKind, Section, prepare_rows, staged_total, ticket_rows};
use super::test_support::{Harness, harness, render, script_with_issues};
use crate::application::test_support::{issue, key, remote};
use crate::domain::{Entry, EntryId, EntryState, Issue, ParentRef, StartTime, StatusCategory};
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

fn day_view_with(name: &str, mine: Vec<Issue>) -> (Harness, NaiveDate) {
    let mut h = harness(name, script_with_issues(mine));
    let date = h.app.today;
    h.app.go_day(date);
    (h, date)
}

fn with_summary(mut i: Issue, summary: &str) -> Issue {
    i.summary = summary.to_string();
    i
}

fn with_subtasks(mut i: Issue, subtasks: Vec<Issue>) -> Issue {
    i.subtasks = subtasks;
    i
}

/// A sub-task: its own key/summary/status, with `parent` pointing at `parent_key`.
fn child_of(k: &str, summary: &str, parent_key: &str, parent_summary: &str) -> Issue {
    let mut i = with_summary(issue(k), summary);
    i.parent = Some(ParentRef { key: key(parent_key), summary: parent_summary.to_string() });
    i
}

fn subtask_of(k: &str, summary: &str, category: StatusCategory, parent_key: &str, parent_summary: &str) -> Issue {
    let mut i = child_of(k, summary, parent_key, parent_summary);
    i.status_category = category;
    i
}

/// Index of the first rendered row containing `needle`.
fn row_with(rows: &[String], needle: &str) -> usize {
    rows.iter().position(|r| r.contains(needle)).unwrap_or_else(|| panic!("no rendered row contains {needle:?}:\n{}", rows.join("\n")))
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

// ---- subtask grouping (phase 1) -------------------------------------------

#[test]
fn day_view_nests_child_under_parent() {
    let parent = with_summary(issue("KAN-12"), "Fix auth");
    let child = child_of("KAN-15", "Dev", "KAN-12", "Fix auth");
    let (mut h, _) = day_view_with("day-nest-child-under-parent", vec![parent, child]);

    let rows = render(&mut h.app);
    let p = row_with(&rows, "KAN-12");
    assert!(rows[p].contains("Fix auth"), "{:?}", rows[p]);
    assert!(rows[p].contains("▸1"), "the parent shows how many children it has: {:?}", rows[p]);
    let c = row_with(&rows, "KAN-15");
    assert!(c > p, "the child renders below its parent");
    assert!(rows[c].contains("↳"), "{:?}", rows[c]);
    assert!(rows[c].contains("Dev"), "{:?}", rows[c]);
}

#[test]
fn day_view_context_head_not_selectable() {
    let dev = child_of("KAN-15", "Dev", "KAN-12", "Fix auth");
    let (mut h, _) = day_view_with("day-nest-context-head", vec![dev]);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("KAN-12") && r.contains("Fix auth")), "the context head still renders:\n{}", rows.join("\n"));
    assert_eq!(ticket_keys(&h.app), vec!["KAN-15".to_string()], "the context head is not a selectable row");

    day_action(&mut h.app, day::Action::SelectTicket(0));
    day_action(&mut h.app, day::Action::QuickStage);
    assert_eq!(h.app.ledger.entries().len(), 1);
    assert_eq!(h.app.ledger.entries()[0].issue_key, key("KAN-15"), "Space stages the child, never the context head");
}

#[test]
fn day_view_filter_by_parent_summary() {
    let dev = child_of("KAN-15", "Dev", "KAN-12", "Fix auth");
    let other = with_summary(issue("KAN-99"), "Other");
    let (mut h, _) = day_view_with("day-nest-filter-parent", vec![dev, other]);

    day_action(&mut h.app, day::Action::FocusSearch);
    typed(&mut h.app, "auth");

    assert_eq!(ticket_keys(&h.app), vec!["KAN-15".to_string()], "only the child matches, via its parent's summary");
    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("Fix auth")), "the group head is shown with it:\n{}", rows.join("\n"));
    assert!(!rows.iter().any(|r| r.contains("KAN-99") || r.contains("Other")), "an unrelated issue is dropped:\n{}", rows.join("\n"));
}

#[test]
fn jira_section_expands_parent_subtasks() {
    let (mut h, _) = day_view("day-jira-subtasks");
    let parent = with_subtasks(
        with_summary(issue("KAN-12"), "Fix auth"),
        vec![
            subtask_of("KAN-15", "Dev", StatusCategory::Indeterminate, "KAN-12", "Fix auth"),
            subtask_of("KAN-16", "QA", StatusCategory::New, "KAN-12", "Fix auth"),
            subtask_of("KAN-17", "Review", StatusCategory::Done, "KAN-12", "Fix auth"),
        ],
    );
    with_search_results(&mut h.app, vec![parent]);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("─ jira (1) ")), "one top-level group:\n{}", rows.join("\n"));
    let p = row_with(&rows, "KAN-12");
    assert!(rows[p].contains("▸2"), "{:?}", rows[p]);
    assert!(rows.iter().any(|r| r.contains("↳") && r.contains("KAN-15")));
    assert!(rows.iter().any(|r| r.contains("↳") && r.contains("KAN-16")));
    assert!(!rows.iter().any(|r| r.contains("KAN-17")), "Done sub-tasks are excluded:\n{}", rows.join("\n"));
}

#[test]
fn jira_section_lists_subtasks_of_a_parent_already_in_mine() {
    // The common case: the card is assigned to me, its sub-tasks are not, so
    // search is the only way to reach them.
    let (mut h, _) = day_view_with("day-jira-parent-in-mine", vec![with_summary(issue("KAN-12"), "Fix auth")]);
    let parent = with_subtasks(
        with_summary(issue("KAN-12"), "Fix auth"),
        vec![subtask_of("KAN-15", "Dev", StatusCategory::New, "KAN-12", "Fix auth")],
    );
    with_search_results(&mut h.app, vec![parent]);

    let m = day_model(&h.app);
    let rows = ticket_rows(&h.app, m);
    let jira: Vec<String> = rows.iter().filter(|r| r.section == Section::Jira).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(jira, vec!["KAN-15"], "the parent stays in mine; its sub-task is reachable from the jira section");
    let rendered = render(&mut h.app);
    assert!(rendered.iter().any(|r| r.contains("↳") && r.contains("KAN-15")), "{}", rendered.join("\n"));
}

#[test]
fn jira_section_more_row_expands() {
    let (mut h, _) = day_view("day-jira-more");
    // Numbered from 11 so these keys never collide with `day_view`'s default
    // "mine" issues (KAN-1, KAN-2).
    let children: Vec<Issue> = (11..=17).map(|n| subtask_of(&format!("KAN-{n}"), &format!("Sub {n}"), StatusCategory::New, "KAN-100", "Parent")).collect();
    let parent = with_subtasks(with_summary(issue("KAN-100"), "Parent"), children);
    with_search_results(&mut h.app, vec![parent]);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("… +2 more")), "{}", rows.join("\n"));
    assert_eq!(rows.iter().filter(|r| r.contains("↳") && r.contains("KAN-")).count(), 5, "capped at 5 children");

    let m = day_model(&h.app);
    let more_idx = ticket_rows(&h.app, m).iter().position(|r| r.kind == RowKind::More).expect("a more row");
    day_action(&mut h.app, day::Action::SelectTicket(more_idx));
    day_action(&mut h.app, day::Action::OpenForm); // Enter on the "more" row expands it in place
    assert!(h.app.overlay.is_none(), "it never opens the stage popup");
    assert!(h.app.ledger.entries().is_empty(), "nothing on it stages anything");

    let rows = render(&mut h.app);
    assert!(!rows.iter().any(|r| r.contains("more")), "{}", rows.join("\n"));
    assert_eq!(rows.iter().filter(|r| r.contains("↳") && r.contains("KAN-")).count(), 7, "all seven now show");

    // changing the query resets the expansion back to collapsed
    day_action(&mut h.app, day::Action::FocusSearch);
    day_action(&mut h.app, day::Action::SearchChar('x'));
    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("more")), "typing collapses the group again:\n{}", rows.join("\n"));
}

#[test]
fn jira_section_subtask_hit_gets_context_head() {
    let (mut h, _) = day_view("day-jira-context");
    let hit = subtask_of("KAN-16", "QA", StatusCategory::New, "KAN-12", "Fix auth");
    with_search_results(&mut h.app, vec![hit]);

    let rows = render(&mut h.app);
    let head = row_with(&rows, "Fix auth");
    assert!(rows[head].contains("KAN-12"), "{:?}", rows[head]);
    assert!(rows[head + 1].contains("↳") && rows[head + 1].contains("KAN-16"), "{:?}", rows[head + 1]);

    let m = day_model(&h.app);
    let jira_keys: Vec<String> = ticket_rows(&h.app, m).into_iter().filter(|r| r.section == day::rows::Section::Jira).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(jira_keys, vec!["KAN-16".to_string()], "the context head is not a selectable row");
}

#[test]
fn staging_jira_subtask_watches_parent() {
    let (mut h, _) = day_view("day-jira-stage-child");
    let hit = subtask_of("KAN-15", "Dev", StatusCategory::Indeterminate, "KAN-12", "Fix auth");
    with_search_results(&mut h.app, vec![hit]);

    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-15").expect("the child row");
    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::QuickStage);

    assert_eq!(h.app.ledger.entries().len(), 1);
    assert_eq!(h.app.ledger.entries()[0].issue_key, key("KAN-15"), "the entry itself still logs on the sub-task");
    assert_eq!(h.app.ledger.watchlist(), &[key("KAN-12")], "only the parent is ever on the watchlist");

    let rows = render(&mut h.app);
    let watch_header = row_with(&rows, "─ watchlist");
    assert!(rows[watch_header + 1].contains("KAN-12") && rows[watch_header + 1].contains("Fix auth"), "real summary, not …: {:?}", rows[watch_header + 1]);
    assert!(rows[watch_header + 2].contains("↳") && rows[watch_header + 2].contains("KAN-15"), "{:?}", rows[watch_header + 2]);

    let r = row_with(&rows, "KAN-15");
    assert!(rows[r].contains("Dev · F"), "{:?}", rows[r]);
}

#[test]
fn w_on_jira_subtask_uses_search_parent_when_present() {
    let (mut h, _) = day_view("day-jira-w-uses-search-parent");
    let parent = with_subtasks(
        with_summary(issue("KAN-12"), "Fix auth"),
        vec![
            subtask_of("KAN-15", "Dev", StatusCategory::New, "KAN-12", "Fix auth"),
            subtask_of("KAN-16", "QA", StatusCategory::New, "KAN-12", "Fix auth"),
        ],
    );
    with_search_results(&mut h.app, vec![parent]);

    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-16").expect("the KAN-16 row");
    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::ToggleWatch);

    assert_eq!(h.app.ledger.watchlist(), &[key("KAN-12")]);
    let m = day_model(&h.app);
    let watch: Vec<String> = ticket_rows(&h.app, m).iter().filter(|r| r.section == Section::Watch).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(watch, vec!["KAN-12".to_string(), "KAN-15".to_string(), "KAN-16".to_string()], "both sub-tasks from the search parent ride along");
}

#[test]
fn w_on_subtask_watches_its_parent() {
    let parent = with_subtasks(
        with_summary(issue("KAN-6"), "Testing Task 2"),
        vec![subtask_of("KAN-7", "Dev", StatusCategory::New, "KAN-6", "Testing Task 2")],
    );
    let (mut h, _) = day_view_with("day-w-subtask-watches-parent", vec![parent]);

    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-7").expect("the nested child row");
    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::ToggleWatch);

    assert_eq!(h.app.ledger.watchlist(), &[key("KAN-6")]);
    let m = day_model(&h.app);
    let watch: Vec<String> = ticket_rows(&h.app, m).iter().filter(|r| r.section == Section::Watch).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(watch, vec!["KAN-6".to_string(), "KAN-7".to_string()]);
}

#[test]
fn w_on_subtask_of_watched_parent_unwatches_parent() {
    let parent = with_subtasks(
        with_summary(issue("KAN-6"), "Testing Task 2"),
        vec![subtask_of("KAN-7", "Dev", StatusCategory::New, "KAN-6", "Testing Task 2")],
    );
    let (mut h, _) = day_view_with("day-w-subtask-unwatches-parent", vec![parent]);
    h.app.ledger.watch(&key("KAN-6"));

    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-7").expect("the nested child row in the watch section");
    let m = day_model(&h.app);
    assert_eq!(ticket_rows(&h.app, m)[idx].section, Section::Watch, "KAN-7 rides along in the watch section");
    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::ToggleWatch);

    assert!(h.app.ledger.watchlist().is_empty());
    let m = day_model(&h.app);
    let mine: Vec<String> = ticket_rows(&h.app, m).iter().filter(|r| r.section == Section::Mine).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(mine, vec!["KAN-6".to_string(), "KAN-7".to_string()], "KAN-6 is back in mine, with KAN-7 still nested under it");
}

#[test]
fn w_on_legacy_watched_subtask_migrates_to_parent() {
    let (mut h, _) = day_view("day-w-legacy-migrate");
    h.app.ledger.watch(&key("KAN-15"));
    h.app.remote.upsert_watched(&child_of("KAN-15", "Dev", "KAN-12", "Fix auth"));

    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-15").expect("the legacy watched row");
    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::ToggleWatch);

    assert_eq!(h.app.ledger.watchlist(), &[key("KAN-12")], "the legacy entry on the child's own key is replaced by its parent");
}

#[test]
fn w_on_plain_issue_unchanged() {
    let (mut h, _) = day_view("day-w-plain-issue-unchanged");
    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-1").expect("a plain mine row");

    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::ToggleWatch);

    assert_eq!(h.app.ledger.watchlist(), &[key("KAN-1")]);
}

#[test]
fn prepare_title_shows_child_and_parent() {
    let (mut h, date) = day_view("day-prepare-title");
    h.app.remote.upsert_watched(&child_of("KAN-15", "Dev", "KAN-12", "Fix auth"));
    stage(&mut h.app, "e1", "KAN-15", date, "09:00", 3600);

    let rows = render(&mut h.app);
    let r = row_with(&rows, "KAN-15");
    // the column is narrow and truncates at the end, so only check the start:
    // "child · parent summary" (e.g. "Dev · Fix auth...").
    assert!(rows[r].contains("Dev · F"), "{:?}", rows[r]);
}

// ---- mine section expands assigned cards' own sub-tasks (phase 1b) --------

#[test]
fn mine_lists_unassigned_open_subtasks() {
    let parent = with_subtasks(
        with_summary(issue("KAN-6"), "Testing Task 2"),
        vec![
            subtask_of("KAN-7", "[DEV]", StatusCategory::Indeterminate, "KAN-6", "Testing Task 2"),
            subtask_of("KAN-8", "QA", StatusCategory::Done, "KAN-6", "Testing Task 2"),
        ],
    );
    let (mut h, _) = day_view_with("day-mine-subtasks", vec![parent]);

    let rows = render(&mut h.app);
    let p = row_with(&rows, "KAN-6");
    assert!(rows[p].contains("Testing Task 2"), "{:?}", rows[p]);
    assert!(rows[p].contains("▸1"), "{:?}", rows[p]);
    let c = row_with(&rows, "KAN-7");
    assert!(c > p, "the child renders below its parent");
    assert!(rows[c].contains("↳") && rows[c].contains("[DEV]"), "{:?}", rows[c]);
    assert!(!rows.iter().any(|r| r.contains("KAN-8")), "the Done sub-task is excluded:\n{}", rows.join("\n"));

    let m = day_model(&h.app);
    let mine_keys: Vec<String> = ticket_rows(&h.app, m).iter().filter(|r| r.section == Section::Mine).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(mine_keys, vec!["KAN-6".to_string(), "KAN-7".to_string()]);
}

#[test]
fn mine_subtask_listed_once() {
    // KAN-7 shows up three ways: nested under KAN-6, as its own top-level
    // "mine" hit, and in the history search — only one row must survive.
    let nested_under = subtask_of("KAN-7", "[DEV]", StatusCategory::Indeterminate, "KAN-6", "summary of KAN-6");
    let parent = with_subtasks(issue("KAN-6"), vec![nested_under.clone()]);
    let script = crate::application::test_support::Script {
        searches: std::collections::VecDeque::from(vec![Ok(vec![parent, nested_under.clone()]), Ok(vec![nested_under])]),
        ..Default::default()
    };
    let mut h = harness("day-mine-subtask-once", script);
    let date = h.app.today;
    h.app.go_day(date);

    let m = day_model(&h.app);
    let keys: Vec<String> = ticket_rows(&h.app, m).iter().map(|r| r.issue.key.to_string()).collect();
    assert_eq!(keys.iter().filter(|k| k.as_str() == "KAN-7").count(), 1, "KAN-7 appears exactly once: {keys:?}");

    let rows = render(&mut h.app);
    let p = row_with(&rows, "KAN-6");
    let c = row_with(&rows, "KAN-7");
    assert!(c > p, "KAN-7 renders under KAN-6 in mine");
    assert!(rows[c].contains("↳"), "{:?}", rows[c]);
}

#[test]
fn staging_mine_subtask_does_not_watch() {
    let parent = with_subtasks(issue("KAN-6"), vec![subtask_of("KAN-7", "Dev", StatusCategory::Indeterminate, "KAN-6", "summary of KAN-6")]);
    let (mut h, _) = day_view_with("day-mine-subtask-stage", vec![parent]);

    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-7").expect("the nested child row");
    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::QuickStage);

    assert_eq!(h.app.ledger.entries().len(), 1);
    assert_eq!(h.app.ledger.entries()[0].issue_key, key("KAN-7"));
    assert!(h.app.ledger.watchlist().is_empty(), "a mine sub-task is not auto-watched, same as any mine row");
}

#[test]
fn prepare_title_for_nested_mine_subtask() {
    let parent = with_subtasks(
        with_summary(issue("KAN-6"), "Testing Task 2"),
        vec![subtask_of("KAN-7", "[DEV]", StatusCategory::Indeterminate, "KAN-6", "Testing Task 2")],
    );
    let (mut h, date) = day_view_with("day-mine-subtask-prepare-title", vec![parent]);
    // KAN-7 is known only via KAN-6's `subtasks` — it is never in `remote.watched`.
    stage(&mut h.app, "e1", "KAN-7", date, "09:00", 3600);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("[DEV] · T")), "{}", rows.join("\n"));
}

#[test]
fn mine_subtasks_capped_with_more_row() {
    let children: Vec<Issue> = (11..=17).map(|n| subtask_of(&format!("KAN-{n}"), &format!("Sub {n}"), StatusCategory::New, "KAN-100", "Parent")).collect();
    let parent = with_subtasks(with_summary(issue("KAN-100"), "Parent"), children);
    let (mut h, _) = day_view_with("day-mine-subtasks-more", vec![parent]);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("… +2 more")), "{}", rows.join("\n"));
    assert_eq!(rows.iter().filter(|r| r.contains("↳") && r.contains("KAN-")).count(), 5, "capped at 5 children");

    let m = day_model(&h.app);
    let more_idx = ticket_rows(&h.app, m).iter().position(|r| r.kind == RowKind::More).expect("a more row");
    day_action(&mut h.app, day::Action::SelectTicket(more_idx));
    day_action(&mut h.app, day::Action::OpenForm); // Enter on the "more" row expands it in place
    assert!(h.app.overlay.is_none(), "it never opens the stage popup");
    assert!(h.app.ledger.entries().is_empty(), "nothing on it stages anything");

    let rows = render(&mut h.app);
    assert!(!rows.iter().any(|r| r.contains("more")), "{}", rows.join("\n"));
    assert_eq!(rows.iter().filter(|r| r.contains("↳") && r.contains("KAN-")).count(), 7, "all seven now show");
}

#[test]
fn history_does_not_expand_subtasks() {
    let h1 = with_subtasks(
        with_summary(issue("H-1"), "Logged already"),
        vec![subtask_of("H-2", "Open child", StatusCategory::New, "H-1", "Logged already")],
    );
    let script = crate::application::test_support::Script {
        searches: std::collections::VecDeque::from(vec![Ok(Vec::new()), Ok(vec![h1])]),
        ..Default::default()
    };
    let mut h = harness("day-history-no-subtasks", script);
    let date = h.app.today;
    h.app.go_day(date);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("H-1")), "{}", rows.join("\n"));
    assert!(!rows.iter().any(|r| r.contains("H-2")), "history never expands sub-tasks:\n{}", rows.join("\n"));

    let m = day_model(&h.app);
    let keys: Vec<String> = ticket_rows(&h.app, m).iter().map(|r| r.issue.key.to_string()).collect();
    assert!(!keys.contains(&"H-2".to_string()));
}

#[test]
fn a_mine_card_hidden_by_the_filter_can_still_come_back_as_a_search_hit() {
    // Jira's text search also matches descriptions; the local filter only sees
    // key and summary. A card filtered out of mine must not be swallowed.
    let (mut h, _) = day_view_with("day-mine-filtered-search-hit", vec![with_summary(issue("KAN-6"), "Testing Task 2")]);
    typed(&mut h.app, "/zzz");
    with_search_results(&mut h.app, vec![with_summary(issue("KAN-6"), "Testing Task 2")]);

    let m = day_model(&h.app);
    let jira: Vec<String> = ticket_rows(&h.app, m).iter().filter(|r| r.section == Section::Jira).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(jira, vec!["KAN-6"]);
}

#[test]
fn a_watched_assigned_card_still_lists_its_open_subtasks() {
    let parent = with_subtasks(
        with_summary(issue("KAN-6"), "Testing Task 2"),
        vec![subtask_of("KAN-7", "[DEV]", StatusCategory::New, "KAN-6", "Testing Task 2")],
    );
    let (mut h, _) = day_view_with("day-watched-mine-subtasks", vec![parent]);
    h.app.ledger.watch(&key("KAN-6"));

    let m = day_model(&h.app);
    let watch: Vec<String> = ticket_rows(&h.app, m).iter().filter(|r| r.section == Section::Watch).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(watch, vec!["KAN-6", "KAN-7"], "the sub-task rides along with its card in the watchlist section");
}

// ---- watchlist = parents only, with their sub-tasks (phase 1c) -----------

#[test]
fn watchlist_shows_open_subtasks_of_watched_card() {
    // OPS-7 is not assigned to me — its subtasks only reach the watchlist
    // through `get_issue`'s own `subtasks` field, stashed in `remote.watched`.
    let (mut h, _) = day_view("day-watch-not-assigned-subtasks");
    h.app.ledger.watch(&key("OPS-7"));
    h.app.remote.upsert_watched(&with_subtasks(
        with_summary(issue("OPS-7"), "Ops card"),
        vec![
            subtask_of("OPS-8", "Dev", StatusCategory::Indeterminate, "OPS-7", "Ops card"),
            subtask_of("OPS-9", "QA", StatusCategory::Done, "OPS-7", "Ops card"),
        ],
    ));

    let m = day_model(&h.app);
    let watch: Vec<String> = ticket_rows(&h.app, m).iter().filter(|r| r.section == Section::Watch).map(|r| r.issue.key.to_string()).collect();
    assert_eq!(watch, vec!["OPS-7".to_string(), "OPS-8".to_string()], "OPS-9 is Done and stays out");

    let rows = render(&mut h.app);
    let c = row_with(&rows, "OPS-8");
    assert!(rows[c].contains("↳"), "{:?}", rows[c]);
}

// ---- R2: parent cards stay stageable (decided 2026-10-02) -----------------

#[test]
fn a_parent_card_with_subtasks_stages_on_the_parent_itself() {
    let parent = with_subtasks(
        with_summary(issue("KAN-6"), "Testing Task 2"),
        vec![subtask_of("KAN-7", "[DEV]", StatusCategory::New, "KAN-6", "Testing Task 2")],
    );
    let (mut h, date) = day_view_with("day-stage-parent-mine", vec![parent]);
    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-6").expect("the parent row");

    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::QuickStage);

    let entries = h.app.ledger.entries_on(date);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].issue_key, key("KAN-6"), "staging is not redirected to a sub-task");
}

#[test]
fn a_parent_search_result_stages_on_the_parent_and_watches_it() {
    let (mut h, date) = day_view("day-stage-parent-jira");
    let parent = with_subtasks(
        with_summary(issue("KAN-12"), "Fix auth"),
        vec![subtask_of("KAN-15", "Dev", StatusCategory::New, "KAN-12", "Fix auth")],
    );
    with_search_results(&mut h.app, vec![parent]);
    let idx = ticket_keys(&h.app).iter().position(|k| k == "KAN-12").expect("the parent row");

    day_action(&mut h.app, day::Action::SelectTicket(idx));
    day_action(&mut h.app, day::Action::QuickStage);

    let entries = h.app.ledger.entries_on(date);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].issue_key, key("KAN-12"));
    assert_eq!(h.app.ledger.watchlist(), &[key("KAN-12")]);
}
