use anyhow::{Result, bail};
use std::fmt;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub key: IssueKey,
    pub summary: String,
    pub status: String,
}

impl Issue {
    /// Case-insensitive substring match on key or summary.
    pub fn matches(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let n = needle.to_lowercase();
        self.key.as_str().to_lowercase().contains(&n) || self.summary.to_lowercase().contains(&n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
