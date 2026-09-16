//! `config.toml` sections: `[global]`, `[year.YYYY]` and `[connection]`
//! precedence over the legacy flat keys (R16).

use super::toml_config::TomlConfigStore;
use crate::application::config::{GlobalSettings, Holiday, YearSettings};
use crate::application::{Config, ConfigStore, Credentials};
use crate::domain::{SiteUrl, StartTime};
use chrono::NaiveDate;
use std::path::PathBuf;

/// Unique temp directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("jira-krub-tomlsec-{}-{name}", std::process::id()));
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

const CONNECTION: &str = "[connection]\nbase_url = \"https://acme.atlassian.net\"\nemail = \"me@example.com\"\napi_token = \"tok\"\n";

/// Write `[connection]` plus `extra`, then load.
fn load(dir: &TempDir, extra: &str) -> anyhow::Result<Option<Config>> {
    let path = dir.file("config.toml");
    std::fs::write(&path, format!("{CONNECTION}{extra}")).unwrap();
    TomlConfigStore::at(path).load()
}

#[test]
fn global_workdays_load_into_the_weekday_flags() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("workdays");
    let cfg = load(&dir, "\n[global]\nworkdays = [\"mon\", \"wed\"]\n").unwrap().expect("loads");
    assert_eq!(cfg.global.workdays, [true, false, true, false, false, false, false]);
}

#[test]
fn unknown_workday_names_are_ignored() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("bad-workdays");
    let cfg = load(&dir, "\n[global]\nworkdays = [\"MON\", \"funday\", \" fri \"]\n").unwrap().expect("loads");
    assert_eq!(cfg.global.workdays, [true, false, false, false, true, false, false], "case and padding are tolerated, nonsense is dropped");
}

#[test]
fn quick_stage_is_parsed_from_the_jira_duration_grammar() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("quick-stage");
    let cfg = load(&dir, "\n[global]\nquick_stage = \"30m\"\n").unwrap().expect("loads");
    assert_eq!(cfg.global.quick_stage_seconds, 1800);
}

#[test]
fn an_unparseable_quick_stage_duration_fails_the_load() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("bad-quick-stage");
    let err = load(&dir, "\n[global]\nquick_stage = \"1.5h\"\n").expect_err("a bad duration is an error");
    assert!(format!("{err:#}").contains("quick_stage"), "{err:#}");
}

#[test]
fn an_unparseable_holiday_date_names_its_year_in_the_error() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("bad-holiday");
    let err = load(&dir, "\n[year.2026]\nholidays = [{ date = \"nope\", name = \"New Year\" }]\n")
        .expect_err("a bad holiday date is an error");
    let text = format!("{err:#}");
    assert!(text.contains("2026"), "the year is in the message: {text}");
    assert!(text.contains("nope"), "the offending value is in the message: {text}");
}

#[test]
fn the_connection_section_wins_over_the_legacy_top_level_keys() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("precedence");
    let path = dir.file("config.toml");
    std::fs::write(
        &path,
        "base_url = \"https://legacy.atlassian.net\"\nemail = \"legacy@example.com\"\napi_token = \"legacy-tok\"\n\n[connection]\nbase_url = \"https://acme.atlassian.net\"\nemail = \"new@example.com\"\napi_token = \"new-tok\"\n",
    )
    .unwrap();
    let cfg = TomlConfigStore::at(path).load().unwrap().expect("loads");
    assert_eq!(cfg.credentials.site.as_str(), "https://acme.atlassian.net");
    assert_eq!(cfg.credentials.email, "new@example.com");
    assert_eq!(cfg.credentials.api_token, "new-tok");
}

#[test]
fn a_file_without_a_global_section_uses_the_global_defaults() {
    if env_token_is_set() {
        eprintln!("skipped: JIRA_API_TOKEN is set in the environment");
        return;
    }
    let dir = TempDir::new("no-global");
    let cfg = load(&dir, "").unwrap().expect("loads");
    assert_eq!(cfg.global, GlobalSettings::default());
}

#[test]
fn years_with_empty_settings_are_not_written_on_save() {
    let dir = TempDir::new("empty-year");
    let path = dir.file("config.toml");
    let mut cfg = Config::with_defaults(Credentials {
        site: SiteUrl::parse("https://acme.atlassian.net").unwrap(),
        email: "me@example.com".into(),
        api_token: "tok".into(),
    });
    cfg.global.default_start = StartTime::parse("09:00").unwrap();
    cfg.years.insert(2025, YearSettings::default());
    cfg.years.insert(
        2026,
        YearSettings {
            hours_per_day: Some(6),
            holidays: vec![Holiday { date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), name: "New Year".into() }],
        },
    );
    TomlConfigStore::at(path.clone()).save(&cfg).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[year.2026]"), "{text}");
    assert!(!text.contains("[year.2025]"), "an empty year is not written: {text}");
}
