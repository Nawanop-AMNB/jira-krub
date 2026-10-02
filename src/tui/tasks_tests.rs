//! R17: the main screen is two tabs (My Tasks | Worklogs). These tests render
//! real frames, so they also pin the column layout the spec asks for.

use super::app::{App, MainTab, Screen, StatusKind};
use super::features::tasks;
use super::test_support::{drain_until_nonempty, harness, render, render_styles, script_with_issues, settle};
use super::widgets::text::display_width;
use crate::application::test_support::{issue, key};
use crate::domain::{Issue, ParentRef, StatusCategory};
use chrono::Days;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Modifier;

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

fn click(app: &mut App, column: u16, row: u16) {
    app.on_mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers: KeyModifiers::NONE });
}

/// An issue with an explicit status name and category.
fn task(k: &str, status: &str, category: StatusCategory) -> Issue {
    let mut i = issue(k);
    i.status = status.to_string();
    i.status_category = category;
    i
}

fn with_summary(mut i: Issue, summary: &str) -> Issue {
    i.summary = summary.to_string();
    i
}

fn tasks_model(app: &App) -> &tasks::Model {
    match &app.screen {
        Screen::Tasks(m) => m,
        _ => panic!("not on the tasks screen"),
    }
}

/// Keys of the issues currently listed, in render order.
fn visible_keys(app: &App) -> Vec<String> {
    tasks::visible_issues(app, tasks_model(app)).iter().map(|i| i.key.to_string()).collect()
}

/// 3 In Progress, 1 In Review, 4 To Do — the spec's worked example.
fn eight_tasks() -> Vec<Issue> {
    let mut out = vec![
        task("KAN-1", "In Progress", StatusCategory::Indeterminate),
        task("KAN-2", "In Progress", StatusCategory::Indeterminate),
        task("KAN-3", "In Progress", StatusCategory::Indeterminate),
        task("KAN-4", "In Review", StatusCategory::Indeterminate),
    ];
    for n in 5..9 {
        out.push(task(&format!("KAN-{n}"), "To Do", StatusCategory::New));
    }
    out
}

/// Index of the first rendered row containing `needle`.
fn row_with(rows: &[String], needle: &str) -> usize {
    rows.iter().position(|r| r.contains(needle)).unwrap_or_else(|| panic!("no rendered row contains {needle:?}:\n{}", rows.join("\n")))
}

/// Column (not byte offset) where `needle` starts on a rendered row.
fn column_of(row: &str, needle: &str) -> usize {
    let at = row.find(needle).unwrap_or_else(|| panic!("{needle:?} not in {row:?}"));
    display_width(&row[..at])
}

// ---- U1 / U2: the tab strip ------------------------------------------------

#[test]
fn startup_shows_my_tasks_tab() {
    let mut h = harness("tasks-startup", script_with_issues(vec![issue("KAN-1")]));
    assert!(matches!(h.app.screen, Screen::Tasks(_)), "My tasks is the startup tab");
    assert_eq!(h.app.main_tab, MainTab::Tasks);

    let rows = render(&mut h.app);
    assert!(rows[0].contains("Worklogs"), "row 0: {:?}", rows[0]);
    assert!(rows[0].contains("My Tasks"), "row 0: {:?}", rows[0]);
    assert!(column_of(&rows[0], "My Tasks") < column_of(&rows[0], "Worklogs"), "My Tasks is the first chip");

    // the active chip is drawn in title style, the inactive one dim
    let styles = render_styles(&mut h.app);
    let at = |needle: &str| styles[0][column_of(&rows[0], needle)];
    let title = crate::tui::theme::title();
    assert_eq!((at("My Tasks").fg, at("My Tasks").bg), (title.fg, title.bg), "the open tab is highlighted");
    assert!(at("My Tasks").add_modifier.contains(Modifier::BOLD));
    assert!(at("Worklogs").add_modifier.contains(Modifier::DIM), "the other tab is dim");
    assert!(!at("Worklogs").add_modifier.contains(Modifier::BOLD));
}

