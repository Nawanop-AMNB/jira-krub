use super::model::{Action, Cell, CellEdit, Model, Pane};
use super::rows::{PrepareRow, Section, prepare_rows, ticket_rows};
use crate::application::ids::new_entry_id;
use crate::application::use_cases::search;
use crate::domain::{Issue, IssueKey, StartTime, Week, duration};
use crate::tui::action::{Action as Global, PushScope};
use crate::tui::app::{App, Screen};
use crate::tui::features::entry_form;
use crate::tui::msg::Msg;
use crate::tui::widgets::TextInput;
use chrono::Days;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(300);

/// Deferred mutation applied to an entry once its cell text parses.
type Apply = Box<dyn FnOnce(&mut crate::domain::Entry)>;

pub fn keys(m: &Model, key: &KeyEvent) -> Option<Global> {
    use Action::*;
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    if let Some(edit) = &m.edit {
        let stepping = matches!(edit.cell, Cell::Start | Cell::Duration);
        let newline_combo = matches!(key.code, KeyCode::Enter) && (ctrl || shift || key.modifiers.contains(KeyModifiers::ALT));
        let a = match key.code {
            KeyCode::Esc => CellCancel,
            KeyCode::Enter if edit.cell == Cell::Title && newline_combo => CellNewline,
            KeyCode::Char('j') if edit.cell == Cell::Title && ctrl => CellNewline,
            // Enter walks start → duration → description, then leaves edit mode.
            KeyCode::Enter => CellNext,
            KeyCode::Up if stepping => CellStep(if shift { 60 } else { 15 }),
            KeyCode::Down if stepping => CellStep(if shift { -60 } else { -15 }),
            KeyCode::Left => CellLeft,
            KeyCode::Right => CellRight,
            KeyCode::Home => CellHome,
            KeyCode::End => CellEnd,
            KeyCode::Backspace => CellBackspace,
            KeyCode::Delete => CellDelete,
            KeyCode::Char('u') if ctrl => return Some(Global::Day(CellHome)), // no-op-ish; keep simple
            KeyCode::Char(c) if !ctrl => CellChar(c),
            _ => return None,
        };
        return Some(Global::Day(a));
    }

    if m.search_focused {
        let a = match key.code {
            KeyCode::Esc => SearchClear,
            KeyCode::Enter => FocusTickets,
            // arrows move the cursor inside the box (type-to-filter, no edit mode)
            KeyCode::Left => SearchCursor(-1),
            KeyCode::Right => SearchCursor(1),
            KeyCode::Home => SearchCursor(i32::MIN),
            KeyCode::End => SearchCursor(i32::MAX),
            // the box is the top row of the pane: ↓ walks into the list, ↑ stays
            KeyCode::Down => FocusTickets,
            KeyCode::Up => return None,
            KeyCode::Backspace => SearchBackspace,
            KeyCode::Delete => SearchDelete,
            KeyCode::Char('u') if ctrl => SearchClear,
            KeyCode::Char(c) if !ctrl => SearchChar(c),
            _ => return None,
        };
        return Some(Global::Day(a));
    }

    let a = match (m.pane, key.code) {
        (_, KeyCode::Char('q')) => return Some(Global::Quit),
        (_, KeyCode::Char('r')) => return Some(Global::Refresh),
        // ← → switch pane; day changes only via [ ] (or ◀ ▶ in the title)
        (_, KeyCode::Left) | (_, KeyCode::Char('h')) => FocusTickets,
        (_, KeyCode::Right) | (_, KeyCode::Char('l')) => FocusPrepare,
        (_, KeyCode::Char('/')) => FocusSearch,
        // Day changes only via [ ] (or the ◀ ▶ in the title) — never arrows,
        // so a stray keypress can't move entries to the wrong day.
        (_, KeyCode::Char('[')) => PrevDay,
        (_, KeyCode::Char(']')) => NextDay,
        (_, KeyCode::Char('p')) => PushDay,
        (_, KeyCode::Char('j')) | (_, KeyCode::Down) => Down,
        (_, KeyCode::Char('k')) | (_, KeyCode::Up) => Up,
        (_, KeyCode::Esc) => Back,

        (Pane::Tickets, KeyCode::Enter) => OpenForm,
        (Pane::Tickets, KeyCode::Char(' ')) => QuickStage,

        (Pane::Tickets, KeyCode::Char('w')) => ToggleWatch,

        (Pane::Prepare, KeyCode::Char('s')) => EditCell(Cell::Start),
        (Pane::Prepare, KeyCode::Char('d')) | (Pane::Prepare, KeyCode::Char('u')) => EditCell(Cell::Duration),
        (Pane::Prepare, KeyCode::Char('n')) => EditCell(Cell::Title),
        (Pane::Prepare, KeyCode::Enter) => EditCell(Cell::Start),
        (Pane::Prepare, KeyCode::Backspace) | (Pane::Prepare, KeyCode::Delete) => Remove,
        _ => return None,
    };
    Some(Global::Day(a))
}

