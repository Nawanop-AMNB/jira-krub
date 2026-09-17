//! What actually goes on the wire (R6, R7): method, path, query string and
//! body for each worklog call, the window filter, and the author filter.
//!
//! A throwaway HTTP server on loopback stands in for Jira; `SiteUrl` allows
//! plain `http://` for exactly this case.

use super::*;
use crate::application::Credentials;
use crate::domain::SiteUrl;
use chrono::NaiveDate;
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// One request as the fake server saw it.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    /// Path plus query string, exactly as sent.
    target: String,
    body: String,
}

impl Seen {
    fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }
    fn query(&self) -> HashMap<String, String> {
        match self.target.split_once('?') {
            None => HashMap::new(),
            Some((_, q)) => q
                .split('&')
                .filter(|p| !p.is_empty())
                .filter_map(|p| p.split_once('='))
                .map(|(k, v)| (k.to_string(), percent_decode(v)))
                .collect(),
        }
    }
}

/// Enough of percent-decoding for the values these tests send.
fn percent_decode(s: &str) -> String {
    let mut out = String::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v as char);
                    i += 3;
                    continue;
                }
                out.push('%');
                i += 1;
            }
            b'+' => {
                out.push(' ');
                i += 1;
            }
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}

/// Serve `bodies.len()` requests, one canned JSON body each, then stop.
/// Returns the base URL and the shared log of what was received.
fn fake_jira(bodies: Vec<serde_json::Value>) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for body in bodies {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                return;
            }
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("").to_string();
            let target = parts.next().unwrap_or("").to_string();
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" || line == "\n" {
                    break;
                }
                if let Some((k, v)) = line.split_once(':')
                    && k.eq_ignore_ascii_case("content-length")
                {
                    length = v.trim().parse().unwrap_or(0);
                }
            }
            let mut buf = vec![0u8; length];
            if length > 0 {
                let _ = reader.read_exact(&mut buf);
            }
            log.lock().unwrap().push(Seen { method, target, body: String::from_utf8_lossy(&buf).to_string() });
            let payload = body.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = stream.flush();
        }
    });
    (format!("http://127.0.0.1:{port}"), seen)
}

fn gateway(base: &str) -> HttpJiraGateway {
    HttpJiraGateway::new(&Credentials {
        site: SiteUrl::parse(base).expect("loopback http is allowed"),
        email: "me@example.com".into(),
        api_token: "tok".into(),
    })
    .unwrap()
}

fn key(s: &str) -> IssueKey {
    IssueKey::parse(s).unwrap()
}

fn request(req: &NewWorklog) -> NewWorklog {
    req.clone()
}

fn new_worklog() -> NewWorklog {
    NewWorklog {
        issue_key: key("A-1"),
        started: Local.with_ymd_and_hms(2026, 9, 16, 9, 0, 0).single().unwrap(),
        seconds: 3600,
        comment_paragraphs: vec!["fix expiry".into()],
    }
}

fn worklog_json(id: &str, account: &str, started: &str, seconds: u64) -> serde_json::Value {
    json!({
        "id": id,
        "started": started,
        "timeSpentSeconds": seconds,
        "author": { "accountId": account }
    })
}

/// A local timestamp formatted the way Jira sends `started`.
fn started_at(y: i32, m: u32, d: u32, h: u32) -> String {
    Local.with_ymd_and_hms(y, m, d, h, 0, 0).single().unwrap().format(STARTED_FMT).to_string()
}

fn only(seen: &Arc<Mutex<Vec<Seen>>>) -> Seen {
    let v = seen.lock().unwrap();
    assert_eq!(v.len(), 1, "exactly one request: {v:?}");
    v[0].clone()
}

// ---- R6: one HTTP verb per intent, always adjustEstimate=leave -------------

#[test]
fn a_new_entry_is_posted_to_the_issue_worklog_endpoint_with_adjust_estimate_leave() {
    let (base, seen) = fake_jira(vec![worklog_json("10042", "acc-1", &started_at(2026, 9, 16, 9), 3600)]);
    gateway(&base).add_worklog(&request(&new_worklog())).expect("created");

    let r = only(&seen);
    assert_eq!(r.method, "POST");
    assert_eq!(r.path(), "/rest/api/3/issue/A-1/worklog");
    assert_eq!(r.query().get("adjustEstimate").map(String::as_str), Some("leave"), "remaining estimate is never touched");
    let body: serde_json::Value = serde_json::from_str(&r.body).expect("json body");
    assert_eq!(body["timeSpentSeconds"], 3600);
    assert_eq!(body["comment"]["content"][0]["content"][0]["text"], "fix expiry");
    assert!(body["started"].as_str().unwrap().starts_with("2026-09-16T09:00:00"), "{body}");
}

#[test]
fn an_edited_entry_is_put_to_its_own_worklog_id_with_adjust_estimate_leave() {
    let (base, seen) = fake_jira(vec![worklog_json("77", "acc-1", &started_at(2026, 9, 16, 9), 3600)]);
    gateway(&base).update_worklog("77", &request(&new_worklog())).expect("updated");

    let r = only(&seen);
    assert_eq!(r.method, "PUT", "an edit must never create a second worklog");
    assert_eq!(r.path(), "/rest/api/3/issue/A-1/worklog/77");
    assert_eq!(r.query().get("adjustEstimate").map(String::as_str), Some("leave"));
}

