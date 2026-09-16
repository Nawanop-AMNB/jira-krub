use crate::application::StateStore;
use crate::domain::{Entry, EntryId, EntryState, Intent, IssueKey, Ledger, StartTime};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---- DTOs (persistence shape, decoupled from domain) --------------------

#[derive(Serialize, Deserialize, Default)]
struct StateFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: Vec<EntryDto>,
    #[serde(default)]
    watchlist: Vec<WatchDto>,
}

#[derive(Serialize, Deserialize)]
struct EntryDto {
    id: String,
    issue_key: String,
    date: NaiveDate,
    start: String,
    seconds: u64,
    #[serde(default)]
    title: String,
    /// Legacy (pre-0.3) second paragraph; folded into `title` on load.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    detail: String,
    state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worklog_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// For `failed`: "create" | "update" | "delete" (default create).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intent: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct WatchDto {
    key: String,
}

impl From<&Entry> for EntryDto {
    fn from(e: &Entry) -> Self {
        let (state, worklog_id, error, intent) = match &e.state {
            EntryState::Staged => ("staged", None, None, None),
            EntryState::Pushed { worklog_id } => ("pushed", Some(worklog_id.clone()), None, None),
            EntryState::Modified { worklog_id } => ("modified", Some(worklog_id.clone()), None, None),
            EntryState::Deleted { worklog_id } => ("deleted", Some(worklog_id.clone()), None, None),
            EntryState::Failed { error, intent } => {
                let (name, id) = match intent {
                    Intent::Create => ("create", None),
                    Intent::Update { worklog_id } => ("update", Some(worklog_id.clone())),
                    Intent::Delete { worklog_id } => ("delete", Some(worklog_id.clone())),
                };
                ("failed", id, Some(error.clone()), Some(name.to_string()))
            }
        };
        Self {
            id: e.id.as_str().to_string(),
            issue_key: e.issue_key.as_str().to_string(),
            date: e.date,
            start: e.start.to_string(),
            seconds: e.seconds,
            title: e.title.clone(),
            detail: String::new(),
            state: state.to_string(),
            worklog_id,
            error,
            intent,
        }
    }
}

impl TryFrom<EntryDto> for Entry {
    type Error = anyhow::Error;
    fn try_from(d: EntryDto) -> Result<Self> {
        let state = match (d.state.as_str(), d.worklog_id, d.error) {
            ("pushed", Some(id), _) => EntryState::Pushed { worklog_id: id },
            ("modified", Some(id), _) => EntryState::Modified { worklog_id: id },
            ("deleted", Some(id), _) => EntryState::Deleted { worklog_id: id },
            ("failed", id, err) => {
                let intent = match (d.intent.as_deref(), id) {
                    (Some("update"), Some(id)) => Intent::Update { worklog_id: id },
                    (Some("delete"), Some(id)) => Intent::Delete { worklog_id: id },
                    _ => Intent::Create,
                };
                EntryState::Failed { error: err.unwrap_or_default(), intent }
            }
            _ => EntryState::Staged,
        };
        Ok(Entry {
            id: EntryId::new(d.id),
            issue_key: IssueKey::parse(&d.issue_key)?,
            date: d.date,
            start: StartTime::parse(&d.start)?,
            seconds: d.seconds,
            title: if d.detail.trim().is_empty() { d.title } else { format!("{}\n{}", d.title, d.detail) },
            state,
        })
    }
}

// ---- store ---------------------------------------------------------------

pub struct JsonStateStore {
    path: PathBuf,
}

impl JsonStateStore {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn default_location() -> Result<Self> {
        Ok(Self::at(super::paths::state_file()?))
    }
}

impl StateStore for JsonStateStore {
    fn load(&self) -> Result<Ledger> {
        if !self.path.exists() {
            return Ok(Ledger::default());
        }
        let raw = std::fs::read_to_string(&self.path).with_context(|| format!("read {}", self.path.display()))?;
        let f: StateFile = serde_json::from_str(&raw).with_context(|| format!("parse {}", self.path.display()))?;
        let mut entries = Vec::with_capacity(f.entries.len());
        for d in f.entries {
            // Skip corrupt rows rather than refusing to start.
            if let Ok(e) = Entry::try_from(d) {
                entries.push(e);
            }
        }
        let watchlist = f
            .watchlist
            .into_iter()
            .filter_map(|w| IssueKey::parse(&w.key).ok())
            .collect();
        Ok(Ledger::new(entries, watchlist))
    }

    fn save(&self, ledger: &Ledger) -> Result<()> {
        let f = StateFile {
            version: 1,
            entries: ledger.entries().iter().map(EntryDto::from).collect(),
            watchlist: ledger
                .watchlist()
                .iter()
                .map(|w| WatchDto { key: w.as_str().to_string() })
                .collect(),
        };
        let body = serde_json::to_vec_pretty(&f)?;
        super::paths::write_atomic(&self.path, &body, Some(0o600))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("jira-krub-test-{}", std::process::id()));
        let store = JsonStateStore::at(dir.join("state.json"));
        let mut l = Ledger::default();
        l.add(Entry {
            id: EntryId::new("a".into()),
            issue_key: IssueKey::parse("X-1").unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 9, 16).unwrap(),
            start: StartTime::parse("13:30").unwrap(),
            seconds: 5400,
            title: "t\nd".into(),
            state: EntryState::Failed { error: "boom".into(), intent: Intent::Update { worklog_id: "5".into() } },
        });
        l.watch(&IssueKey::parse("OPS-7").unwrap());
        store.save(&l).unwrap();
        let back = store.load().unwrap();
        assert_eq!(back, l);
        let _ = std::fs::remove_dir_all(dir);
    }
}
