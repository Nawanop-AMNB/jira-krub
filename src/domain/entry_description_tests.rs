//! R3: an entry's description is multi-line and each non-empty line becomes
//! one Jira comment paragraph.

use super::{Entry, EntryId, EntryState, IssueKey, StartTime};
use chrono::NaiveDate;

fn described(title: &str) -> Entry {
    Entry {
        id: EntryId::new("e1".into()),
        issue_key: IssueKey::parse("KAN-1").unwrap(),
        date: NaiveDate::from_ymd_opt(2026, 9, 16).unwrap(),
        start: StartTime::NINE,
        seconds: 3600,
        title: title.to_string(),
        state: EntryState::Staged,
    }
}

#[test]
fn comment_paragraphs_splits_on_newlines_and_keeps_the_order() {
    let e = described("first\nsecond\nthird");
    assert_eq!(e.comment_paragraphs(), vec!["first", "second", "third"]);
}

#[test]
fn comment_paragraphs_trims_each_line() {
    let e = described("  padded  \n\ttabbed\t");
    assert_eq!(e.comment_paragraphs(), vec!["padded", "tabbed"]);
}

#[test]
fn comment_paragraphs_drops_blank_lines() {
    let e = described("first\n\n   \nsecond");
    assert_eq!(e.comment_paragraphs(), vec!["first", "second"]);
}

#[test]
fn a_description_of_only_blank_lines_yields_no_paragraphs() {
    assert!(described("\n  \n\t\n").comment_paragraphs().is_empty());
    assert!(described("").comment_paragraphs().is_empty());
}

#[test]
fn has_title_is_false_for_a_whitespace_only_description() {
    assert!(!described("   \n\t ").has_title());
    assert!(!described("").has_title());
}

#[test]
fn has_title_is_true_once_a_line_has_text() {
    assert!(described("  fix expiry  ").has_title());
}