#[test]
fn tab_cycles_and_esc_is_inert() {
    let mut h = harness("tasks-tab-cycle", script_with_issues(vec![issue("KAN-1")]));
    press(&mut h.app, KeyCode::Tab);
    assert!(matches!(h.app.screen, Screen::Week(_)), "Tab from My tasks opens Worklog");
    press(&mut h.app, KeyCode::Esc);
    assert!(matches!(h.app.screen, Screen::Week(_)), "Esc does nothing on Worklog");

    press(&mut h.app, KeyCode::Tab);
    assert!(matches!(h.app.screen, Screen::Tasks(_)), "Tab again returns to My tasks");
    press(&mut h.app, KeyCode::Esc);
    assert!(matches!(h.app.screen, Screen::Tasks(_)), "Esc does nothing on My tasks");
}

// ---- U3 / U4: grouping and the cursor ---------------------------------------

#[test]
fn groups_render_in_order_with_counts_and_cursor_on_first_row() {
    let mut h = harness("tasks-groups", script_with_issues(eight_tasks()));
    let rows = render(&mut h.app);

    let ip = row_with(&rows, "─ In Progress (assigned to me: 3)");
    let ir = row_with(&rows, "─ In Review (assigned to me: 1)");
    let td = row_with(&rows, "─ To Do (assigned to me: 4)");
    assert!(ip < ir && ir < td, "indeterminate before new, then alphabetical: {ip} {ir} {td}");

    let marked: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| r.contains('▶')).map(|(i, _)| i).collect();
    assert_eq!(marked, vec![ip + 1], "the cursor sits on the first In Progress row and nowhere else");
    assert!(rows[ip + 1].contains("KAN-1"), "{:?}", rows[ip + 1]);
}

#[test]
fn down_skips_group_header() {
    let mut h = harness("tasks-down", script_with_issues(eight_tasks()));
    press(&mut h.app, KeyCode::Down);
    press(&mut h.app, KeyCode::Down);
    assert_eq!(visible_keys(&h.app)[tasks_model(&h.app).selected], "KAN-3", "last In Progress row");

    press(&mut h.app, KeyCode::Down);
    assert_eq!(visible_keys(&h.app)[tasks_model(&h.app).selected], "KAN-4", "↓ crosses the In Review header without landing on it");

    let rows = render(&mut h.app);
    let marked = row_with(&rows, "▶");
    assert!(rows[marked].contains("KAN-4"));
    assert!(rows[marked - 1].contains("─ In Review (assigned to me: 1)"), "the header is right above, unselected");
}

// ---- U5: the filter box -----------------------------------------------------

#[test]
fn up_from_first_row_enters_filter_and_hides_cursor() {
    let mine = vec![
        with_summary(task("KAN-1", "In Progress", StatusCategory::Indeterminate), "Fix auth token expiry"),
        with_summary(task("KAN-2", "In Progress", StatusCategory::Indeterminate), "Refactor worklog sync"),
        with_summary(task("KAN-5", "To Do", StatusCategory::New), "Migrate config"),
    ];
    let mut h = harness("tasks-filter", script_with_issues(mine));

    press(&mut h.app, KeyCode::Up);
    assert!(tasks_model(&h.app).filter_focused, "↑ from the first row walks into the filter box");
    let rows = render(&mut h.app);
    assert!(!rows.iter().any(|r| r.contains('▶')), "no row cursor while the box is focused");

    typed(&mut h.app, "auth");
    assert_eq!(visible_keys(&h.app), vec!["KAN-1"]);
    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("KAN-1")));
    assert!(!rows.iter().any(|r| r.contains("KAN-2")), "non-matching rows are gone");
    assert!(!rows.iter().any(|r| r.contains("─ To Do")), "a group with no match loses its header too");

    ctrl(&mut h.app, 'u');
    assert_eq!(visible_keys(&h.app), vec!["KAN-1", "KAN-2", "KAN-5"], "Ctrl+U restores every row");
    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("─ To Do (assigned to me: 1)")));
}

// ---- U6 / U7: opening in the browser ----------------------------------------

