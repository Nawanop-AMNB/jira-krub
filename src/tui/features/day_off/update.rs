use super::model::{Action, DEFAULT_NAME, Focus, Model};
use crate::application::Config;
use crate::tui::action::Action as Global;
use crate::tui::app::{App, Overlay};
use crate::tui::features::settings::normalise_date;
use crate::tui::widgets::TextInput;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn keys(key: &KeyEvent) -> Option<Global> {
    use Action::*;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let a = match key.code {
        KeyCode::Char(_) if ctrl => return None,
        KeyCode::Char(c) => Char(c),
        KeyCode::Esc => Cancel,
        KeyCode::Enter => Enter,
        KeyCode::Tab => Next,
        KeyCode::BackTab => Prev,
        KeyCode::Up => Up,
        KeyCode::Down => Down,
        KeyCode::Left => Left,
        KeyCode::Right => Right,
        KeyCode::Home => Home,
        KeyCode::End => End,
        KeyCode::Backspace => Backspace,
        KeyCode::Delete => Delete,
        _ => return None,
    };
    Some(Global::DayOff(a))
}

fn model(app: &mut App) -> Option<&mut Model> {
    match &mut app.overlay {
        Some(Overlay::DayOff(m)) => Some(m),
        _ => None,
    }
}

pub fn update(app: &mut App, action: Action) {
    use Action::*;
    match action {
        Enter => enter(app),
        Save => save(app),
        Remove => remove(app),
        Cancel => app.close_overlay(),
        other => {
            let Some(m) = model(app) else { return };
            match other {
                Focus(f) => m.focus = f,
                Next => m.tab(1),
                Prev => m.tab(-1),
                Up => m.up(),
                Down => m.down(),
                Left => m.left(),
                Right => m.right(),
                Char(c) => m.edit(|t| t.insert(c)),
                Paste(s) => m.edit(|t| t.insert_str(&s)),
                Backspace => m.edit(TextInput::backspace),
                Delete => m.edit(TextInput::delete),
                Home => m.edit(TextInput::home),
                End => m.edit(TextInput::end),
                Enter | Save | Remove | Cancel => {}
            }
        }
    }
}

/// Enter walks Date -> Name -> Save, then presses whichever button has focus.
fn enter(app: &mut App) {
    let Some(m) = model(app) else { return };
    match m.focus {
        Focus::Date => m.focus = Focus::Name,
        Focus::Name => m.focus = Focus::Save,
        Focus::Save => save(app),
        Focus::Delete => remove(app),
        Focus::Cancel => app.close_overlay(),
    }
}

fn save(app: &mut App) {
    let Some(m) = model(app) else { return };
    let date = match normalise_date(m.date.text(), m.year) {
        Ok(d) => d,
        Err(e) => {
            m.error = Some(e);
            return;
        }
    };
    let name = match m.name.text().trim() {
        "" => DEFAULT_NAME.to_string(),
        n => n.to_string(),
    };
    let moved_from = m.original;
    persist(app, "day off saved", |cfg| cfg.set_day_off(moved_from, date, name));
}

fn remove(app: &mut App) {
    let Some(original) = model(app).and_then(|m| m.original) else { return };
    persist(app, "day off removed", |cfg| cfg.remove_day_off(original));
}

/// Apply `change` to a copy of the config and write it to disk; only a
/// successful write replaces `app.config` and closes the modal.
fn persist(app: &mut App, status: &str, change: impl FnOnce(&mut Config)) {
    let Some(mut cfg) = app.config.clone() else {
        app.close_overlay();
        app.set_error("not connected");
        return;
    };
    change(&mut cfg);
    match app.deps.config_store.save(&cfg) {
        Ok(()) => {
            app.config = Some(cfg);
            app.close_overlay();
            app.set_status(status);
        }
        Err(e) => {
            if let Some(m) = model(app) {
                m.error = Some(format!("could not save: {e}"));
            }
        }
    }
}
