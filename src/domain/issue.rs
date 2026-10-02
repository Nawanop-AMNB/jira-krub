use super::week::Week;
use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate};
use std::collections::HashMap;
use std::fmt;

/// Stand-in `updated` for issues whose real value is unknown, so they sort last.
pub const EPOCH: NaiveDate = match NaiveDate::from_ymd_opt(1970, 1, 1) {
    Some(d) => d,
    None => unreachable!(),
};

/// `PROJ-123`. Uppercased on construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IssueKey(String);

impl IssueKey {
    pub fn parse(input: &str) -> Result<Self> {
        let s = input.trim().to_uppercase();
        if !Self::looks_like(&s) {
            bail!("'{input}' is not an issue key like STW-123");
        }
        Ok(Self(s))
    }

    /// `ABC-1` shape: letters/digits, dash, digits.
    pub fn looks_like(s: &str) -> bool {
        let Some((proj, num)) = s.rsplit_once('-') else { return false };
        !proj.is_empty()
            && proj.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && proj.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !num.is_empty()
            && num.chars().all(|c| c.is_ascii_digit())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IssueKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `pad`, not `write_str`, so `{:<9}` column layouts actually pad.
        f.pad(&self.0)
    }
}

/// Jira's coarse status bucket, stable across per-project status names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCategory {
    New,
    Indeterminate,
    Done,
}

/// The parent of a sub-task: just enough to show it has context (R2/R4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentRef {
    pub key: IssueKey,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub key: IssueKey,
    pub summary: String,
    pub status: String,
    pub status_category: StatusCategory,
    pub due: Option<NaiveDate>,
    pub updated: NaiveDate,
    /// `Some` only when this issue is a real Jira sub-task (an issue whose
    /// `parent` happens to be an Epic is not one — `None` there too).
    pub parent: Option<ParentRef>,
    /// Open + done children, as returned by Jira. Empty when not requested
    /// or unknown — never used to mean "no sub-tasks exist".
    pub subtasks: Vec<Issue>,
}

impl Issue {
    /// For call sites that only know key/summary/status: `New` category, no due
    /// date, `updated` at the epoch so it sorts last, no parent, no sub-tasks.
    pub fn new(key: IssueKey, summary: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            key,
            summary: summary.into(),
            status: status.into(),
            status_category: StatusCategory::New,
            due: None,
            updated: EPOCH,
            parent: None,
            subtasks: Vec::new(),
        }
    }

    /// Case-insensitive substring match on key or summary — or, for a
    /// sub-task, its parent's key or summary, so a card that only says "Dev"
    /// or "QA" can still be found by what it is actually about.
    pub fn matches(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let n = needle.to_lowercase();
        self.key.as_str().to_lowercase().contains(&n)
            || self.summary.to_lowercase().contains(&n)
            || self.parent.as_ref().is_some_and(|p| p.key.as_str().to_lowercase().contains(&n) || p.summary.to_lowercase().contains(&n))
    }
}

/// One row of a nested ticket list: either a real issue or, when a sub-task's
/// parent is not itself in the list, a stand-in context row built from the
/// sub-task's own `parent` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Head {
    Issue(Issue),
    Context(ParentRef),
}

/// A group in a nested ticket list: one head row plus its sub-tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nested {
    pub head: Head,
    pub children: Vec<Issue>,
}

/// The numeric part of `PROJ-123`, for sorting children in human order:
/// `IssueKey`'s derived `Ord` is string order, so `KAN-15` sorts before
/// `KAN-9` there.
fn issue_number(key: &IssueKey) -> u64 {
    key.as_str().rsplit_once('-').and_then(|(_, n)| n.parse().ok()).unwrap_or(0)
}

/// Nests a flat, ordered issue list into display groups for the day view and
/// My Tasks (R2/R4): a sub-task goes under its parent's row when the parent
/// is also in `issues`, otherwise under one shared `Context` head per parent.
/// A group sits at the position of its head's or its first child's first
/// appearance in `issues`; children are sorted by issue number ascending.
/// Pure (no IO): callers decide what "the list" is (a whole screen, one
/// section, one search result page, …).
pub fn nest(issues: &[Issue]) -> Vec<Nested> {
    let by_key: HashMap<&str, &Issue> = issues.iter().map(|i| (i.key.as_str(), i)).collect();
    let mut groups: Vec<Nested> = Vec::new();
    let mut group_index: HashMap<&str, usize> = HashMap::new();

    for issue in issues {
        match &issue.parent {
            Some(parent) => {
                let gkey = parent.key.as_str();
                if let Some(&gi) = group_index.get(gkey) {
                    groups[gi].children.push(issue.clone());
                } else if let Some(&real_parent) = by_key.get(gkey) {
                    group_index.insert(gkey, groups.len());
                    groups.push(Nested { head: Head::Issue(real_parent.clone()), children: vec![issue.clone()] });
                } else {
                    group_index.insert(gkey, groups.len());
                    groups.push(Nested { head: Head::Context(parent.clone()), children: vec![issue.clone()] });
                }
            }
            None => {
                let gkey = issue.key.as_str();
                match group_index.get(gkey) {
                    // A context head created from an earlier child catches up
                    // with the real issue once it is reached.
                    Some(&gi) => groups[gi].head = Head::Issue(issue.clone()),
                    None => {
                        group_index.insert(gkey, groups.len());
                        groups.push(Nested { head: Head::Issue(issue.clone()), children: Vec::new() });
                    }
                }
            }
        }
    }
    for g in &mut groups {
        g.children.sort_by_key(|c| issue_number(&c.key));
    }
    groups
}

