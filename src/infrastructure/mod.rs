//! Adapters: HTTP, files, threads. Implements `application::ports`.

pub mod jira_http;
pub mod json_state;
pub mod paths;
pub mod toml_config;
pub mod worker;

pub use jira_http::HttpJiraGateway;
pub use json_state::JsonStateStore;
pub use toml_config::TomlConfigStore;
pub use worker::Worker;
#[cfg(test)]
mod toml_config_tests;
#[cfg(test)]
mod toml_config_sections_tests;
