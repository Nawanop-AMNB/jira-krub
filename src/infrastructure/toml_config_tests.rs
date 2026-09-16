use super::toml_config::TomlConfigStore;
use crate::application::config::{GlobalSettings, Holiday, YearSettings};
use crate::application::{Config, ConfigStore, Credentials};
use chrono::NaiveDate;
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
        global: GlobalSettings {
            hours_per_day: 7,
            workdays: [true, true, true, true, false, false, true],
            default_start: StartTime::parse("10:15").unwrap(),
            quick_stage_seconds: 1800,
            lookback_weeks: 5,
            auto_watch: false,
        },
        years: [(
            2026,
            YearSettings {
                hours_per_day: Some(6),
                holidays: vec![Holiday { date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), name: "New Year".into() }],
            },
        )]
        .into(),
        jql: Some("project = KAN".into()),
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
    assert_eq!(loaded.global, GlobalSettings::default());
    assert_eq!(loaded.jql, None);
    assert!(loaded.years.is_empty());
}

#[test]
fn legacy_flat_file_still_loads() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("legacy");
    let path = dir.file("config.toml");
    std::fs::write(
        &path,
        "base_url = \"https://acme.atlassian.net\"\nemail = \"me@example.com\"\napi_token = \"tok\"\nhours_per_day = 7\ndefault_start_time = \"10:00\"\njql = \"project = X\"\n",
    )
    .unwrap();
    let loaded = TomlConfigStore::at(path).load().unwrap().expect("legacy loads");
    assert_eq!(loaded.global.hours_per_day, 7);
    assert_eq!(loaded.global.default_start, StartTime::parse("10:00").unwrap());
    assert_eq!(loaded.jql.as_deref(), Some("project = X"));
}

#[test]
fn saved_file_uses_sections_and_holidays_roundtrip_sorted() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("sections");
    let store = TomlConfigStore::at(dir.file("config.toml"));
    let mut c = config();
    c.years.get_mut(&2026).unwrap().holidays.insert(0, Holiday { date: NaiveDate::from_ymd_opt(2026, 4, 13).unwrap(), name: "Songkran".into() });
    store.save(&c).unwrap();
    let text = std::fs::read_to_string(dir.file("config.toml")).unwrap();
    assert!(text.contains("[connection]") && text.contains("[global]") && text.contains("[year.2026]"), "{text}");
    let loaded = store.load().unwrap().unwrap();
    let dates: Vec<String> = loaded.years[&2026].holidays.iter().map(|h| h.date.to_string()).collect();
    assert_eq!(dates, vec!["2026-01-01", "2026-04-13"], "sorted on load");
}