#[test]
fn enter_opens_issue_in_browser() {
    let mut h = harness("tasks-open", script_with_issues(vec![issue("KAN-12"), issue("KAN-13")]));
    assert_eq!(visible_keys(&h.app)[tasks_model(&h.app).selected], "KAN-12");
    press(&mut h.app, KeyCode::Enter);
    assert_eq!(h.opener.urls(), vec!["https://acme.atlassian.net/browse/KAN-12".to_string()]);
    let status = h.app.status.as_ref().expect("a status line");
    assert!(status.text.contains("opened KAN-12"), "{:?}", status.text);
}

#[test]
fn opener_failure_shows_error_status() {
    let mut h = harness("tasks-open-fails", script_with_issues(vec![issue("KAN-12")]));
    h.opener.set_fail("boom");
    press(&mut h.app, KeyCode::Enter);
    let status = h.app.status.as_ref().expect("a status line");
    assert!(status.kind == StatusKind::Error, "the failure is an error, not a note");
    assert!(status.text.contains("could not open browser: boom"), "{:?}", status.text);
    assert!(h.opener.urls().is_empty());
}

// ---- U8: this tab never logs work -------------------------------------------

#[test]
fn logging_keys_do_nothing_on_tasks() {
    let mut h = harness("tasks-inert-keys", script_with_issues(vec![issue("KAN-1")]));
    let jql_before = h.gateway.calls().jql.len();
    let entries_before = h.app.ledger.entries().len();

    for code in [KeyCode::Char(' '), KeyCode::Char('w'), KeyCode::Char('a'), KeyCode::Char('p'), KeyCode::Char('['), KeyCode::Char(']'), KeyCode::Left, KeyCode::Right] {
        press(&mut h.app, code);
    }

    assert!(h.app.overlay.is_none(), "no popup opened");
    assert_eq!(h.app.ledger.staged_count(), 0, "nothing was staged");
    assert_eq!(h.app.ledger.entries().len(), entries_before);
    assert!(matches!(h.app.screen, Screen::Tasks(_)), "and we never left the tab");
    assert_eq!(h.gateway.calls().jql.len(), jql_before, "no push or sync was triggered");
}

// ---- U9: empty state ---------------------------------------------------------

#[test]
fn empty_state_text() {
    let mut h = harness("tasks-empty", script_with_issues(Vec::new()));
    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("nothing assigned to you · r sync")), "{}", rows.join("\n"));
}

// ---- U10: fixed columns (R9c) -------------------------------------------------

#[test]
fn due_and_updated_columns_align_with_and_without_due() {
    let today = chrono::Local::now().date_naive();
    let mut a = with_summary(task("KAN-1", "To Do", StatusCategory::New), "Fix auth token expiry");
    a.due = Some(today - Days::new(1));
    a.updated = today;
    let mut b = with_summary(task("KAN-2", "To Do", StatusCategory::New), "No deadline on this one");
    b.updated = today;
    let mut c = with_summary(task("KAN-3", "To Do", StatusCategory::New), "แก้ไขระบบล็อกอิน");
    c.updated = today;

    let mut h = harness("tasks-columns", script_with_issues(vec![a, b, c]));
    let rows = render(&mut h.app);

    let ra = &rows[row_with(&rows, "KAN-1")];
    let rb = &rows[row_with(&rows, "KAN-2")];
    let rc = &rows[row_with(&rows, "KAN-3")];
    assert!(ra.contains("overdue 1d"), "{ra:?}");
    assert!(!rb.contains("due ") && !rb.contains("overdue"), "no due date renders blank: {rb:?}");

    let cols: Vec<usize> = [ra, rb, rc].iter().map(|r| column_of(r, "updated")).collect();
    assert_eq!(cols[0], cols[1], "the updated column does not move when due is missing");
    assert_eq!(cols[0], cols[2], "nor when the summary is Thai");
}

// ---- U11: r re-syncs ----------------------------------------------------------

#[test]
fn r_on_tasks_resyncs() {
    let mut h = harness("tasks-resync", script_with_issues(vec![issue("KAN-1")]));
    let before = h.gateway.calls().jql.len();
    press(&mut h.app, KeyCode::Char('r'));
    settle(&mut h.app);
    assert!(h.gateway.calls().jql.len() > before, "r starts another sync");
}

// ---- U12 / U16: coming back from Settings and the day view ---------------------

