use super::model::{Action, Model, model, visible_issues};
use crate::tui::action::Action as Global;
use crate::tui::app::{App, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn keys(m: &Model, key: &KeyEvent) -> Option<Global> {
    use Action::*;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    if m.filter_focused {
        let a = match key.code {
            KeyCode::Tab => return Some(Global::SwitchMainTab),
            KeyCode::Left => FilterCursor(-1),
            KeyCode::Right => FilterCursor(1),
            KeyCode::Home => FilterCursor(i32::MIN),
            KeyCode::End => FilterCursor(i32::MAX),
            // the box is the top row of the list: ↓ walks into it, ↑ stays
            KeyCode::Down => Down,
            KeyCode::Up => return None,
            KeyCode::Backspace => FilterBackspace,
            KeyCode::Delete => FilterDelete,
            KeyCode::Char('u') if ctrl => FilterClear,
            // while the box is focused, letters are never hotkeys
            KeyCode::Char(c) if !ctrl => FilterChar(c),
            _ => return None,
        };
        return Some(Global::Tasks(a));
    }

    let a = match key.code {
        KeyCode::Char('q') => return Some(Global::Quit),
        KeyCode::Char('r') => return Some(Global::Refresh),
        KeyCode::Char(',') => return Some(Global::OpenSettings),
        KeyCode::Tab => return Some(Global::SwitchMainTab),
        KeyCode::Char('/') => FocusFilter,
        KeyCode::Char('j') | KeyCode::Down => Down,
        KeyCode::Char('k') | KeyCode::Up => Up,
        KeyCode::Enter => Open,
        // Space, w, a, p, [ ] and the arrows log work elsewhere: inert here.
        _ => return None,
    };
    Some(Global::Tasks(a))
}

pub fn update(app: &mut App, action: Action) {
    use Action::*;
    match action {
        FocusFilter => {
            if let Some(m) = model(app) {
                m.filter_focused = true;
            }
        }
        FilterChar(c) => edit_filter(app, |m| m.filter.insert(c)),
        FilterBackspace => edit_filter(app, |m| m.filter.backspace()),
        FilterDelete => edit_filter(app, |m| m.filter.delete()),
        FilterClear => {
            let Some(m) = model(app) else { return };
            if !m.filter.is_empty() {
                m.filter.clear();
                m.selected = 0;
            }
        }
        FilterCursor(d) => {
            let Some(m) = model(app) else { return };
            match d {
                i32::MIN => m.filter.home(),
                i32::MAX => m.filter.end(),
                d if d < 0 => m.filter.left(),
                _ => m.filter.right(),
            }
        }
        Up => {
            let len = visible_len(app);
            let Some(m) = model(app) else { return };
            if m.filter_focused {
                return;
            }
            // ↑ past the first row lands on the filter box (typing starts at once)
            if m.selected == 0 || len == 0 {
                m.filter_focused = true;
            } else {
                m.selected -= 1;
            }
        }
        Down => {
            let len = visible_len(app);
            let Some(m) = model(app) else { return };
            if m.filter_focused {
                m.filter_focused = false;
                m.selected = 0;
            } else if m.selected + 1 < len {
                m.selected += 1;
            }
        }
        Select(i) => select(app, i),
        SelectAndOpen(i) => {
            select(app, i);
            open_selected(app);
        }
        Open => open_selected(app),
    }
}

fn edit_filter(app: &mut App, f: impl FnOnce(&mut Model)) {
    let Some(m) = model(app) else { return };
    f(m);
    // the list just changed shape: never leave the cursor past its end
    m.selected = 0;
}

fn visible_len(app: &App) -> usize {
    match &app.screen {
        Screen::Tasks(m) => visible_issues(app, m).len(),
        _ => 0,
    }
}

fn select(app: &mut App, i: usize) {
    let len = visible_len(app);
    let Some(m) = model(app) else { return };
    m.filter_focused = false;
    m.selected = if len == 0 { 0 } else { i.min(len - 1) };
}

/// `<site>/browse/KEY` through the `UrlOpener` port — the TUI never spawns.
fn open_selected(app: &mut App) {
    let Screen::Tasks(m) = &app.screen else { return };
    let Some(issue) = visible_issues(app, m).into_iter().nth(m.selected) else { return };
    // Tasks is only reachable once connected, but a missing config is no reason to panic.
    let Some(site) = app.config.as_ref().map(|c| c.credentials.site.as_str().to_string()) else { return };
    let url = format!("{site}/browse/{}", issue.key);
    let opener = app.deps.opener.clone();
    match opener.open(&url) {
        Ok(()) => app.set_status(format!("opened {}", issue.key)),
        Err(e) => app.set_error(format!("could not open browser: {e}")),
    }
}
