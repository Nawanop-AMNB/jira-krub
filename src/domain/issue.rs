use super::week::Week;
use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate};
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
        f.write_str(&self.0)
    }
}

/// Jira's coarse status bucket, stable across per-project status names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCategory {
    New,
    Indeterminate,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub key: IssueKey,
    pub summary: String,
    pub status: String,
    pub status_category: StatusCategory,
    pub due: Option<NaiveDate>,
    pub updated: NaiveDate,
}

impl Issue {
    /// For call sites that only know key/summary/status: `New` category, no due
    /// date, `updated` at the epoch so it sorts last.
    pub fn new(key: IssueKey, summary: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            key,
            summary: summary.into(),
            status: status.into(),
            status_category: StatusCategory::New,
            due: None,
            updated: EPOCH,
        }
    }

    /// Case-insensitive substring match on key or summary.
    pub fn matches(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let n = needle.to_lowercase();
        self.key.as_str().to_lowercase().contains(&n) || self.summary.to_lowercase().contains(&n)
    }
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
