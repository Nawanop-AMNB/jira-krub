use crate::application::ports::{GatewayError, GatewayErrorKind, IssueWithWorklogs, JiraGateway, Me, NewWorklog, Window};
use crate::application::Credentials;
use crate::domain::{Issue, IssueKey, RemoteWorklog};
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Days, Local, TimeZone};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::{Value, json};
use std::time::Duration;

const STARTED_FMT: &str = "%Y-%m-%dT%H:%M:%S%.3f%z";

pub struct HttpJiraGateway {
    http: Client,
    base: String,
    email: String,
    token: String,
}

impl HttpJiraGateway {
    pub fn new(creds: &Credentials) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("jira-krub/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            http,
            base: creds.site.as_str().to_string(),
            email: creds.email.clone(),
            token: creds.api_token.clone(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/rest/api/3{}", self.base, path)
    }

    /// Paged `/search/jql`, returning the raw issue objects.
    fn search_raw(&self, jql: &str, max: usize, fields: &str) -> Result<Vec<Value>> {
        let mut out: Vec<Value> = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let page = (max - out.len()).clamp(1, 100);
            let mut q = vec![("jql", jql.to_string()), ("fields", fields.to_string()), ("maxResults", page.to_string())];
            if let Some(t) = &token {
                q.push(("nextPageToken", t.clone()));
            }
            let v = self.send(self.http.get(self.url("/search/jql")).query(&q), "search")?;
            out.extend(v["issues"].as_array().cloned().unwrap_or_default());
            match v["nextPageToken"].as_str() {
                Some(t) if v["isLast"].as_bool() != Some(true) && out.len() < max => token = Some(t.to_string()),
                _ => break,
            }
        }
        out.truncate(max);
        Ok(out)
    }

    fn send(&self, rb: RequestBuilder, what: &str) -> Result<Value> {
        let resp = rb
            .basic_auth(&self.email, Some(&self.token))
            .header("Accept", "application/json")
            .send()
            .map_err(|e| {
                let kind = if e.is_connect() || e.is_timeout() || e.is_request() {
                    GatewayErrorKind::Network
                } else {
                    GatewayErrorKind::Other
                };
                anyhow!(GatewayError { kind, message: format!("{what}: {e}") })
            })?;
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            let kind = match status.as_u16() {
                401 | 403 => GatewayErrorKind::Unauthorized,
                404 => GatewayErrorKind::NotFound,
                _ => GatewayErrorKind::Other,
            };
            let detail = extract_error_message(&text).unwrap_or_else(|| text.chars().take(200).collect());
            return Err(anyhow!(GatewayError { kind, message: format!("{what}: HTTP {} {}", status.as_u16(), detail) }));
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).with_context(|| format!("{what}: bad JSON"))
    }
}

/// `[from 00:00, to 24:00)` in local time as epoch milliseconds.
fn window_millis(w: &Window) -> (i64, i64) {
    let start = Local.from_local_datetime(&w.from.and_hms_opt(0, 0, 0).unwrap()).single();
    let end = Local.from_local_datetime(&(w.to + Days::new(1)).and_hms_opt(0, 0, 0).unwrap()).single();
    (
        start.map(|d| d.timestamp_millis()).unwrap_or(0),
        end.map(|d| d.timestamp_millis() - 1).unwrap_or(i64::MAX),
    )
}

fn extract_error_message(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    if let Some(arr) = v["errorMessages"].as_array() {
        let msgs: Vec<&str> = arr.iter().filter_map(|m| m.as_str()).collect();
        if !msgs.is_empty() {
            return Some(msgs.join("; "));
        }
    }
    if let Some(obj) = v["errors"].as_object() {
        let msgs: Vec<String> = obj.iter().map(|(k, m)| format!("{k}: {}", m.as_str().unwrap_or(""))).collect();
        if !msgs.is_empty() {
            return Some(msgs.join("; "));
        }
    }
    None
}

fn parse_issue(v: &Value) -> Option<Issue> {
    Some(Issue {
        key: IssueKey::parse(v["key"].as_str()?).ok()?,
        summary: v["fields"]["summary"].as_str().unwrap_or("").to_string(),
        status: v["fields"]["status"]["name"].as_str().unwrap_or("").to_string(),
    })
}

fn parse_worklog(key: &IssueKey, w: &Value) -> Option<RemoteWorklog> {
    let started = DateTime::parse_from_str(w["started"].as_str()?, STARTED_FMT).ok()?;
    Some(RemoteWorklog {
        id: w["id"].as_str().unwrap_or("").to_string(),
        issue_key: key.clone(),
        started,
        seconds: w["timeSpentSeconds"].as_u64().unwrap_or(0),
        comment: adf_text(&w["comment"]),
    })
}

