//! Regression test: an issue that is both mine and on the watchlist must not
//! double-count its worklogs in the week overview (see
//! `application::use_cases::sync::run_with`'s dedup).

use super::action::Action;
use super::test_support::{go_worklog, harness, render, settle};
use crate::application::test_support::{Script, issue, key, remote};
use crate::domain::{DaySummary, Week};
use chrono::{Datelike, Local};

#[test]
fn week_total_counts_a_worklog_once_when_its_issue_is_mine_and_watched() {
    let date = Week::containing(Local::now().date_naive()).monday();
    let raw = (date.year(), date.month(), date.day());

    // First sync (during startup) sees KAN-1 only as mine; the second, after
    // we watch it and refresh, also sees it via the watchlist leg.
    let mut h = harness(
        "week-mine-and-watched-dedup",
        Script {
            searches: [Ok(vec![issue("KAN-1")]), Ok(vec![]), Ok(vec![issue("KAN-1")]), Ok(vec![])].into(),
            issues: [("KAN-1".to_string(), issue("KAN-1"))].into(),
            embedded: [("KAN-1".to_string(), (vec![remote("w1", "KAN-1", raw, 7200)], 1))].into(),
            worklogs: [("KAN-1".to_string(), vec![remote("w1", "KAN-1", raw, 7200)])].into(),
            ..Script::default()
        },
    );

    h.app.ledger.watch(&key("KAN-1"));
    h.app.dispatch(Action::Refresh);
    settle(&mut h.app);

    let summary = DaySummary::compute(date, h.app.today, &h.app.calendar(), h.app.ledger.entries(), &h.app.remote.worklogs);
    assert_eq!(summary.pushed_seconds, 7200, "the shared worklog must be counted once, not twice");

    go_worklog(&mut h.app);
    let frame = render(&mut h.app).join("\n");
    assert!(frame.contains("2h"), "the rendered week row should show 2h for the day:\n{frame}");
    assert!(!frame.contains("4h"), "the rendered week row must not show the doubled 4h:\n{frame}");
}