/// Due-date label for a row, or `None` when the issue has no due date.
/// Detail shrinks with distance: a weekday inside the current week, a bare day
/// and month within the year, the year only when it differs.
pub fn due_label(due: Option<NaiveDate>, today: NaiveDate, week: &Week) -> Option<String> {
    let due = due?;
    Some(if due < today {
        format!("overdue {}d", (today - due).num_days())
    } else if due == today {
        "due today".to_string()
    } else if week.contains(due) {
        format!("due {}", due.format("%a %d %b"))
    } else if due.year() == today.year() {
        format!("due {}", due.format("%d %b"))
    } else {
        format!("due {}", due.format("%d %b %Y"))
    })
}

/// How stale an issue is, at day granularity.
pub fn updated_label(updated: NaiveDate, today: NaiveDate) -> String {
    match (today - updated).num_days() {
        d if d <= 0 => "updated today".to_string(),
        d => format!("updated {d}d ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("valid test date")
    }

    #[test]
    fn due_label_cases() {
        let today = d(2026, 9, 17); // Thursday
        let week = Week::containing(today); // Mon 14 – Sun 20 Sep
        let l = |due: Option<NaiveDate>| due_label(due, today, &week);

        assert_eq!(l(None), None);
        assert_eq!(l(Some(d(2026, 9, 16))).as_deref(), Some("overdue 1d"));
        assert_eq!(l(Some(d(2026, 9, 17))).as_deref(), Some("due today"));
        assert_eq!(l(Some(d(2026, 9, 19))).as_deref(), Some("due Sat 19 Sep"));
        assert_eq!(l(Some(d(2026, 9, 30))).as_deref(), Some("due 30 Sep"));
        assert_eq!(l(Some(d(2027, 1, 5))).as_deref(), Some("due 05 Jan 2027"));
    }

    #[test]
    fn updated_label_cases() {
        let today = d(2026, 9, 17);
        assert_eq!(updated_label(d(2026, 9, 17), today), "updated today");
        assert_eq!(updated_label(d(2026, 9, 14), today), "updated 3d ago");
    }

    #[test]
    fn key_shapes() {
        assert!(IssueKey::looks_like("STW-123"));
        assert!(IssueKey::looks_like("A1-9"));
        assert!(!IssueKey::looks_like("stw123"));
        assert!(!IssueKey::looks_like("-1"));
        assert!(!IssueKey::looks_like("STW-"));
        assert!(!IssueKey::looks_like("1AB-2"));
        assert_eq!(IssueKey::parse(" stw-12 ").unwrap().as_str(), "STW-12");
    }
}

#[cfg(test)]
mod nest_tests {
    use super::*;

    fn key(s: &str) -> IssueKey {
        IssueKey::parse(s).unwrap()
    }

    fn issue(k: &str) -> Issue {
        Issue::new(key(k), format!("summary of {k}"), "To Do")
    }

    fn child(k: &str, parent_key: &str, parent_summary: &str) -> Issue {
        let mut i = Issue::new(key(k), k, "To Do");
        i.parent = Some(ParentRef { key: key(parent_key), summary: parent_summary.into() });
        i
    }

    fn head_key(n: &Nested) -> String {
        match &n.head {
            Head::Issue(i) => i.key.to_string(),
            Head::Context(p) => p.key.to_string(),
        }
    }

    fn children_keys(n: &Nested) -> Vec<String> {
        n.children.iter().map(|c| c.key.to_string()).collect()
    }

    #[test]
    fn nest_groups_child_under_parent_in_list() {
        let parent = issue("KAN-12");
        let out = nest(&[parent.clone(), child("KAN-15", "KAN-12", "Fix auth")]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].head, Head::Issue(parent));
        assert_eq!(children_keys(&out[0]), vec!["KAN-15"]);
    }

    #[test]
    fn nest_context_head_when_parent_absent() {
        let out = nest(&[child("KAN-15", "KAN-12", "Fix auth"), child("KAN-16", "KAN-12", "Fix auth")]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].head, Head::Context(ParentRef { key: key("KAN-12"), summary: "Fix auth".into() }));
        assert_eq!(children_keys(&out[0]), vec!["KAN-15", "KAN-16"]);
    }

    #[test]
    fn nest_sorts_children_by_number() {
        let out = nest(&[child("KAN-15", "KAN-12", "P"), child("KAN-9", "KAN-12", "P"), child("KAN-100", "KAN-12", "P")]);
        assert_eq!(out.len(), 1);
        assert_eq!(children_keys(&out[0]), vec!["KAN-9", "KAN-15", "KAN-100"]);
    }

    #[test]
    fn nest_keeps_order_of_first_appearance() {
        let out = nest(&[issue("A-1"), child("KAN-15", "KAN-12", "P"), issue("A-2"), issue("KAN-12")]);
        let heads: Vec<String> = out.iter().map(head_key).collect();
        assert_eq!(heads, vec!["A-1", "KAN-12", "A-2"], "KAN-12's group sits where KAN-15 first appeared");
        assert_eq!(children_keys(&out[1]), vec!["KAN-15"]);
        assert!(out[0].children.is_empty());
        assert!(out[2].children.is_empty());
    }

    #[test]
    fn matches_parent_summary_and_key() {
        let dev = child("KAN-15", "KAN-12", "Fix auth");
        assert!(dev.matches("auth"), "matches the parent's summary");
        assert!(dev.matches("kan-12"), "matches the parent's key, case-insensitively");
        assert!(!dev.matches("zzz"));
    }
}
