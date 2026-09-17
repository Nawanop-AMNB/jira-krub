//! Test-only wiring: a real `App` built on temp files and a `FakeGateway`,
//! with a deterministic replacement for the event loop's message pump.

use crate::application::test_support::{FakeGateway, Script};
use crate::application::{JiraGateway, JiraGatewayFactory, UrlOpener};
use crate::domain::Issue;
use crate::infrastructure::{JsonStateStore, TomlConfigStore};
use crate::tui::app::App;
use crate::tui::deps::Deps;
use crate::tui::msg::Msg;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A config file good enough to open the main screen. The start time and
/// quick-stage duration are deliberately not the built-in defaults, so tests
/// can prove those values come from config.
pub const CONFIG_TOML: &str = "\
[connection]
base_url = \"https://acme.atlassian.net\"
email = \"me@example.com\"
api_token = \"test-token\"

[global]
hours_per_day = 8
workdays = [\"mon\", \"tue\", \"wed\", \"thu\", \"fri\"]
default_start_time = \"13:00\"
quick_stage = \"45m\"
lookback_weeks = 3
auto_watch = true
";

/// Unique temp directory for one test's config + state, removed on drop.
struct TempHome(PathBuf);

impl TempHome {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("jira-krub-tui-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Records what the app asked to open, and can be made to fail on demand.
#[derive(Default)]
pub struct FakeOpener {
    urls: Mutex<Vec<String>>,
    fail: Mutex<Option<String>>,
}

impl FakeOpener {
    pub fn urls(&self) -> Vec<String> {
        self.urls.lock().unwrap().clone()
    }
    /// Every subsequent `open()` fails with this message instead of recording.
    pub fn set_fail(&self, message: impl Into<String>) {
        *self.fail.lock().unwrap() = Some(message.into());
    }
}

impl UrlOpener for FakeOpener {
    fn open(&self, url: &str) -> anyhow::Result<()> {
        if let Some(msg) = self.fail.lock().unwrap().clone() {
            return Err(anyhow::anyhow!(msg));
        }
        self.urls.lock().unwrap().push(url.to_string());
        Ok(())
    }
}

pub struct Harness {
    pub app: App,
    pub gateway: Arc<FakeGateway>,
    pub opener: Arc<FakeOpener>,
    /// Kept so the temp files outlive the app.
    _home: TempHome,
}

impl Harness {
    /// Path to the state (ledger) file backing this harness, for tests that
    /// need to prove it was or wasn't touched.
    pub fn state_file(&self) -> PathBuf {
        self._home.file("state.json")
    }
}

/// The two searches one sync performs: "work for me", then history.
pub fn script_with_issues(mine: Vec<Issue>) -> Script {
    Script { searches: VecDeque::from(vec![Ok(mine), Ok(Vec::new())]), ..Script::default() }
}

/// `name` must be unique per test: it names the temp directory.
pub fn harness(name: &str, script: Script) -> Harness {
    harness_with_config(name, script, CONFIG_TOML)
}

pub fn harness_with_config(name: &str, script: Script, config_toml: &str) -> Harness {
    let home = TempHome::new(name);
    let config_path = home.file("config.toml");
    std::fs::write(&config_path, config_toml).unwrap();

    let gateway = Arc::new(FakeGateway::new(script));
    let for_factory = gateway.clone();
    let gateway_factory: JiraGatewayFactory = Arc::new(move |_creds| Ok(for_factory.clone() as Arc<dyn JiraGateway>));

    let opener = Arc::new(FakeOpener::default());
    let deps = Deps {
        config_store: Arc::new(TomlConfigStore::at(config_path)),
        state_store: Arc::new(JsonStateStore::at(home.file("state.json"))),
        gateway_factory,
        opener: opener.clone(),
    };
    let mut app = App::new(deps).expect("app starts");
    settle(&mut app);
    Harness { app, gateway, opener, _home: home }
}

/// Startup opens the My tasks tab (R17); week, day and popup tests want the
/// Worklog one.
pub fn go_worklog(app: &mut App) {
    app.dispatch(crate::tui::action::Action::SetMainTab(crate::tui::app::MainTab::Worklog));
}

/// Draw one frame the way the event loop does, into an off-screen buffer wide
/// enough for the 80-column mockups plus margin.
fn frame(app: &mut App) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(120, 30)).expect("test backend");
    let mut hits = crate::tui::hit::HitRegistry::default();
    term.draw(|f| crate::tui::view::draw(f, app, &mut hits)).expect("draw");
    // the real loop keeps the hit map from the last frame, so clicks work next
    app.hits = hits;
    term.backend().buffer().clone()
}

/// One rendered frame as plain text, one `String` per row.
pub fn render(app: &mut App) -> Vec<String> {
    let buf = frame(app);
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>()).collect()
}

/// The same frame as styles, one row of cells per line.
pub fn render_styles(app: &mut App) -> Vec<Vec<Style>> {
    let buf = frame(app);
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].style()).collect()).collect()
}

/// Stand in for the event loop: drain the worker and apply every message
/// until the background sync has landed. Bounded at roughly one second so a
/// wedged test fails instead of hanging.
pub fn settle(app: &mut App) {
    for _ in 0..100 {
        let msgs = app.worker.drain();
        let idle = msgs.is_empty();
        for m in msgs {
            app.on_msg(m);
        }
        if idle && !app.remote.syncing {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("background work did not settle within 1s");
}

/// Wait for the worker to deliver at least one message and return every
/// message that has arrived so far, without applying any of it. Lets a test
/// inspect app state after just the first streamed message (e.g.
/// `Msg::SyncMine`) instead of draining all the way to `Msg::Synced` like
/// `settle` does. Bounded at roughly one second so a wedged test fails
/// instead of hanging.
pub fn drain_until_nonempty(app: &mut App) -> Vec<Msg> {
    for _ in 0..100 {
        let msgs = app.worker.drain();
        if !msgs.is_empty() {
            return msgs;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no message arrived within 1s");
}