#[test]
fn settings_returns_to_the_tab_it_came_from() {
    let mut h = harness("tasks-settings-worklog", script_with_issues(Vec::new()));
    press(&mut h.app, KeyCode::Tab);
    assert!(matches!(h.app.screen, Screen::Week(_)));
    press(&mut h.app, KeyCode::Char(','));
    assert!(matches!(h.app.screen, Screen::Settings(_)));
    press(&mut h.app, KeyCode::Esc);
    assert!(matches!(h.app.screen, Screen::Week(_)), "Settings returns to Worklog");

    let mut h = harness("tasks-settings-tasks", script_with_issues(Vec::new()));
    press(&mut h.app, KeyCode::Char(','));
    assert!(matches!(h.app.screen, Screen::Settings(_)));
    press(&mut h.app, KeyCode::Esc);
    assert!(matches!(h.app.screen, Screen::Tasks(_)), "Settings returns to My tasks");
}

#[test]
fn day_view_esc_returns_to_worklog() {
    let mut h = harness("tasks-day-esc", script_with_issues(Vec::new()));
    press(&mut h.app, KeyCode::Tab);
    press(&mut h.app, KeyCode::Enter);
    assert!(matches!(h.app.screen, Screen::Day(_)), "Enter opens the selected day");
    press(&mut h.app, KeyCode::Esc);
    assert!(matches!(h.app.screen, Screen::Week(_)), "the day view always steps back to Worklog");
}

// ---- U13: the global staged warning is gone (R1) --------------------------------

#[test]
fn week_header_has_no_global_staged_warning() {
    let mut h = harness("tasks-no-staged-warning", script_with_issues(vec![issue("KAN-1")]));
    let today = h.app.today;
    h.app.ledger.quick_stage(crate::domain::EntryId::new("e1".into()), key("KAN-1"), today, crate::domain::StartTime::parse("09:00").unwrap(), 3600);
    press(&mut h.app, KeyCode::Tab);

    let rows = render(&mut h.app);
    assert!(!rows.iter().any(|r| r.contains("staged not pushed")), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("staged")), "the per-day `+Xh staged` text stays");
}

// ---- U14: the mouse -------------------------------------------------------------

#[test]
fn click_tab_label_switches_and_double_click_row_opens() {
    let mut h = harness("tasks-mouse", script_with_issues(vec![issue("KAN-12"), issue("KAN-13")]));
    let rows = render(&mut h.app);
    click(&mut h.app, column_of(&rows[0], "Worklogs") as u16, 0);
    assert!(matches!(h.app.screen, Screen::Week(_)), "clicking a tab label switches tab");

    press(&mut h.app, KeyCode::Tab);
    let rows = render(&mut h.app);
    let y = row_with(&rows, "KAN-13");
    let x = column_of(&rows[y], "KAN-13") as u16;
    click(&mut h.app, x, y as u16);
    click(&mut h.app, x, y as u16);
    assert_eq!(h.opener.urls(), vec!["https://acme.atlassian.net/browse/KAN-13".to_string()], "double-click opens the row it is on");
}

// ---- streamed sync: My Tasks renders from the partial message -----------------

#[test]
fn my_tasks_renders_from_the_partial_message_while_still_syncing() {
    // harness() runs and settles one sync during App::new; queue the same
    // two search responses again so the `r`-triggered sync below also sees
    // the two mine issues instead of the fake's empty default.
    let mine = vec![issue("KAN-1"), issue("KAN-2")];
    let script = crate::application::test_support::Script {
        searches: std::collections::VecDeque::from(vec![
            Ok(mine.clone()),
            Ok(Vec::new()),
            Ok(mine),
            Ok(Vec::new()),
        ]),
        ..Default::default()
    };
    let mut h = harness("tasks-partial-render", script);
    // harness() already settled the startup sync; trigger a fresh one so we
    // can catch it mid-flight.
    press(&mut h.app, KeyCode::Char('r'));
    assert!(h.app.remote.syncing);

    let mut msgs = drain_until_nonempty(&mut h.app);
    let first = msgs.remove(0);
    h.app.on_msg(first);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("KAN-1")), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("KAN-2")), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("syncing…")), "{}", rows.join("\n"));
    assert!(h.app.remote.syncing, "still syncing after only the partial message");
    assert!(h.app.remote.worklogs.is_empty(), "worklogs are not touched until Synced");

    for m in msgs {
        h.app.on_msg(m);
    }
    settle(&mut h.app);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("my tasks · 2 ")), "{}", rows.join("\n"));
    assert!(!rows.iter().any(|r| r.contains("syncing…")), "{}", rows.join("\n"));
    assert!(!h.app.remote.syncing);
}

