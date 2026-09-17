use crate::application::ports::JiraGateway;
use crate::domain::Issue;
use anyhow::Result;

pub const MIN_QUERY_CHARS: usize = 2;
pub const MAX_RESULTS: usize = 5;
/// Fetch a few more than shown so client-side ranking has something to rank.
const FETCH: usize = 20;

/// What the typed query looks like.
#[derive(Debug, PartialEq, Eq)]
enum Shape {
    /// `KAN` — a project key prefix
    Project(String),
    /// `KAN-1` — a full or partial issue key
    Key { project: String, key: String },
    /// anything else — free text
    Text,
}

fn classify(q: &str) -> Shape {
    let up = q.trim().to_uppercase();
    let is_proj = |s: &str| {
        !s.is_empty()
            && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    match up.split_once('-') {
        Some((p, n)) if is_proj(p) && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => {
            Shape::Key { project: p.to_string(), key: up.clone() }
        }
        None if is_proj(&up) => Shape::Project(up),
        _ => Shape::Text,
    }
}

fn escape(q: &str) -> String {
    q.replace('\\', "\\\\").replace('"', "\\\"")
}

fn text_jql(q: &str) -> String {
    format!("text ~ \"{}*\" ORDER BY updated DESC", escape(q.trim()))
}

/// Primary JQL: key-shaped input also lists the project so `KAN` finds
/// `KAN-1`, `KAN-2`, … (Jira's text search never matches keys).
pub fn jql_for(query: &str) -> String {
    match classify(query) {
        Shape::Project(p) => format!("(project = {p} OR text ~ \"{}*\") ORDER BY updated DESC", escape(query.trim())),
        Shape::Key { project, key } => format!("(key = {key} OR project = {project}) ORDER BY updated DESC"),
        Shape::Text => text_jql(query),
    }
}

/// Rank: exact key, then key starting with the query, then the rest (server order).
fn rank(mut issues: Vec<Issue>, query: &str) -> Vec<Issue> {
    let up = query.trim().to_uppercase();
    let score = |i: &Issue| -> u8 {
        let k = i.key.as_str();
        if k == up {
            0
        } else if k.starts_with(&up) {
            1
        } else {
            2
        }
    };
    issues.sort_by_key(|i| score(i));
    issues.truncate(MAX_RESULTS);
    issues
}

pub fn run(gateway: &dyn JiraGateway, query: &str) -> Result<Vec<Issue>> {
    if query.trim().chars().count() < MIN_QUERY_CHARS {
        return Ok(Vec::new());
    }
    let primary = jql_for(query);
    let issues = match gateway.search_issues(&primary, FETCH) {
        Ok(v) => v,
        // `project = XYZ` on a non-existent project is a JQL error (400);
        // fall back to plain text search rather than failing the whole query.
        Err(_) if classify(query) != Shape::Text => gateway.search_issues(&text_jql(query), FETCH)?,
        Err(e) => return Err(e),
    };
    Ok(rank(issues, query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::IssueKey;

    #[test]
    fn classifies() {
        assert_eq!(classify("kan"), Shape::Project("KAN".into()));
        assert_eq!(classify("KAN-1"), Shape::Key { project: "KAN".into(), key: "KAN-1".into() });
        assert_eq!(classify("sso outage"), Shape::Text);
        assert_eq!(classify("kan-"), Shape::Text);
        assert_eq!(classify("1kan"), Shape::Text);
    }

    #[test]
    fn builds_jql() {
        assert_eq!(jql_for("kan"), "(project = KAN OR text ~ \"kan*\") ORDER BY updated DESC");
        assert_eq!(jql_for("stw-123"), "(key = STW-123 OR project = STW) ORDER BY updated DESC");
        assert_eq!(jql_for("supp"), "(project = SUPP OR text ~ \"supp*\") ORDER BY updated DESC");
        assert_eq!(jql_for("a \"b\""), "text ~ \"a \\\"b\\\"*\" ORDER BY updated DESC");
    }

    #[test]
    fn ranks_exact_then_prefix() {
        let mk = |k: &str| Issue::new(IssueKey::parse(k).unwrap(), "", "");
        let out = rank(vec![mk("KAN-10"), mk("KAN-12"), mk("KAN-1"), mk("KAN-2"), mk("KAN-3"), mk("KAN-4"), mk("KAN-5")], "kan-1");
        let keys: Vec<&str> = out.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, vec!["KAN-1", "KAN-10", "KAN-12", "KAN-2", "KAN-3"]);
    }
}