/// Flatten Atlassian Document Format to plain text, one line per paragraph
/// (hard breaks inside a paragraph become lines too).
fn adf_text(v: &Value) -> String {
    fn walk(v: &Value, out: &mut String) {
        if v["type"] == "hardBreak" {
            out.push('\n');
        }
        if let Some(t) = v["text"].as_str() {
            out.push_str(t);
        }
        if let Some(arr) = v["content"].as_array() {
            for (i, c) in arr.iter().enumerate() {
                if i > 0 && c["type"] == "paragraph" {
                    out.push('\n');
                }
                walk(c, out);
            }
        }
    }
    let mut s = String::new();
    walk(v, &mut s);
    s.trim_end().to_string()
}

fn adf_doc(paragraphs: &[String]) -> Value {
    json!({
        "type": "doc", "version": 1,
        "content": paragraphs.iter().map(|p| json!({
            "type": "paragraph",
            "content": [{ "type": "text", "text": p }]
        })).collect::<Vec<_>>()
    })
}

impl JiraGateway for HttpJiraGateway {
    fn myself(&self) -> Result<Me> {
        let v = self.send(self.http.get(self.url("/myself")), "myself")?;
        Ok(Me {
            account_id: v["accountId"].as_str().context("myself: no accountId")?.to_string(),
            display_name: v["displayName"].as_str().unwrap_or("").to_string(),
        })
    }

    fn search_issues(&self, jql: &str, max: usize) -> Result<Vec<Issue>> {
        Ok(self.search_raw(jql, max, "summary,status")?.iter().filter_map(parse_issue).collect())
    }

    fn search_issues_with_worklogs(&self, jql: &str, max: usize, account_id: &str) -> Result<Vec<IssueWithWorklogs>> {
        let raw = self.search_raw(jql, max, "summary,status,worklog")?;
        Ok(raw
            .iter()
            .filter_map(|v| {
                let issue = parse_issue(v)?;
                let field = &v["fields"]["worklog"];
                let my_worklogs = field["worklogs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|w| w["author"]["accountId"].as_str() == Some(account_id))
                    .filter_map(|w| parse_worklog(&issue.key, w))
                    .collect();
                let total = field["total"].as_u64().unwrap_or(0);
                Some(IssueWithWorklogs { issue, my_worklogs, total })
            })
            .collect())
    }

    fn get_issue(&self, key: &IssueKey) -> Result<Issue> {
        let q = [("fields", "summary,status")];
        let v = self.send(self.http.get(self.url(&format!("/issue/{key}"))).query(&q), &format!("issue {key}"))?;
        parse_issue(&v).context("issue: unparseable")
    }

    fn my_worklogs(&self, key: &IssueKey, account_id: &str, window: Option<&Window>) -> Result<Vec<RemoteWorklog>> {
        let mut out = Vec::new();
        let mut start = 0u64;
        loop {
            let mut q = vec![("startAt", start.to_string()), ("maxResults", "1000".to_string())];
            if let Some(w) = window {
                // Jira filters on the worklog's `started` timestamp (epoch ms).
                let (after, before) = window_millis(w);
                q.push(("startedAfter", after.to_string()));
                q.push(("startedBefore", before.to_string()));
            }
            let v = self.send(
                self.http.get(self.url(&format!("/issue/{key}/worklog"))).query(&q),
                &format!("worklogs {key}"),
            )?;
            let list = v["worklogs"].as_array().cloned().unwrap_or_default();
            let n = list.len() as u64;
            out.extend(
                list.iter()
                    .filter(|w| w["author"]["accountId"].as_str() == Some(account_id))
                    .filter_map(|w| parse_worklog(key, w)),
            );
            let total = v["total"].as_u64().unwrap_or(0);
            start += n;
            if n == 0 || start >= total {
                break;
            }
        }
        Ok(out)
    }

    fn add_worklog(&self, req: &NewWorklog) -> Result<RemoteWorklog> {
        let mut body = json!({
            "timeSpentSeconds": req.seconds,
            "started": req.started.format(STARTED_FMT).to_string(),
        });
        if !req.comment_paragraphs.is_empty() {
            body["comment"] = adf_doc(&req.comment_paragraphs);
        }
        let v = self.send(
            self.http
                .post(self.url(&format!("/issue/{}/worklog", req.issue_key)))
                .query(&[("adjustEstimate", "leave")])
                .json(&body),
            &format!("add worklog {}", req.issue_key),
        )?;
        parse_worklog(&req.issue_key, &v).context("add worklog: unparseable response")
    }

    fn update_worklog(&self, worklog_id: &str, req: &NewWorklog) -> Result<RemoteWorklog> {
        let body = json!({
            "timeSpentSeconds": req.seconds,
            "started": req.started.format(STARTED_FMT).to_string(),
            // An explicit empty doc clears a previous comment.
            "comment": adf_doc(&req.comment_paragraphs),
        });
        let v = self.send(
            self.http
                .put(self.url(&format!("/issue/{}/worklog/{worklog_id}", req.issue_key)))
                .query(&[("adjustEstimate", "leave")])
                .json(&body),
            &format!("update worklog {} #{worklog_id}", req.issue_key),
        )?;
        parse_worklog(&req.issue_key, &v).context("update worklog: unparseable response")
    }

    fn delete_worklog(&self, issue_key: &IssueKey, worklog_id: &str) -> Result<()> {
        self.send(
            self.http
                .delete(self.url(&format!("/issue/{issue_key}/worklog/{worklog_id}")))
                .query(&[("adjustEstimate", "leave")]),
            &format!("delete worklog {issue_key} #{worklog_id}"),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> IssueKey {
        IssueKey::parse(s).unwrap()
    }

    #[test]
    fn adf_text_one_line_per_paragraph() {
        let doc = json!({
            "type": "doc", "version": 1,
            "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "fix expiry" }] },
                { "type": "paragraph", "content": [{ "type": "text", "text": "details" }] }
            ]
        });
        assert_eq!(adf_text(&doc), "fix expiry\ndetails");
        assert_eq!(adf_text(&Value::Null), "");
    }

    #[test]
    fn adf_doc_builds_one_paragraph_per_string() {
        let doc = adf_doc(&["a".to_string(), "b".to_string()]);
        assert_eq!(doc["type"], "doc");
        assert_eq!(doc["version"], 1);
        let paras = doc["content"].as_array().unwrap();
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0]["type"], "paragraph");
        assert_eq!(paras[0]["content"][0]["text"], "a");
        assert_eq!(paras[1]["content"][0]["text"], "b");
        assert!(adf_doc(&[])["content"].as_array().unwrap().is_empty());
    }