#[test]
fn partial_message_does_not_touch_the_ledger() {
    let mut h = harness("tasks-partial-ledger", script_with_issues(vec![issue("KAN-1")]));
    let today = h.app.today;
    h.app.ledger.quick_stage(
        crate::domain::EntryId::new("e1".into()),
        key("KAN-1"),
        today,
        crate::domain::StartTime::parse("09:00").unwrap(),
        3600,
    );
    h.app.save_ledger();

    let ledger_before = h.app.ledger.clone();
    let state_before = std::fs::read(h.state_file()).expect("state file exists");

    press(&mut h.app, KeyCode::Char('r'));
    let mut msgs = drain_until_nonempty(&mut h.app);
    let first = msgs.remove(0);
    h.app.on_msg(first);

    assert_eq!(h.app.ledger, ledger_before, "the partial message must not touch the ledger");
    let state_after = std::fs::read(h.state_file()).expect("state file exists");
    assert_eq!(state_after, state_before, "the state file must not be rewritten by the partial message");

    for m in msgs {
        h.app.on_msg(m);
    }
    settle(&mut h.app);
}

#[test]
fn empty_state_waits_for_the_full_sync() {
    let mut h = harness("tasks-empty-partial", script_with_issues(Vec::new()));
    press(&mut h.app, KeyCode::Char('r'));

    let mut msgs = drain_until_nonempty(&mut h.app);
    let first = msgs.remove(0);
    h.app.on_msg(first);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("syncing…")), "{}", rows.join("\n"));
    assert!(!rows.iter().any(|r| r.contains("nothing assigned to you")), "{}", rows.join("\n"));

    for m in msgs {
        h.app.on_msg(m);
    }
    settle(&mut h.app);

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("nothing assigned to you · r sync")), "{}", rows.join("\n"));
}

// ---- subtask grouping (phase 1) -------------------------------------------

/// A sub-task: its own key/summary/status, with `parent` pointing at `parent_key`.
fn child_task(k: &str, summary: &str, status: &str, category: StatusCategory, parent_key: &str, parent_summary: &str) -> Issue {
    let mut i = task(k, status, category);
    i.summary = summary.to_string();
    i.parent = Some(ParentRef { key: key(parent_key), summary: parent_summary.to_string() });
    i
}

#[test]
fn my_tasks_nests_children() {
    let parent = with_summary(task("KAN-12", "In Progress", StatusCategory::Indeterminate), "Fix auth");
    let child = child_task("KAN-15", "Dev", "In Progress", StatusCategory::Indeterminate, "KAN-12", "Fix auth");
    let mut h = harness("tasks-nest-children", script_with_issues(vec![parent, child]));

    assert_eq!(visible_keys(&h.app), vec!["KAN-12", "KAN-15"], "the child follows its parent");
    let rows = render(&mut h.app);
    let p = row_with(&rows, "KAN-12");
    assert!(rows[p].contains("▸1"), "{:?}", rows[p]);
    let c = row_with(&rows, "KAN-15");
    assert!(c > p);
    assert!(rows[c].contains("↳"), "{:?}", rows[c]);

    press(&mut h.app, KeyCode::Down);
    assert_eq!(visible_keys(&h.app)[tasks_model(&h.app).selected], "KAN-15", "Down from the parent selects the child");

    press(&mut h.app, KeyCode::Enter);
    assert_eq!(h.opener.urls(), vec!["https://acme.atlassian.net/browse/KAN-15".to_string()], "Enter on a child opens the child's URL");
}