fn model(app: &mut App) -> Option<&mut Model> {
    match &mut app.screen {
        Screen::Day(m) => Some(m),
        _ => None,
    }
}

// ---- search debounce ---------------------------------------------------

pub fn tick(app: &mut App) {
    let Screen::Day(m) = &mut app.screen else { return };
    let Some(since) = m.s.dirty_since else { return };
    if since.elapsed() < DEBOUNCE {
        return;
    }
    m.s.dirty_since = None;
    let q = m.query();
    if q.chars().count() < search::MIN_QUERY_CHARS {
        m.s.results.clear();
        m.s.sent = None;
        m.s.loading = false;
        return;
    }
    if m.s.sent.as_deref() == Some(q.as_str()) {
        return;
    }
    m.s.sent = Some(q.clone());
    if let Some(cached) = m.s.cache.get(&q) {
        m.s.results = cached.clone();
        m.s.loading = false;
        return;
    }
    let Some(gateway) = app.gateway.clone() else { return };
    app.search_req += 1;
    let req_id = app.search_req;
    m.s.req_id = req_id;
    m.s.loading = true;
    app.worker.spawn(move || Msg::SearchDone {
        req_id,
        query: q.clone(),
        result: search::run(gateway.as_ref(), &q).map_err(|e| e.to_string()),
    });
}

pub fn on_search_done(app: &mut App, req_id: u64, query: String, result: Result<Vec<Issue>, String>) {
    let Screen::Day(m) = &mut app.screen else { return };
    match result {
        Ok(issues) => {
            m.s.cache.insert(query, issues.clone());
            if req_id == m.s.req_id {
                m.s.results = issues;
                m.s.loading = false;
            }
        }
        Err(e) => {
            if req_id == m.s.req_id {
                m.s.loading = false;
                app.set_error(format!("search: {e}"));
            }
        }
    }
}

// ---- helpers -------------------------------------------------------------

fn stage(app: &mut App, issue: &Issue, auto_watch: bool) {
    let default_start = app.default_start();
    let quick = app.quick_stage_seconds();
    let auto_watch = auto_watch && app.auto_watch();
    let Screen::Day(m) = &app.screen else { return };
    let date = m.date;
    if auto_watch && app.ledger.watch(&issue.key) {
        app.remote.upsert_watched(issue);
        app.set_status(format!("watching {}", issue.key));
    }
    let id = new_entry_id();
    app.ledger.quick_stage(id.clone(), issue.key.clone(), date, default_start, quick);
    app.save_ledger();
    // Point the prepare cursor at the new row but keep focus where it is,
    // so several tickets can be staged in a row.
    let idx = match &app.screen {
        Screen::Day(m) => prepare_rows(app, m).index_of_entry(&id).unwrap_or(0),
        _ => 0,
    };
    let m = model(app).unwrap();
    m.prepare_sel = idx;
    app.set_status(format!("staged {} on {} — → then d duration · n description · s start", duration::format(quick), issue.key));
}

fn selected_ticket(app: &App, m: &Model) -> Option<super::rows::TicketRow> {
    ticket_rows(app, m).get(m.ticket_sel).cloned()
}

/// Make sure the row at `idx` is a local entry (adopting a Jira-only worklog
/// if needed) and return it with its (possibly new) index.
fn ensure_local(app: &mut App, idx: usize) -> Option<(usize, crate::domain::Entry)> {
    let Screen::Day(m) = &app.screen else { return None };
    let rows = prepare_rows(app, m);
    match rows.rows.get(idx)? {
        PrepareRow::Local(e) => Some((idx, e.clone())),
        PrepareRow::Remote(w) => {
            let w = w.clone();
            let id = app.ledger.adopt_remote(new_entry_id(), &w);
            app.save_ledger();
            let Screen::Day(m) = &app.screen else { return None };
            let rows = prepare_rows(app, m);
            let idx = rows.index_of_entry(&id)?;
            let e = rows.rows[idx].local()?.clone();
            Some((idx, e))
        }
    }
}

