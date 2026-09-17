use super::model::{Action, Model};
use crate::domain::Week;
use crate::tui::action::{Action as Global, PushScope};
use crate::tui::app::{App, Screen};
use crate::tui::features::entry_form;
use crossterm::event::{KeyCode, KeyEvent};

pub fn keys(m: &Model, key: &KeyEvent) -> Option<Global> {
    use Action::*;
    let a = match key.code {
        KeyCode::Char('q') => return Some(Global::Quit),
        KeyCode::Char('r') => return Some(Global::Refresh),
        KeyCode::Char(',') => return Some(Global::OpenSettings),
        KeyCode::Tab => return Some(Global::SwitchMainTab),
        KeyCode::Char('[') | KeyCode::Char('h') | KeyCode::Left => PrevWeek,
        KeyCode::Char(']') | KeyCode::Char('l') | KeyCode::Right => NextWeek,
        KeyCode::Char('t') => Today,
        KeyCode::Char('j') | KeyCode::Down => Down,
        KeyCode::Char('k') | KeyCode::Up => Up,
        KeyCode::Enter => OpenDay(m.selected),
        KeyCode::Char('a') | KeyCode::Char(' ') => AddEntry,
        KeyCode::Char('p') => PushWeek,
        _ => return None,
    };
    Some(Global::Week(a))
}

pub fn update(app: &mut App, action: Action) {
    use Action::*;
    let today = app.today;
    let Screen::Week(m) = &mut app.screen else { return };
    match action {
        PrevWeek => {
            m.week = m.week.prev();
            app.ensure_window();
        }
        NextWeek => {
            m.week = m.week.next();
            app.ensure_window();
        }
        Today => {
            m.week = Week::containing(today);
            m.select_date(today);
            app.ensure_window();
        }
        Up => m.selected = (m.selected + 6) % 7,
        Down => m.selected = (m.selected + 1) % 7,
        Select(i) => m.selected = i.min(6),
        OpenDay(i) => {
            let date = m.week.days()[i.min(6)];
            app.go_day(date);
        }
        AddEntry => {
            let date = m.selected_date();
            let start = app.default_start();
            let form = entry_form::Model::new_entry(date, None, start, app.local_issues());
            app.open_form(form);
        }
        PushWeek => {
            let scope = PushScope::Week(m.week);
            app.dispatch(Global::PushRequest(scope));
        }
    }
}
