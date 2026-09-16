use super::action::{Action, PushScope};
use super::deps::Deps;
use super::features::{day, entry_form, push, setup, week};
use super::hit::HitRegistry;
use super::msg::{Msg, SyncError};
use crate::application::ports::GatewayErrorKind;
use crate::application::use_cases::sync::{self, SyncInput};
use crate::application::{Config, JiraGateway};
use crate::domain::{Issue, IssueKey, Ledger, RemoteWorklog, StartTime, Week};
use crate::infrastructure::Worker;
use anyhow::Result;
use chrono::{Days, Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// What we last heard from Jira.
#[derive(Default)]
pub struct RemoteCache {
    pub account_id: Option<String>,
    pub display_name: String,
    pub mine: Vec<Issue>,
    pub watched: Vec<Issue>,
    pub history: Vec<Issue>,
    pub worklogs: Vec<RemoteWorklog>,
    pub syncing: bool,
    pub offline: bool,
}

impl RemoteCache {
    pub fn issue(&self, key: &IssueKey) -> Option<&Issue> {
        self.watched.iter().chain(self.mine.iter()).chain(self.history.iter()).find(|i| &i.key == key)
    }
    /// Remember a freshly fetched issue (from search or sync) for display.
    pub fn upsert_watched(&mut self, issue: &Issue) {
        match self.watched.iter_mut().find(|i| i.key == issue.key) {
            Some(slot) => *slot = issue.clone(),
            None => self.watched.push(issue.clone()),
        }
    }
}

pub enum Screen {
    Setup(setup::Model),
    Week(week::Model),
    Day(day::Model),
}

pub enum Overlay {
    Form(Box<entry_form::Model>),
    Confirm(Confirm),
}

pub struct Confirm {
    pub text: String,
    pub yes: Action,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Error,
    Busy,
}

pub struct Status {
    pub text: String,
    pub kind: StatusKind,
}

pub struct PushProgress {
    pub total: usize,
    pub done: usize,
}

pub struct App {
    pub deps: Deps,
    pub gateway: Option<Arc<dyn JiraGateway>>,
    pub config: Option<Config>,
    pub ledger: Ledger,
    pub remote: RemoteCache,
    pub screen: Screen,
    pub overlay: Option<Overlay>,
    pub status: Option<Status>,
    pub worker: Worker<Msg>,
    pub hits: HitRegistry,
    pub quit: bool,
    pub today: NaiveDate,
    pub push: Option<PushProgress>,
    pub search_req: u64,
    last_click: Option<(Instant, Position)>,
    drag: Option<(IssueKey, Position)>,
}

impl App {
    pub fn new(deps: Deps) -> Result<Self> {
        let today = Local::now().date_naive();
        let (ledger, ledger_err) = match deps.state_store.load() {
            Ok(l) => (l, None),
            Err(e) => (Ledger::default(), Some(e.to_string())),
        };
        let config = deps.config_store.load().unwrap_or(None);
        let gateway = config.as_ref().and_then(|c| (deps.gateway_factory)(&c.credentials).ok());
        let screen = if gateway.is_some() {
            Screen::Week(week::Model::new(today))
        } else {
            Screen::Setup(setup::Model::new(config.as_ref().map(|c| &c.credentials), None))
        };
        let mut app = Self {
            deps,
            gateway,
            config,
            ledger,
            remote: RemoteCache::default(),
            screen,
            overlay: None,
            status: None,
            worker: Worker::default(),
            hits: HitRegistry::default(),
            quit: false,
            today,
            push: None,
            search_req: 0,
            last_click: None,
            drag: None,
        };
        if let Some(e) = ledger_err {
            app.set_error(format!("state file unreadable, starting empty: {e}"));
        }
        if app.gateway.is_some() {
            app.start_sync();
        }
        Ok(app)
    }

    // ---- per-frame ----------------------------------------------------

    pub fn begin_tick(&mut self) {
        self.today = Local::now().date_naive();
    }

    pub fn tick(&mut self) {
        day::tick(self);
    }

    // ---- config helpers -----------------------------------------------

    pub fn default_start(&self) -> StartTime {
        self.config.as_ref().map(|c| c.default_start).unwrap_or(StartTime::NINE)
    }
    pub fn target_seconds(&self) -> u64 {
        self.config.as_ref().map(|c| c.target_seconds()).unwrap_or(8 * 3600)
    }
    pub fn weekly_target_seconds(&self) -> u64 {
        self.config.as_ref().map(|c| c.weekly_target_seconds()).unwrap_or(40 * 3600)
    }

    // ---- status -------------------------------------------------------

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some(Status { text: text.into(), kind: StatusKind::Info });
    }
    pub fn set_error(&mut self, text: impl Into<String>) {
        self.status = Some(Status { text: text.into(), kind: StatusKind::Error });
    }
    pub fn set_busy(&mut self, text: impl Into<String>) {
        self.status = Some(Status { text: text.into(), kind: StatusKind::Busy });
    }

    // ---- persistence --------------------------------------------------

    pub fn save_ledger(&mut self) {
        if let Err(e) = self.deps.state_store.save(&self.ledger) {
            self.set_error(format!("could not save state: {e}"));
        }
    }

    // ---- navigation ---------------------------------------------------

    pub fn go_setup(&mut self, banner: Option<String>) {
        self.overlay = None;
        self.screen = Screen::Setup(setup::Model::new(self.config.as_ref().map(|c| &c.credentials), banner));
    }
    pub fn go_week(&mut self, week: Week, selected: NaiveDate) {
        self.overlay = None;
        let mut m = week::Model::new(self.today);
        m.week = week;
        m.select_date(selected);
        self.screen = Screen::Week(m);
    }
    pub fn go_day(&mut self, date: NaiveDate) {
        self.overlay = None;
        self.screen = Screen::Day(day::Model::new(date));
    }
    pub fn open_form(&mut self, model: entry_form::Model) {
        self.overlay = Some(Overlay::Form(Box::new(model)));
    }
    pub fn confirm(&mut self, text: impl Into<String>, yes: Action) {
        self.overlay = Some(Overlay::Confirm(Confirm { text: text.into(), yes }));
    }
    pub fn close_overlay(&mut self) {
        self.overlay = None;
    }

    /// Issues the user can pick from without a network call.
    pub fn local_issues(&self) -> Vec<Issue> {
        let mut out: Vec<Issue> = Vec::new();
        for key in self.ledger.watchlist() {
            out.push(self.remote.issue(key).cloned().unwrap_or_else(|| Issue {
                key: key.clone(),
                summary: String::new(),
                status: String::new(),
            }));
        }
        for i in self.remote.mine.iter().chain(self.remote.history.iter()) {
            if !out.iter().any(|o| o.key == i.key) {
                out.push(i.clone());
            }
        }
        out
    }

    // ---- sync -----------------------------------------------------------

    pub fn start_sync(&mut self) {
        let Some(gateway) = self.gateway.clone() else { return };
        let Some(jql) = self.config.as_ref().map(|c| c.jql().to_string()) else { return };
        if self.remote.syncing {
            return;
        }
        self.remote.syncing = true;
        self.set_busy("syncing…");
        let input = SyncInput {
            jql,
            watchlist: self.ledger.watchlist().to_vec(),
            history_from: Week::containing(self.today).monday() - Days::new(21),
        };
        self.worker.spawn(move || {
            Msg::Synced(sync::run(gateway.as_ref(), &input).map_err(|e| SyncError {
                kind: crate::application::ports::classify(&e),
                message: e.to_string(),
            }))
        });
    }

    // ---- messages from workers ----------------------------------------

    pub fn on_msg(&mut self, msg: Msg) {
        match msg {
            Msg::ConnectionTested { creds, outcome } => setup::on_tested(self, creds, outcome),
            Msg::Synced(Ok(out)) => {
                self.remote.syncing = false;
                self.remote.offline = false;
                self.remote.account_id = Some(out.account_id);
                self.remote.display_name = out.display_name;
                let covered: Vec<IssueKey> =
                    out.mine.iter().chain(out.watched.iter()).chain(out.history.iter()).map(|i| i.key.clone()).collect();
                self.ledger.reconcile(&out.worklogs, &covered);
                self.remote.mine = out.mine;
                self.remote.watched = out.watched;
                self.remote.history = out.history;
                self.remote.worklogs = out.worklogs;
                self.save_ledger();
                self.set_status(format!(
                    "synced · {} issues · {} worklogs",
                    self.remote.mine.len() + self.remote.watched.len() + self.remote.history.len(),
                    self.remote.worklogs.len()
                ));
            }
            Msg::Synced(Err(e)) => {
                self.remote.syncing = false;
                match e.kind {
                    GatewayErrorKind::Unauthorized => self.go_setup(Some("✗ 401 token rejected — create a new one and paste below".into())),
                    GatewayErrorKind::Network => {
                        self.remote.offline = true;
                        self.set_error(format!("offline: {}", e.message));
                    }
                    _ => self.set_error(format!("sync failed: {}", e.message)),
                }
            }
            Msg::SearchDone { req_id, query, result } => day::on_search_done(self, req_id, query, result),
            Msg::PushProgress { id, result } => push::on_progress(self, id, result),
            Msg::PushDone { ok, failed } => push::on_done(self, ok, failed),
        }
    }

    // ---- input ----------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.dispatch(Action::ForceQuit);
            return;
        }
        let action = match &self.overlay {
            Some(Overlay::Confirm(_)) => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(Action::ConfirmYes),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(Action::ConfirmNo),
                _ => None,
            },
            Some(Overlay::Form(m)) => entry_form::keys(m, &key),
            None => match &self.screen {
                Screen::Setup(m) => setup::keys(m, &key),
                Screen::Week(m) => week::keys(m, &key),
                Screen::Day(m) => day::keys(m, &key),
            },
        };
        if let Some(a) = action {
            self.dispatch(a);
        }
    }

    pub fn on_paste(&mut self, text: String) {
        let action = match &self.overlay {
            Some(Overlay::Form(_)) => Some(Action::Form(entry_form::Action::Paste(text))),
            Some(Overlay::Confirm(_)) => None,
            None => match &self.screen {
                Screen::Setup(_) => Some(Action::Setup(setup::Action::Paste(text))),
                Screen::Day(_) => Some(Action::Day(day::Action::Paste(text))),
                Screen::Week(_) => None,
            },
        };
        if let Some(a) = action {
            self.dispatch(a);
        }
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        let pos = Position { x: m.column, y: m.row };
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let now = Instant::now();
                let is_double = matches!(self.last_click, Some((t, p)) if now.duration_since(t) < DOUBLE_CLICK && p == pos);
                self.last_click = Some((now, pos));
                let Some(area) = self.hits.at(pos) else { return };
                self.drag = area.drag.clone().map(|k| (k, pos));
                let action = if is_double && area.double.is_some() {
                    area.double.clone()
                } else if let Some(f) = &area.click_at {
                    Some(f(pos.x.saturating_sub(area.rect.x)))
                } else {
                    area.click.clone()
                };
                if let Some(a) = action {
                    self.dispatch(a);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some((key, from)) = self.drag.take()
                    && from != pos && self.hits.find(pos, |a| a.drop_target).is_some() {
                        self.dispatch(Action::Day(day::Action::StageKey(key)));
                    }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let up = matches!(m.kind, MouseEventKind::ScrollUp);
                let action = self
                    .hits
                    .find(pos, |a| if up { a.scroll_up.is_some() } else { a.scroll_down.is_some() })
                    .and_then(|a| if up { a.scroll_up.clone() } else { a.scroll_down.clone() });
                if let Some(a) = action {
                    self.dispatch(a);
                }
            }
            _ => {}
        }
    }

    // ---- dispatch -------------------------------------------------------

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::Nop => {}
            Action::ForceQuit => self.quit = true,
            Action::Quit => {
                let n = self.ledger.staged_count();
                if n > 0 {
                    self.confirm(format!("{n} staged entr{} not pushed, quit? [y/n]", if n == 1 { "y" } else { "ies" }), Action::ForceQuit);
                } else {
                    self.quit = true;
                }
            }
            Action::Refresh => self.start_sync(),
            Action::OpenSettings => self.go_setup(None),
            Action::ConfirmYes => {
                if let Some(Overlay::Confirm(c)) = self.overlay.take() {
                    self.dispatch(c.yes);
                }
            }
            Action::ConfirmNo => {
                if matches!(self.overlay, Some(Overlay::Confirm(_))) {
                    self.overlay = None;
                }
            }
            Action::PushRequest(scope) => push::request(self, scope),
            Action::PushConfirmed(scope) => push::confirmed(self, scope),
            Action::Setup(a) => setup::update(self, a),
            Action::Week(a) => week::update(self, a),
            Action::Day(a) => day::update(self, a),
            Action::Form(a) => entry_form::update(self, a),
        }
    }

    pub fn push_scope_label(&self, scope: PushScope) -> String {
        match scope {
            PushScope::Day(d) => d.format("%a %d %b").to_string(),
            PushScope::Week(w) => format!("week {} → {}", w.monday().format("%d %b"), w.sunday().format("%d %b")),
        }
    }
}
