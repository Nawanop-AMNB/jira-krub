use super::model::{Action, Field, Model};
use crate::application::ids::new_entry_id;
use crate::domain::{Entry, EntryState, StartTime, duration};
use crate::tui::action::Action as Global;
use crate::tui::app::{App, Overlay};
use chrono::Days;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn keys(m: &Model, key: &KeyEvent) -> Option<Global> {
    use Action::*;
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if key.code == KeyCode::Char('s') && ctrl {
        return Some(Global::Form(Save));
    }
    let newline_combo = matches!(key.code, KeyCode::Enter) && (ctrl || shift || key.modifiers.contains(KeyModifiers::ALT));
    let a = if m.editing {
        match key.code {
            KeyCode::Esc => Revert,
            // multi-line description: Ctrl/Shift/Alt+Enter (Kitty-capable terminals) or Ctrl+J anywhere
            KeyCode::Enter if m.focus == Field::Title && newline_combo => Newline,
            KeyCode::Char('j') if m.focus == Field::Title && ctrl => Newline,
            KeyCode::Enter if m.focus == Field::Issue => AcceptIssue,
            KeyCode::Enter => Commit(1),
            // inside these fields ↑↓ have a field meaning; elsewhere they commit and move
            KeyCode::Up => match m.focus {
                Field::Issue => SuggestPrev,
                Field::Start => StartStep(if shift { 60 } else { 15 }),
                _ => Commit(-1),
            },
            KeyCode::Down => match m.focus {
                Field::Issue => SuggestNext,
                Field::Start => StartStep(if shift { -60 } else { -15 }),
                _ => Commit(1),
            },
            KeyCode::Left if m.focus == Field::Date => DateShift(-1),
            KeyCode::Right if m.focus == Field::Date => DateShift(1),
            KeyCode::Left => Left,
            KeyCode::Right => Right,
            KeyCode::Home => Home,
            KeyCode::End => End,
            KeyCode::Backspace => Backspace,
            KeyCode::Delete => Delete,
            KeyCode::Char('h') if m.focus == Field::Date => DateShift(-1),
            KeyCode::Char('l') if m.focus == Field::Date => DateShift(1),
            KeyCode::Char('t') if m.focus == Field::Date => DateShift(i64::MIN),
            KeyCode::Char(c) if !ctrl => Char(c),
            _ => return None,
        }
    } else {
        match key.code {
            KeyCode::Esc => Cancel,
            KeyCode::Enter => Open,
            KeyCode::Up => FocusPrev,
            KeyCode::Down => FocusNext,
            // the date is the screen's unit: arrows shift it even in NAV
            KeyCode::Left if m.focus == Field::Date => DateShift(-1),
            KeyCode::Right if m.focus == Field::Date => DateShift(1),
            KeyCode::Char('t') if m.focus == Field::Date => DateShift(i64::MIN),
            _ => return None,
        }
    };
    Some(Global::Form(a))
}

fn model(app: &mut App) -> Option<&mut Model> {
    match &mut app.overlay {
        Some(Overlay::Form(m)) => Some(m),
        _ => None,
    }
}

fn open(m: &mut Model) {
    if m.focus == Field::Save {
        return;
    }
    m.snapshot();
    m.editing = true;
    let f = m.focus;
    if let Some(t) = m.field_mut(f) {
        t.end();
    }
}

/// Validate the open field. Returns false (and sets the error) if it must stay open.
fn field_ok(m: &mut Model) -> bool {
    let err = match m.focus {
        Field::Start if !m.start.is_empty() => StartTime::parse(m.start.text()).err().map(|e| format!("start: {e}")),
        Field::Duration if !m.duration.is_empty() => duration::parse(m.duration.text()).err().map(|e| format!("duration: {e} — e.g. 1h30m, 90m")),
        _ => None,
    };
    m.error = err;
    m.error.is_none()
}