fn begin_edit(app: &mut App, idx: Option<usize>, cell: Cell) {
    let Screen::Day(m) = &app.screen else { return };
    let idx = idx.unwrap_or(m.prepare_sel);
    let Some((idx, entry)) = ensure_local(app, idx) else { return };
    if entry.is_deleted() {
        app.set_error("row is marked for deletion — Backspace again to undo first");
        return;
    }
    let text = match cell {
        Cell::Start => entry.start.to_string(),
        Cell::Duration => duration::format(entry.seconds),
        Cell::Title => entry.title.clone(),
    };
    let id = entry.id.clone();
    // Walk start: freeze the pending order so this row stays put until done.
    let frozen = match &app.screen {
        Screen::Day(m) if m.frozen.is_none() => Some(prepare_rows(app, m).pending().iter().map(|r| r.identity()).collect::<Vec<_>>()),
        _ => None,
    };
    let m = model(app).unwrap();
    m.pane = Pane::Prepare;
    m.search_focused = false;
    m.prepare_sel = idx;
    if frozen.is_some() {
        m.frozen = frozen;
        m.walk_id = Some(id.clone());
    }
    m.edit = Some(CellEdit { id, cell, input: TextInput::with(text), error: None, pristine: true });
}

/// Returns true when committed (or nothing to commit).
fn commit_edit(app: &mut App) -> bool {
    let Some(m) = model(app) else { return true };
    let Some(edit) = m.edit.clone() else { return true };
    let text = edit.input.text().trim().to_string();
    let result: Result<Apply, String> = match edit.cell {
        Cell::Start => StartTime::parse(&text).map(|s| Box::new(move |e: &mut crate::domain::Entry| e.start = s) as _).map_err(|e| e.to_string()),
        Cell::Duration => duration::parse(&text)
            .map(|s| Box::new(move |e: &mut crate::domain::Entry| e.seconds = s) as _)
            .map_err(|e| format!("{e}")),
        Cell::Title => Ok(Box::new(move |e: &mut crate::domain::Entry| e.title = text.clone()) as _),
    };
    match result {
        Ok(apply) => {
            m.edit = None;
            app.ledger.edit(&edit.id, apply);
            app.save_ledger();
            // Rows are sorted by start time, so the edited row may have moved:
            // keep the cursor on the same entry, not the same index.
            if let Screen::Day(m) = &app.screen
                && let Some(idx) = prepare_rows(app, m).index_of_entry(&edit.id)
                && let Some(m) = model(app)
            {
                m.prepare_sel = idx;
            }
            true
        }
        Err(e) => {
            if let Some(ed) = &mut m.edit {
                ed.error = Some(e);
            }
            false
        }
    }
}

/// The walk is over: drop the frozen order, let rows re-sort, and put the
/// cursor back on the entry that was being edited.
fn end_walk(app: &mut App) {
    let Some(m) = model(app) else { return };
    m.edit = None;
    m.frozen = None;
    let Some(id) = m.walk_id.take() else { return };
    if let Screen::Day(m) = &app.screen
        && let Some(idx) = prepare_rows(app, m).index_of_entry(&id)
        && let Some(m) = model(app)
    {
        m.prepare_sel = idx;
    }
}

/// Identity of the prepare row at `idx` as currently displayed.
fn row_identity(app: &App, idx: usize) -> Option<String> {
    let Screen::Day(m) = &app.screen else { return None };
    prepare_rows(app, m).rows.get(idx).map(|r| r.identity())
}

fn index_of_identity(app: &App, ident: &str) -> Option<usize> {
    let Screen::Day(m) = &app.screen else { return None };
    prepare_rows(app, m).rows.iter().position(|r| r.identity() == ident)
}

fn clamp(app: &mut App) {
    let Screen::Day(m) = &app.screen else { return };
    let t = ticket_rows(app, m).len();
    let p = prepare_rows(app, m).rows.len();
    let m = model(app).unwrap();
    m.ticket_sel = if t == 0 { 0 } else { m.ticket_sel.min(t - 1) };
    m.prepare_sel = if p == 0 { 0 } else { m.prepare_sel.min(p - 1) };
}

// ---- update ----------------------------------------------------------------

/// Actions that operate on the active cell editor. Anything else moves focus
/// away and therefore commits the in-progress edit first (Esc is the only
/// way to revert); unparseable text is dropped and the old value kept.
fn is_cell_action(a: &Action) -> bool {
    use Action::*;
    matches!(
        a,
        CellChar(_) | CellNewline | CellBackspace | CellDelete | CellLeft | CellRight | CellHome | CellEnd | CellStep(_) | CellNext | CellCancel | CellCursor(_) | Paste(_)
    )
}

