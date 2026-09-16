//! Composition root: wire infrastructure into the UI and run.

mod application;
mod domain;
mod infrastructure;
mod tui;

use anyhow::Result;
use application::JiraGatewayFactory;
use infrastructure::{HttpJiraGateway, JsonStateStore, TomlConfigStore};
use std::sync::Arc;
use tui::deps::Deps;

fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("jira-krub {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let gateway_factory: JiraGatewayFactory =
        Arc::new(|creds| Ok(Arc::new(HttpJiraGateway::new(creds)?) as Arc<dyn application::JiraGateway>));
    let deps = Deps {
        config_store: Arc::new(TomlConfigStore::default_location()?),
        state_store: Arc::new(JsonStateStore::default_location()?),
        gateway_factory,
    };
    tui::run(deps)
}