fn save(app: &mut App) {
    let default_start = app.default_start();
    let Some(m) = model(app) else { return };
    m.error = None;
    let Some(key) = m.resolve_issue() else {
        m.error = Some("pick an issue (type part of a key or summary)".into());
        m.focus = Field::Issue;
        open(m);
        return;
    };
    let start = if m.start.is_empty() {
        default_start
    } else {
        match StartTime::parse(m.start.text()) {
            Ok(s) => s,
            Err(e) => {
                m.error = Some(format!("start: {e}"));
                m.focus = Field::Start;
                open(m);
                return;
            }
        }
    };
    let seconds = match duration::parse(m.duration.text()) {
        Ok(s) => s,
        Err(e) => {
            m.error = Some(format!("duration: {e} — e.g. 1h30m, 90m"));
            m.focus = Field::Duration;
            open(m);
            return;
        }
    };
    let entry = Entry {
        id: new_entry_id(),
        issue_key: key,
        date: m.date,
        start,
        seconds,
        title: m.title.text().trim().to_string(),
        state: EntryState::Staged,
    };
    let label = format!("{} {} on {}", duration::format(entry.seconds), entry.issue_key, entry.date.format("%a %d %b"));
    app.ledger.add(entry);
    app.save_ledger();
    app.close_overlay();
    app.set_status(format!("staged {label}"));
}

pub fn update(app: &mut App, action: Action) {
    use Action::*;
    let today = app.today;
    match action {
        Save => save(app),
        Cancel => app.close_overlay(),
        other => {
            let Some(m) = model(app) else { return };
            match other {
                Open => {
                    if m.focus == Field::Save {
                        save(app);
                    } else {
                        m.error = None;
                        open(m);
                    }
                }
                Commit(delta) => {
                    if !field_ok(m) {
                        return;
                    }
                    m.editing = false;
                    m.focus = if delta < 0 { m.focus.prev() } else { m.focus.next() };
                    // walk forward keeps typing until the Save button
                    if delta > 0 && m.focus != Field::Save {
                        m.error = None;
                        open(m);
                    }
                }
                Revert => {
                    let backup = m.backup.clone();
                    m.date = m.backup_date;
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.set(backup);
                    }
                    m.editing = false;
                    m.error = None;
                }
                FocusNext => m.focus = m.focus.next(),
                FocusPrev => m.focus = m.focus.prev(),
                // click = focus and open (click is Enter)
                Focus(f) => {
                    m.editing = false;
                    m.focus = f;
                    m.error = None;
                    open(m);
                }
                AcceptIssue => {
                    if let Some(key) = m.resolve_issue() {
                        m.issue.set(key.as_str().to_string());
                        m.editing = false;
                        m.focus = Field::Duration;
                        open(m);
                    } else {
                        m.error = Some("no matching issue".into());
                    }
                }
                SuggestNext => {
                    let n = m.suggestions().len();
                    if n > 0 {
                        m.suggestion_sel = (m.suggestion_sel + 1) % n;
                    }
                }
                SuggestPrev => {
                    let n = m.suggestions().len();
                    if n > 0 {
                        m.suggestion_sel = (m.suggestion_sel + n - 1) % n;
                    }
                }
                DateShift(i64::MIN) => m.date = today,
                DateShift(d) => {
                    m.date = if d >= 0 { m.date + Days::new(d as u64) } else { m.date - Days::new((-d) as u64) };
                }
                StartStep(delta) => {
                    let cur = StartTime::parse(m.start.text()).unwrap_or(StartTime::NINE);
                    m.start.set(cur.stepped(delta).to_string());
                }
                Newline => {
                    if m.focus == Field::Title {
                        m.title.insert('\n');
                    }
                }
                Char(c) => {
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.insert(c);
                    }
                    if f == Field::Issue {
                        m.suggestion_sel = 0;
                    }
                    m.error = None;
                }
                Paste(s) => {
                    if !m.editing {
                        open(m);
                    }
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.insert_str(s.trim());
                    }
                }
                Backspace => {
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.backspace();
                    }
                    if f == Field::Issue {
                        m.suggestion_sel = 0;
                    }
                }
                Delete => {
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.delete();
                    }
                }
                Left => {
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.left();
                    }
                }
                Right => {
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.right();
                    }
                }
                Home => {
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.home();
                    }
                }
                End => {
                    let f = m.focus;
                    if let Some(t) = m.field_mut(f) {
                        t.end();
                    }
                }
                Save | Cancel => {}
            }
        }
    }
}
