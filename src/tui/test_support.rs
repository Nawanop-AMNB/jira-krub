//! Test-only wiring: a real `App` built on temp files and a `FakeGateway`,
//! with a deterministic replacement for the event loop's message pump.

use crate::application::test_support::{FakeGateway, Script};
use crate::application::{JiraGateway, JiraGatewayFactory};
use crate::domain::Issue;
use crate::infrastructure::{JsonStateStore, TomlConfigStore};
use crate::tui::app::App;
use crate::tui::deps::Deps;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
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

pub struct Harness {
    pub app: App,
    pub gateway: Arc<FakeGateway>,
    /// Kept so the temp files outlive the app.
    _home: TempHome,
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

    let deps = Deps {
        config_store: Arc::new(TomlConfigStore::at(config_path)),
        state_store: Arc::new(JsonStateStore::at(home.file("state.json"))),
        gateway_factory,
    };
    let mut app = App::new(deps).expect("app starts");
    settle(&mut app);
    Harness { app, gateway, _home: home }
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