    #[test]
    fn extract_error_message_joins_error_messages() {
        let body = r#"{"errorMessages":["Issue does not exist","Nope"],"errors":{}}"#;
        assert_eq!(extract_error_message(body).as_deref(), Some("Issue does not exist; Nope"));
    }

    #[test]
    fn extract_error_message_formats_field_errors() {
        let body = r#"{"errorMessages":[],"errors":{"timeSpent":"Required"}}"#;
        assert_eq!(extract_error_message(body).as_deref(), Some("timeSpent: Required"));
        assert_eq!(extract_error_message("not json"), None);
        assert_eq!(extract_error_message(r#"{"errorMessages":[],"errors":{}}"#), None);
    }

    #[test]
    fn parse_worklog_handles_offset_and_missing_comment() {
        let w = json!({
            "id": "10042",
            "started": "2026-09-16T09:00:00.000+0700",
            "timeSpentSeconds": 3600
        });
        let parsed = parse_worklog(&key("A-1"), &w).unwrap();
        assert_eq!(parsed.id, "10042");
        assert_eq!(parsed.issue_key, key("A-1"));
        assert_eq!(parsed.seconds, 3600);
        assert_eq!(parsed.comment, "");
        assert_eq!(parsed.started.offset().local_minus_utc(), 7 * 3600);
        assert_eq!(parsed.started.to_rfc3339(), "2026-09-16T09:00:00+07:00");
        assert!(parse_worklog(&key("A-1"), &json!({ "id": "1" })).is_none());
    }

    #[test]
    fn parse_issue_returns_none_on_bad_key() {
        assert!(parse_issue(&json!({ "key": "not a key", "fields": {} })).is_none());
        assert!(parse_issue(&json!({ "fields": {} })).is_none());
        let ok = parse_issue(&json!({ "key": "kan-1", "fields": { "summary": "S", "status": { "name": "Done" } } })).unwrap();
        assert_eq!(ok.key, key("KAN-1"));
        assert_eq!(ok.summary, "S");
        assert_eq!(ok.status, "Done");
    }
}
