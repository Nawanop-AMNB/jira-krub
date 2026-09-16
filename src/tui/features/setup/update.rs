use super::model::{Action, Focus as F, Model, TestState};
use crate::application::use_cases::test_connection::{self, Outcome};
use crate::application::{Config, Credentials};
use crate::domain::{SiteUrl, Week};
use crate::tui::action::Action as Global;
use crate::tui::app::{App, Screen};
use crate::tui::msg::Msg;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn keys(m: &Model, key: &KeyEvent) -> Option<Global> {
    use Action::*;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let a = match key.code {
        KeyCode::Esc => Quit,
        KeyCode::Tab => FocusNext,
        KeyCode::BackTab => FocusPrev,
        KeyCode::Down => FocusNext,
        KeyCode::Up => FocusPrev,
        KeyCode::Enter => Activate,
        KeyCode::Char('u') if ctrl => ClearField,
        KeyCode::Backspace => Backspace,
        KeyCode::Delete => Delete,
        KeyCode::Left if m.focus.is_field() => Left,
        KeyCode::Right if m.focus.is_field() => Right,
        KeyCode::Left => FocusPrev,
        KeyCode::Right => FocusNext,
        KeyCode::Home => Home,
        KeyCode::End => End,
        KeyCode::Char(c) if m.focus.is_field() && !ctrl => Char(c),
        KeyCode::Char('q') => Quit,
        _ => return None,
    };
    Some(Global::Setup(a))
}

fn model(app: &mut App) -> Option<&mut Model> {
    match &mut app.screen {
        Screen::Setup(m) => Some(m),
        _ => None,
    }
}

fn validate(m: &mut Model) -> Option<Credentials> {
    m.site_err = None;
    m.email_err = None;
    m.token_err = None;
    let site = match SiteUrl::parse(m.site.text()) {
        Ok(s) => Some(s),
        Err(e) => {
            m.site_err = Some(e.to_string());
            None
        }
    };
    let email = m.email.text().trim().to_string();
    let email_ok = {
        let parts: Vec<&str> = email.split('@').collect();
        parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.') && !parts[1].starts_with('.')
    };
    if !email_ok {
        m.email_err = Some("enter a valid email".into());
    }
    let token = m.token_value();
    if token.trim().is_empty() {
        m.token_err = Some("token is required".into());
    }
    match (site, email_ok, m.token_err.is_none()) {
        (Some(site), true, true) => Some(Credentials { site, email, api_token: token }),
        _ => {
            m.focus = if m.site_err.is_some() {
                F::Site
            } else if m.email_err.is_some() {
                F::Email
            } else {
                F::Token
            };
            None
        }
    }
}

fn start_test(app: &mut App) {
    let Some(m) = model(app) else { return };
    let Some(creds) = validate(m) else {
        m.save_after_test = false;
        return;
    };
    m.test = TestState::Testing;
    let factory = app.deps.gateway_factory.clone();
    app.worker.spawn(move || {
        let outcome = match factory(&creds) {
            Ok(g) => test_connection::run(g.as_ref()),
            Err(e) => Outcome::Other(e.to_string()),
        };
        Msg::ConnectionTested { creds, outcome }
    });
}

fn save(app: &mut App) {
    let Some(m) = model(app) else { return };
    let tested = match &m.test {
        TestState::Ok { creds, .. } => Some(creds.clone()),
        _ => None,
    };
    // Only save what was tested.
    let current = validate(m);
    let creds = match (tested, current) {
        (Some(t), Some(c)) if t == c => c,
        _ => {
            m.save_after_test = true;
            start_test(app);
            return;
        }
    };
    let config = match &app.config {
        Some(c) => Config { credentials: creds.clone(), ..c.clone() },
        None => Config::with_defaults(creds.clone()),
    };
    if let Err(e) = app.deps.config_store.save(&config) {
        app.set_error(format!("could not save config: {e}"));
        return;
    }
    match (app.deps.gateway_factory)(&creds) {
        Ok(g) => app.gateway = Some(g),
        Err(e) => {
            app.set_error(format!("gateway: {e}"));
            return;
        }
    }
    app.config = Some(config);
    let today = app.today;
    app.go_week(Week::containing(today), today);
    app.set_status(format!("saved to {}", app.deps.config_store.location()));
    app.start_sync();
}

pub fn on_tested(app: &mut App, creds: Credentials, outcome: Outcome) {
    let Some(m) = model(app) else { return };
    let save_after = m.save_after_test;
    m.save_after_test = false;
    match outcome {
        Outcome::Ok(me) => {
            m.test = TestState::Ok { display_name: me.display_name, creds };
            m.banner = None;
            if save_after {
                save(app);
            }
        }
        Outcome::Unauthorized => {
            m.test = TestState::Err("401 unauthorized — check email and token".into());
            m.token_err = Some("rejected".into());
            m.focus = F::Token;
        }
        Outcome::NotJira => {
            m.test = TestState::Err("404 — site reachable but not a Jira Cloud API, check URL".into());
            m.site_err = Some("not a Jira Cloud API".into());
            m.focus = F::Site;
        }
        Outcome::Network(e) => {
            m.test = TestState::Err(format!("site not found / network: {e}"));
            m.site_err = Some("unreachable".into());
            m.focus = F::Site;
        }
        Outcome::Other(e) => m.test = TestState::Err(e),
    }
}

pub fn update(app: &mut App, action: Action) {
    use Action::*;
    match action {
        Quit => {
            // Settings from main → back. First run → exit.
            if app.gateway.is_some() {
                let today = app.today;
                app.go_week(Week::containing(today), today);
            } else {
                app.quit = true;
            }
        }
        Test => start_test(app),
        Save => save(app),
        Activate => {
            let focus = model(app).map(|m| m.focus);
            match focus {
                Some(F::SaveBtn) => save(app),
                Some(F::QuitBtn) => update(app, Quit),
                Some(_) => start_test(app),
                None => {}
            }
        }
        other => {
            let Some(m) = model(app) else { return };
            match other {
                FocusNext => m.focus = m.focus.next(),
                FocusPrev => m.focus = m.focus.prev(),
                Focus(f) => m.focus = f,
                ClearField => {
                    if m.focus == self::F::Token {
                        m.token_kept = false;
                    }
                    if let Some(f) = m.field_mut() {
                        f.clear();
                    }
                }
                Char(c) => {
                    if m.focus == self::F::Token {
                        m.token_kept = false;
                    }
                    if let Some(f) = m.field_mut() {
                        f.insert(c);
                    }
                    m.test = TestState::Idle;
                }
                Paste(s) => {
                    if m.focus == self::F::Token {
                        m.token_kept = false;
                    }
                    if let Some(f) = m.field_mut() {
                        f.insert_str(s.trim());
                    }
                    m.test = TestState::Idle;
                }
                Backspace => {
                    if m.focus == self::F::Token && m.token_kept {
                        m.token_kept = false;
                    } else if let Some(f) = m.field_mut() {
                        f.backspace();
                    }
                    m.test = TestState::Idle;
                }
                Delete => {
                    if let Some(f) = m.field_mut() {
                        f.delete();
                    }
                }
                Left => {
                    if let Some(f) = m.field_mut() {
                        f.left();
                    }
                }
                Right => {
                    if let Some(f) = m.field_mut() {
                        f.right();
                    }
                }
                Home => {
                    if let Some(f) = m.field_mut() {
                        f.home();
                    }
                }
                End => {
                    if let Some(f) = m.field_mut() {
                        f.end();
                    }
                }
                Quit | Test | Save | Activate => {}
            }
        }
    }
}