pub fn update(app: &mut App, action: Action) {
    use Action::*;
    let mut action = action;
    if !is_cell_action(&action) {
        // Remember which row was clicked *before* the list may re-sort.
        let clicked = match &action {
            SelectPrepare(i) | EditCellOf(i, _) => row_identity(app, *i),
            _ => None,
        };
        if !commit_edit(app) {
            // Could not parse: keep the previous value, tell the user, move on.
            let err = model(app).and_then(|m| m.edit.as_ref()).and_then(|e| e.error.clone()).unwrap_or_default();
            app.set_error(format!("edit dropped: {err}"));
        }
        if model(app).is_some_and(|m| m.frozen.is_some() || m.edit.is_some()) {
            end_walk(app);
        }
        if let Some(ident) = clicked
            && let Some(i) = index_of_identity(app, &ident)
        {
            action = match action {
                SelectPrepare(_) => SelectPrepare(i),
                EditCellOf(_, c) => EditCellOf(i, c),
                other => other,
            };
        }
    }
    match action {
        FocusTickets => {
            let Some(m) = model(app) else { return };
            m.pane = Pane::Tickets;
            m.search_focused = false;
        }
        FocusPrepare => {
            let Some(m) = model(app) else { return };
            m.pane = Pane::Prepare;
            m.search_focused = false;
        }
        FocusSearch => {
            let Some(m) = model(app) else { return };
            m.pane = Pane::Tickets;
            m.search_focused = true;
        }
        SearchChar(c) => {
            let Some(m) = model(app) else { return };
            m.search.insert(c);
            m.s.dirty_since = Some(Instant::now());
            m.ticket_sel = 0;
        }
        SearchBackspace => {
            let Some(m) = model(app) else { return };
            m.search.backspace();
            m.s.dirty_since = Some(Instant::now());
            m.ticket_sel = 0;
        }
        SearchDelete => {
            let Some(m) = model(app) else { return };
            m.search.delete();
            m.s.dirty_since = Some(Instant::now());
            m.ticket_sel = 0;
        }
        SearchCursor(d) => {
            let Some(m) = model(app) else { return };
            match d {
                i32::MIN => m.search.home(),
                i32::MAX => m.search.end(),
                d if d < 0 => m.search.left(),
                _ => m.search.right(),
            }
        }
        SearchClear => {
            let Some(m) = model(app) else { return };
            if m.search.is_empty() {
                m.search_focused = false;
            } else {
                m.search.clear();
                m.s.results.clear();
                m.s.sent = None;
                m.s.loading = false;
                m.s.dirty_since = None;
                m.ticket_sel = 0;
            }
        }
        Paste(s) => {
            let Some(m) = model(app) else { return };
            if let Some(ed) = &mut m.edit {
                if ed.pristine {
                    ed.input.clear();
                    ed.pristine = false;
                }
                ed.input.insert_str(s.trim());
            } else if m.search_focused {
                m.search.insert_str(s.trim());
                m.s.dirty_since = Some(Instant::now());
            }
        }
        Up | Down => {
            let delta: i32 = if matches!(action, Up) { -1 } else { 1 };
            let Screen::Day(m) = &app.screen else { return };
            let (t, p) = (ticket_rows(app, m).len() as i32, prepare_rows(app, m).rows.len() as i32);
            let m = model(app).unwrap();
            match m.pane {
                // ↑ past the first ticket lands on the search box (typing starts at once)
                Pane::Tickets if delta < 0 && (m.ticket_sel == 0 || t == 0) => m.search_focused = true,
                Pane::Tickets if t > 0 => m.ticket_sel = (m.ticket_sel as i32 + delta).min(t - 1) as usize,
                Pane::Prepare if p > 0 => m.prepare_sel = (m.prepare_sel as i32 + delta).rem_euclid(p) as usize,
                _ => {}
            }
        }
        SelectTicket(i) => {
            let Some(m) = model(app) else { return };
            m.pane = Pane::Tickets;
            m.search_focused = false;
            m.ticket_sel = i;
            clamp(app);
        }
        SelectPrepare(i) => {
            let Some(m) = model(app) else { return };
            m.pane = Pane::Prepare;
            m.search_focused = false;
            m.prepare_sel = i;
            clamp(app);
        }
        QuickStage => {
            let Screen::Day(m) = &app.screen else { return };
            let Some(row) = selected_ticket(app, m) else {
                app.set_error("no ticket selected");
                return;
            };
            stage(app, &row.issue, row.section == Section::Jira);
        }
        StageKey(key) => {
            let Screen::Day(m) = &app.screen else { return };
            let found = ticket_rows(app, m).into_iter().find(|r| r.issue.key == key);
            if let Some(row) = found {
                stage(app, &row.issue, row.section == Section::Jira);
            }
        }
        OpenForm => {
            let Screen::Day(m) = &app.screen else { return };
            let date = m.date;
            let issue = selected_ticket(app, m).map(|r| r.issue);
            let start = app.default_start();
            let mut candidates = app.local_issues();
            if let Some(i) = &issue
                && !candidates.iter().any(|c| c.key == i.key) {
                    candidates.insert(0, i.clone());
                }
            let form = entry_form::Model::new_entry(date, issue.as_ref(), start, candidates);
            app.open_form(form);
        }
        ToggleWatch => {
            let Screen::Day(m) = &app.screen else { return };
            let Some(row) = selected_ticket(app, m) else { return };
            if app.ledger.is_watched(&row.issue.key) {
                app.ledger.unwatch(&row.issue.key);
                app.save_ledger();
                app.set_status(format!("removed {} from watchlist", row.issue.key));
                clamp(app);
            } else {
                app.ledger.watch(&row.issue.key);
                app.remote.upsert_watched(&row.issue);
                app.save_ledger();
                app.set_status(format!("watching {}", row.issue.key));
            }
        }
        EditCell(cell) => begin_edit(app, None, cell),
        EditCellOf(i, cell) => begin_edit(app, Some(i), cell),
        CellChar(c) => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                if ed.pristine {
                    ed.input.clear();
                    ed.pristine = false;
                }
                ed.input.insert(c);
                ed.error = None;
            }
        }
        CellNewline => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                ed.pristine = false;
                ed.input.insert('\n');
            }
        }
        CellBackspace => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                if ed.pristine {
                    ed.input.clear();
                    ed.pristine = false;
                } else {
                    ed.input.backspace();
                }
                ed.error = None;
            }
        }
        CellDelete => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                ed.input.delete();
            }
        }
        CellLeft => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                ed.pristine = false;
                ed.input.left();
            }
        }
        CellRight => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                ed.input.right();
            }
        }
        CellHome => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                ed.input.home();
            }
        }
        CellEnd => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                ed.input.end();
            }
        }
        CellCursor(col) => {
            if let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) {
                ed.pristine = false;
                ed.input.set_cursor_col(col as usize);
            }
        }
        CellStep(delta) => {
            let Some(ed) = model(app).and_then(|m| m.edit.as_mut()) else { return };
            match ed.cell {
                Cell::Start => {
                    let cur = StartTime::parse(ed.input.text()).unwrap_or(StartTime::NINE);
                    ed.input.set(cur.stepped(delta).to_string());
                }
                Cell::Duration => {
                    let cur = duration::parse(ed.input.text()).unwrap_or(3600) as i64;
                    let next = (cur + delta as i64 * 60).max(15 * 60) as u64;
                    ed.input.set(duration::format(duration::snap(next)));
                }
                Cell::Title => {}
            }
            ed.error = None;
        }
        CellNext => {
            let next = model(app).and_then(|m| m.edit.as_ref()).and_then(|e| e.cell.next());
            if commit_edit(app) {
                match next {
                    Some(cell) => begin_edit(app, None, cell),
                    None => end_walk(app),
                }
            }
        }
        CellCancel => end_walk(app),
        Remove => {
            let Screen::Day(m) = &app.screen else { return };
            let Some((idx, e)) = ensure_local(app, m.prepare_sel) else { return };
            let removed = app.ledger.toggle_delete(&e.id);
            app.save_ledger();
            let label = format!("{} {}", duration::format(e.seconds), e.issue_key);
            if removed {
                app.set_status(format!("removed {label}"));
            } else if e.is_deleted() {
                app.set_status(format!("kept {label} (delete undone)"));
            } else {
                app.set_status(format!("{label} marked for deletion — sent on push, Backspace to undo"));
            }
            if let Some(m) = model(app) {
                m.prepare_sel = idx;
            }
            clamp(app);
        }
        PrevDay | NextDay => {
            let Some(m) = model(app) else { return };
            m.date = if matches!(action, PrevDay) { m.date - Days::new(1) } else { m.date + Days::new(1) };
            m.prepare_sel = 0;
            m.edit = None;
            clamp(app);
            app.ensure_window();
        }
        Back => {
            let Some(m) = model(app) else { return };
            match m.pane {
                Pane::Prepare => {
                    m.pane = Pane::Tickets;
                }
                Pane::Tickets => {
                    let date = m.date;
                    app.go_week(Week::containing(date), date);
                }
            }
        }
        PushDay => {
            let Screen::Day(m) = &app.screen else { return };
            let date = m.date;
            app.dispatch(Global::PushRequest(PushScope::Day(date)));
        }
    }
}

#[allow(dead_code)]
fn _unused(_: IssueKey) {}
