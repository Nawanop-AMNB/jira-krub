use super::toml_config::TomlConfigStore;
use crate::application::{Config, ConfigStore, Credentials};
use crate::domain::{SiteUrl, StartTime};
use std::path::PathBuf;

/// Unique temp directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("jira-krub-toml-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `JIRA_API_TOKEN` is process-global and overrides the file; tests that
/// depend on the file's token bail out rather than mutate the environment.
fn env_token_is_set() -> bool {
    std::env::var("JIRA_API_TOKEN").is_ok_and(|t| !t.is_empty())
}

fn config() -> Config {
    Config {
        credentials: Credentials {
            site: SiteUrl::parse("acme.atlassian.net").unwrap(),
            email: "me@example.com".into(),
            api_token: "secret-token".into(),
        },
        hours_per_day: 7,
        jql: Some("project = KAN".into()),
        default_start: StartTime::parse("10:15").unwrap(),
    }
}

#[test]
fn load_returns_none_when_file_is_missing() {
    let dir = TempDir::new("missing");
    let store = TomlConfigStore::at(dir.file("config.toml"));
    assert_eq!(store.load().unwrap(), None);
}

#[test]
fn load_returns_none_when_file_has_no_token() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("no-token");
    let path = dir.file("config.toml");
    std::fs::write(&path, "base_url = \"https://acme.atlassian.net\"\nemail = \"me@example.com\"\n").unwrap();
    assert_eq!(TomlConfigStore::at(path).load().unwrap(), None);
}

#[test]
fn save_then_load_roundtrips_every_field() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("roundtrip");
    let store = TomlConfigStore::at(dir.file("config.toml"));
    store.save(&config()).unwrap();
    assert_eq!(store.load().unwrap(), Some(config()));
}

#[test]
fn location_is_the_path() {
    let dir = TempDir::new("location");
    let path = dir.file("config.toml");
    assert_eq!(TomlConfigStore::at(path.clone()).location(), path.display().to_string());
}

#[cfg(unix)]
#[test]
fn saved_file_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new("mode");
    let path = dir.file("config.toml");
    TomlConfigStore::at(path.clone()).save(&config()).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn base_url_is_normalised_through_site_url() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("normalise");
    let path = dir.file("config.toml");
    std::fs::write(
        &path,
        "base_url = \"Acme.Atlassian.net/browse/X-1\"\nemail = \"me@example.com\"\napi_token = \"tok\"\n",
    )
    .unwrap();
    let loaded = TomlConfigStore::at(path).load().unwrap().expect("config with token loads");
    assert_eq!(loaded.credentials.site.as_str(), "https://acme.atlassian.net");
    assert_eq!(loaded.hours_per_day, 8);
    assert_eq!(loaded.jql, None);
    assert_eq!(loaded.default_start, StartTime::NINE);
}
