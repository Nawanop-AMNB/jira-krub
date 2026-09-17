//! In-memory `JiraGateway` for use-case tests. Responses are scripted up
//! front; every call is recorded so tests can assert on what was sent.

use crate::application::ports::{GatewayError, GatewayErrorKind, IssueWithWorklogs, JiraGateway, Me, NewWorklog, Window};
use crate::domain::{Issue, IssueKey, RemoteWorklog};
use anyhow::{Result, anyhow};
use chrono::{DateTime, FixedOffset, TimeZone};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

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
    /// Embedded page attached to search results by key: (my worklogs, total).
    /// Missing keys answer (empty, 0).
    pub embedded: HashMap<String, (Vec<RemoteWorklog>, u64)>,
    pub worklogs_error: Option<GatewayError>,
    /// Default: echoes the request back with id `"created"`.
    pub add_result: Option<Result<RemoteWorklog, GatewayError>>,
    /// Default: echoes the request back with the given worklog id.
    pub update_result: Option<Result<RemoteWorklog, GatewayError>>,
    pub delete_error: Option<GatewayError>,
    /// Simulated per-request latency, applied inside every gateway method
    /// that represents one network round trip. Lets concurrency tests prove
    /// requests overlap instead of running one after another.
    pub delay: Duration,
}

#[derive(Default, Debug)]
pub struct Calls {
    pub myself: usize,
    pub jql: Vec<String>,
    /// Account id each `search_issues_with_worklogs` asked to filter by.
    pub search_account_ids: Vec<String>,
    pub get_issue: Vec<IssueKey>,
    /// (issue key, account id, window)
    pub my_worklogs: Vec<(IssueKey, String, Option<Window>)>,
    pub added: Vec<NewWorklog>,
    /// (worklog id, request)
    pub updated: Vec<(String, NewWorklog)>,
    /// (issue key, worklog id)
    pub deleted: Vec<(IssueKey, String)>,
}

pub struct FakeGateway {
    script: Mutex<Script>,
    calls: Mutex<Calls>,
    in_flight: AtomicUsize,
    peak_in_flight: AtomicUsize,
}

/// Marks one gateway call as in flight for the lifetime of the guard, and
/// applies the script's simulated latency before the call's own work runs.
struct InFlightGuard<'a> {
    gateway: &'a FakeGateway,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.gateway.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

impl FakeGateway {
    pub fn new(script: Script) -> Self {
        Self { script: Mutex::new(script), calls: Mutex::new(Calls::default()), in_flight: AtomicUsize::new(0), peak_in_flight: AtomicUsize::new(0) }
    }

    pub fn calls(&self) -> std::sync::MutexGuard<'_, Calls> {
        self.calls.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The largest number of gateway calls this fake ever had in flight at
    /// once. Reset only by constructing a new `FakeGateway`.
    pub fn peak_in_flight(&self) -> usize {
        self.peak_in_flight.load(Ordering::SeqCst)
    }

    /// Call at the top of every trait method that represents one network
    /// round trip: records the call as in flight and sleeps for the
    /// configured delay before returning the guard that un-counts it.
    fn enter(&self) -> InFlightGuard<'_> {
        let now_in_flight = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_in_flight.fetch_max(now_in_flight, Ordering::SeqCst);
        let delay = self.script.lock().unwrap_or_else(|e| e.into_inner()).delay;
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        InFlightGuard { gateway: self }
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
        let _guard = self.enter();
        self.calls().myself += 1;
        let s = self.script.lock().unwrap();
        if let Some(e) = &s.myself_error {
            return Err(to_anyhow(e.clone()));
        }
        Ok(s.me.clone().unwrap_or_else(|| me("acc-1", "Fake User")))
    }

    fn search_issues(&self, jql: &str, _max: usize) -> Result<Vec<Issue>> {
        let _guard = self.enter();
        self.calls().jql.push(jql.to_string());
        match self.script.lock().unwrap().searches.pop_front() {
            Some(Ok(v)) => Ok(v),
            Some(Err(e)) => Err(to_anyhow(e)),
            None => Ok(Vec::new()),
        }
    }

    fn search_issues_with_worklogs(&self, jql: &str, max: usize, account_id: &str) -> Result<Vec<IssueWithWorklogs>> {
        self.calls().search_account_ids.push(account_id.to_string());
        let issues = self.search_issues(jql, max)?;
        let s = self.script.lock().unwrap();
        Ok(issues
            .into_iter()
            .map(|issue| {
                let (my_worklogs, total) = s.embedded.get(issue.key.as_str()).cloned().unwrap_or_default();
                IssueWithWorklogs { issue, my_worklogs, total }
            })
            .collect())
    }

    fn get_issue(&self, key: &IssueKey) -> Result<Issue> {
        let _guard = self.enter();
        self.calls().get_issue.push(key.clone());
        self.script
            .lock()
            .unwrap()
            .issues
            .get(key.as_str())
            .cloned()
            .ok_or_else(|| to_anyhow(gateway_error(GatewayErrorKind::NotFound, format!("issue {key}: HTTP 404"))))
    }

    fn my_worklogs(&self, key: &IssueKey, account_id: &str, window: Option<&Window>) -> Result<Vec<RemoteWorklog>> {
        let _guard = self.enter();
        self.calls().my_worklogs.push((key.clone(), account_id.to_string(), window.copied()));
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
    Issue::new(key(k), format!("summary of {k}"), "To Do")
}

/// A remote worklog started at 09:00 in UTC+7 on the given day.
pub fn remote(id: &str, k: &str, date: (i32, u32, u32), seconds: u64) -> RemoteWorklog {
    let started: DateTime<FixedOffset> =
        FixedOffset::east_opt(7 * 3600).unwrap().with_ymd_and_hms(date.0, date.1, date.2, 9, 0, 0).unwrap();
    RemoteWorklog { id: id.into(), issue_key: key(k), started, seconds, comment: String::new() }
}
