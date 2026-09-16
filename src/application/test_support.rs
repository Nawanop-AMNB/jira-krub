//! In-memory `JiraGateway` for use-case tests. Responses are scripted up
//! front; every call is recorded so tests can assert on what was sent.

use crate::application::ports::{GatewayError, GatewayErrorKind, JiraGateway, Me, NewWorklog};
use crate::domain::{Issue, IssueKey, RemoteWorklog};
use anyhow::{Result, anyhow};
use chrono::{DateTime, FixedOffset, TimeZone};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// What the fake answers with. Anything left `None`/empty falls back to a
/// sensible success value (see each method).
#[derive(Default)]
pub struct Script {
    pub me: Option<Me>,
    pub myself_error: Option<GatewayError>,
    /// One result per `search_issues` call, consumed in order. When the
    /// queue runs dry the fake answers `Ok(vec![])`.
    pub searches: VecDeque<Result<Vec<Issue>, GatewayError>>,
    /// `get_issue` by key. Missing keys answer `NotFound`.
    pub issues: HashMap<String, Issue>,
    /// `my_worklogs` by key. Missing keys answer an empty list.
    pub worklogs: HashMap<String, Vec<RemoteWorklog>>,
    pub worklogs_error: Option<GatewayError>,
    /// Default: echoes the request back with id `"created"`.
    pub add_result: Option<Result<RemoteWorklog, GatewayError>>,
    /// Default: echoes the request back with the given worklog id.
    pub update_result: Option<Result<RemoteWorklog, GatewayError>>,
    pub delete_error: Option<GatewayError>,
}

#[derive(Default, Debug)]
pub struct Calls {
    pub myself: usize,
    pub jql: Vec<String>,
    pub get_issue: Vec<IssueKey>,
    /// (issue key, account id)
    pub my_worklogs: Vec<(IssueKey, String)>,
    pub added: Vec<NewWorklog>,
    /// (worklog id, request)
    pub updated: Vec<(String, NewWorklog)>,
    /// (issue key, worklog id)
    pub deleted: Vec<(IssueKey, String)>,
}

pub struct FakeGateway {
    script: Mutex<Script>,
    calls: Mutex<Calls>,
}

impl FakeGateway {
    pub fn new(script: Script) -> Self {
        Self { script: Mutex::new(script), calls: Mutex::new(Calls::default()) }
    }

    pub fn calls(&self) -> std::sync::MutexGuard<'_, Calls> {
        self.calls.lock().unwrap()
    }
}

impl Default for FakeGateway {
    fn default() -> Self {
        Self::new(Script::default())
    }
}

fn to_anyhow(e: GatewayError) -> anyhow::Error {
    anyhow!(e)
}

fn echo(req: &NewWorklog, id: &str) -> RemoteWorklog {
    RemoteWorklog {
        id: id.to_string(),
        issue_key: req.issue_key.clone(),
        started: req.started.fixed_offset(),
        seconds: req.seconds,
        comment: req.comment_paragraphs.join(" "),
    }
}

impl JiraGateway for FakeGateway {
    fn myself(&self) -> Result<Me> {
        self.calls().myself += 1;
        let s = self.script.lock().unwrap();
        if let Some(e) = &s.myself_error {
            return Err(to_anyhow(e.clone()));
        }
        Ok(s.me.clone().unwrap_or_else(|| me("acc-1", "Fake User")))
    }

    fn search_issues(&self, jql: &str, _max: usize) -> Result<Vec<Issue>> {
        self.calls().jql.push(jql.to_string());
        match self.script.lock().unwrap().searches.pop_front() {
            Some(Ok(v)) => Ok(v),
            Some(Err(e)) => Err(to_anyhow(e)),
            None => Ok(Vec::new()),
        }
    }

    fn get_issue(&self, key: &IssueKey) -> Result<Issue> {
        self.calls().get_issue.push(key.clone());
        self.script
            .lock()
            .unwrap()
            .issues
            .get(key.as_str())
            .cloned()
            .ok_or_else(|| to_anyhow(gateway_error(GatewayErrorKind::NotFound, format!("issue {key}: HTTP 404"))))
    }

    fn my_worklogs(&self, key: &IssueKey, account_id: &str) -> Result<Vec<RemoteWorklog>> {
        self.calls().my_worklogs.push((key.clone(), account_id.to_string()));
        let s = self.script.lock().unwrap();
        if let Some(e) = &s.worklogs_error {
            return Err(to_anyhow(e.clone()));
        }
        Ok(s.worklogs.get(key.as_str()).cloned().unwrap_or_default())
    }

    fn add_worklog(&self, req: &NewWorklog) -> Result<RemoteWorklog> {
        self.calls().added.push(req.clone());
        match self.script.lock().unwrap().add_result.clone() {
            Some(Ok(w)) => Ok(w),
            Some(Err(e)) => Err(to_anyhow(e)),
            None => Ok(echo(req, "created")),
        }
    }

    fn update_worklog(&self, worklog_id: &str, req: &NewWorklog) -> Result<RemoteWorklog> {
        self.calls().updated.push((worklog_id.to_string(), req.clone()));
        match self.script.lock().unwrap().update_result.clone() {
            Some(Ok(w)) => Ok(w),
            Some(Err(e)) => Err(to_anyhow(e)),
            None => Ok(echo(req, worklog_id)),
        }
    }

    fn delete_worklog(&self, issue_key: &IssueKey, worklog_id: &str) -> Result<()> {
        self.calls().deleted.push((issue_key.clone(), worklog_id.to_string()));
        match &self.script.lock().unwrap().delete_error {
            Some(e) => Err(to_anyhow(e.clone())),
            None => Ok(()),
        }
    }
}

// ---- builders shared by tests ---------------------------------------------

pub fn me(account_id: &str, display_name: &str) -> Me {
    Me { account_id: account_id.into(), display_name: display_name.into() }
}

pub fn gateway_error(kind: GatewayErrorKind, message: impl Into<String>) -> GatewayError {
    GatewayError { kind, message: message.into() }
}

pub fn key(s: &str) -> IssueKey {
    IssueKey::parse(s).unwrap()
}

pub fn issue(k: &str) -> Issue {
    Issue { key: key(k), summary: format!("summary of {k}"), status: "To Do".into() }
}

/// A remote worklog started at 09:00 in UTC+7 on the given day.
pub fn remote(id: &str, k: &str, date: (i32, u32, u32), seconds: u64) -> RemoteWorklog {
    let started: DateTime<FixedOffset> =
        FixedOffset::east_opt(7 * 3600).unwrap().with_ymd_and_hms(date.0, date.1, date.2, 9, 0, 0).unwrap();
    RemoteWorklog { id: id.into(), issue_key: key(k), started, seconds, comment: String::new() }
}