#[test]
fn a_deleted_entry_is_deleted_by_id_with_adjust_estimate_leave() {
    let (base, seen) = fake_jira(vec![json!(null)]);
    gateway(&base).delete_worklog(&key("A-1"), "77").expect("deleted");

    let r = only(&seen);
    assert_eq!(r.method, "DELETE");
    assert_eq!(r.path(), "/rest/api/3/issue/A-1/worklog/77");
    assert_eq!(
        r.query().get("adjustEstimate").map(String::as_str),
        Some("leave"),
        "without it Jira gives the time back to the remaining estimate"
    );
}

#[test]
fn an_entry_without_a_description_is_posted_without_a_comment() {
    let (base, seen) = fake_jira(vec![worklog_json("1", "acc-1", &started_at(2026, 9, 16, 9), 3600)]);
    let mut req = new_worklog();
    req.comment_paragraphs.clear();
    gateway(&base).add_worklog(&req).expect("created");

    let body: serde_json::Value = serde_json::from_str(&only(&seen).body).unwrap();
    assert!(body.get("comment").is_none(), "title is optional: {body}");
}

// ---- R7: the window filter covers the whole last day ----------------------

#[test]
fn the_worklog_window_covers_every_hour_of_the_first_and_last_day() {
    let w = Window {
        from: NaiveDate::from_ymd_opt(2026, 9, 14).unwrap(),
        to: NaiveDate::from_ymd_opt(2026, 9, 20).unwrap(),
    };
    let (after, before) = window_millis(&w);
    let at = |y, m, d, h| Local.with_ymd_and_hms(y, m, d, h, 0, 0).single().unwrap().timestamp_millis();

    assert!(after <= at(2026, 9, 14, 0), "midnight on the first day is inside the window");
    assert!(
        before >= at(2026, 9, 20, 23),
        "an entry logged late on the last day must still be fetched — otherwise today's worklogs go missing"
    );
    assert!(after > at(2026, 9, 13, 23), "the day before the window is excluded");
    assert!(before < at(2026, 9, 21, 0), "the day after the window is excluded");
}

#[test]
fn a_windowed_worklog_fetch_sends_started_after_and_before() {
    let (base, seen) = fake_jira(vec![json!({ "worklogs": [], "total": 0 })]);
    let w = Window {
        from: NaiveDate::from_ymd_opt(2026, 9, 14).unwrap(),
        to: NaiveDate::from_ymd_opt(2026, 9, 20).unwrap(),
    };
    gateway(&base).my_worklogs(&key("OPS-7"), "acc-1", Some(&w)).expect("fetched");

    let r = only(&seen);
    assert_eq!(r.method, "GET");
    assert_eq!(r.path(), "/rest/api/3/issue/OPS-7/worklog");
    let q = r.query();
    let after: i64 = q.get("startedAfter").expect("startedAfter").parse().unwrap();
    let before: i64 = q.get("startedBefore").expect("startedBefore").parse().unwrap();
    let late_on_the_last_day = Local.with_ymd_and_hms(2026, 9, 20, 23, 0, 0).single().unwrap().timestamp_millis();
    assert!(after <= late_on_the_last_day && late_on_the_last_day <= before, "the whole last day is inside the server filter");
}

#[test]
fn a_fetch_without_a_window_sends_no_started_filter() {
    let (base, seen) = fake_jira(vec![json!({ "worklogs": [], "total": 0 })]);
    gateway(&base).my_worklogs(&key("OPS-7"), "acc-1", None).expect("fetched");
    let q = only(&seen).query();
    assert!(!q.contains_key("startedAfter"), "{q:?}");
    assert!(!q.contains_key("startedBefore"), "{q:?}");
}

// ---- R7: only my worklogs count ------------------------------------------

#[test]
fn a_per_issue_fetch_keeps_only_my_worklogs() {
    let (base, _seen) = fake_jira(vec![json!({
        "worklogs": [
            worklog_json("mine", "acc-1", &started_at(2026, 9, 16, 9), 3600),
            worklog_json("theirs", "acc-2", &started_at(2026, 9, 16, 10), 7200)
        ],
        "total": 2
    })]);
    let out = gateway(&base).my_worklogs(&key("OPS-7"), "acc-1", None).expect("fetched");
    let ids: Vec<&str> = out.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, vec!["mine"], "a shared ticket carries other people's time");
}

/// R17: the tasks tab shows due dates and staleness, so every search must ask
/// Jira for those fields — without regressing the ones the week view needs.
#[test]
fn search_requests_due_and_updated_fields() {
    let (base, seen) = fake_jira(vec![json!({ "issues": [], "isLast": true })]);
    gateway(&base).search_issues("assignee = currentUser()", 50).expect("searched");

    let fields = only(&seen).query().get("fields").cloned().expect("a fields parameter");
    for want in ["summary", "status", "duedate", "updated"] {
        assert!(fields.contains(want), "fields {fields:?} must request {want}");
    }
}

#[test]
fn the_embedded_search_page_keeps_only_my_worklogs() {
    let (base, _seen) = fake_jira(vec![json!({
        "issues": [{
            "key": "A-1",
            "fields": {
                "summary": "Fix auth", "status": { "name": "In Progress" },
                "worklog": {
                    "total": 2,
                    "worklogs": [
                        worklog_json("mine", "acc-1", &started_at(2026, 9, 16, 9), 3600),
                        worklog_json("theirs", "acc-2", &started_at(2026, 9, 16, 10), 7200)
                    ]
                }
            }
        }],
        "isLast": true
    })]);
    let out = gateway(&base).search_issues_with_worklogs("assignee = currentUser()", 50, "acc-1").expect("searched");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].total, 2, "the total counts everyone, so the page can be known incomplete");
    let ids: Vec<&str> = out[0].my_worklogs.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, vec!["mine"], "someone else logging on my ticket must not fill my day");
}