#[test]
fn epic_children_not_nested() {
    // An issue whose parent is an Epic is never a sub-task — `parent` stays
    // `None` (filtered out at parse time, R2/R4), so it renders plainly.
    let story = with_summary(task("KAN-20", "To Do", StatusCategory::New), "A story under an epic");
    assert_eq!(story.parent, None);
    let mut h = harness("tasks-epic-not-nested", script_with_issues(vec![story]));

    assert_eq!(visible_keys(&h.app), vec!["KAN-20"]);
    let rows = render(&mut h.app);
    let r = row_with(&rows, "KAN-20");
    assert!(!rows[r].contains("↳"), "{:?}", rows[r]);
    assert!(!rows[r].contains('▸'), "a plain issue has no child badge: {:?}", rows[r]);
}

// ---- My Tasks expands assigned cards' own sub-tasks (phase 1b) ------------

#[test]
fn my_tasks_lists_unassigned_open_subtasks() {
    let mut parent = with_summary(task("KAN-6", "In Progress", StatusCategory::Indeterminate), "Testing Task 2");
    parent.subtasks = vec![
        child_task("KAN-7", "[DEV]", "To Do", StatusCategory::New, "KAN-6", "Testing Task 2"),
        child_task("KAN-8", "QA", "Done", StatusCategory::Done, "KAN-6", "Testing Task 2"),
    ];
    let mut h = harness("tasks-mine-subtasks", script_with_issues(vec![parent]));

    assert_eq!(visible_keys(&h.app), vec!["KAN-6", "KAN-7"], "KAN-8 is done; KAN-7 nests under its parent despite its own status");
    let rows = render(&mut h.app);
    let p = row_with(&rows, "KAN-6");
    assert!(rows[p].contains("▸1"), "{:?}", rows[p]);
    let c = row_with(&rows, "KAN-7");
    assert!(c > p);
    assert!(rows[c].contains("↳"), "{:?}", rows[c]);
    assert!(rows.iter().any(|r| r.contains("─ In Progress")), "{}", rows.join("\n"));
    assert!(!rows.iter().any(|r| r.contains("─ To Do")), "KAN-7's own status never creates a separate group:\n{}", rows.join("\n"));

    press(&mut h.app, KeyCode::Down);
    assert_eq!(visible_keys(&h.app)[tasks_model(&h.app).selected], "KAN-7", "Down from the parent selects the nested child");

    press(&mut h.app, KeyCode::Enter);
    assert_eq!(h.opener.urls(), vec!["https://acme.atlassian.net/browse/KAN-7".to_string()], "Enter on a nested child opens the child's URL");

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("my tasks · 1 ")), "the box title counts only the assigned card, not its sub-task:\n{}", rows.join("\n"));
}

#[test]
fn a_subtask_without_an_updated_date_shows_no_updated_label() {
    // Jira's `subtasks` field carries no `updated`; "updated 20000d ago" would be a lie.
    let mut parent = with_summary(task("KAN-6", "In Progress", StatusCategory::Indeterminate), "Testing Task 2");
    parent.subtasks = vec![child_task("KAN-7", "[DEV]", "To Do", StatusCategory::New, "KAN-6", "Testing Task 2")];
    let mut h = harness("tasks-subtask-no-updated", script_with_issues(vec![parent]));

    let rows = render(&mut h.app);
    let c = row_with(&rows, "KAN-7");
    assert!(!rows[c].contains("updated"), "{:?}", rows[c]);
}

#[test]
fn group_header_counts_only_issues_assigned_to_me() {
    // KAN-7 rides along under KAN-6 but is not assigned to me.
    let mut parent = with_summary(task("KAN-6", "In Progress", StatusCategory::Indeterminate), "Testing Task 2");
    parent.subtasks = vec![child_task("KAN-7", "[DEV]", "To Do", StatusCategory::New, "KAN-6", "Testing Task 2")];
    let other = task("KAN-2", "In Progress", StatusCategory::Indeterminate);
    let mut h = harness("tasks-group-assigned-count", script_with_issues(vec![parent, other]));

    let rows = render(&mut h.app);
    assert!(rows.iter().any(|r| r.contains("─ In Progress (assigned to me: 2) ")), "{}", rows.join("\n"));
}
