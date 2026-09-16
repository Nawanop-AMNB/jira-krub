use super::sync::{SyncInput, run};
use crate::application::ports::Window;
use crate::application::test_support::{FakeGateway, Script, issue, key, me, remote};
use crate::domain::IssueKey;
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn window() -> Window {
    Window { from: d(2026, 9, 14), to: d(2026, 9, 20) }
}

fn input(watchlist: &[&str]) -> SyncInput {
    SyncInput { jql: "assignee = currentUser()".into(), watchlist: watchlist.iter().map(|k| key(k)).collect(), window: window() }
}

fn keys(issues: &[crate::domain::Issue]) -> Vec<&str> {
    issues.iter().map(|i| i.key.as_str()).collect()
}

#[test]
fn mine_comes_from_the_config_jql() {
    let gw = FakeGateway::new(Script { searches: [Ok(vec![issue("A-1")])].into(), ..Default::default() });
    let out = run(&gw, &input(&[])).unwrap();
    assert_eq!(keys(&out.mine), vec!["A-1"]);
    assert_eq!(gw.calls().jql[0], "assignee = currentUser()");
}

#[test]
fn watchlist_is_fetched_per_key_and_missing_ones_are_skipped() {
    let gw = FakeGateway::new(Script { issues: [("OPS-7".to_string(), issue("OPS-7"))].into(), ..Default::default() });
    let out = run(&gw, &input(&["OPS-7", "GONE-1"])).unwrap();
    assert_eq!(keys(&out.watched), vec!["OPS-7"]);
    assert_eq!(gw.calls().get_issue, vec![key("OPS-7"), key("GONE-1")]);
}

#[test]
fn history_excludes_issues_already_mine_or_watched() {
    let gw = FakeGateway::new(Script {
        searches: [Ok(vec![issue("A-1")]), Ok(vec![issue("A-1"), issue("OPS-7"), issue("H-1")])].into(),
        issues: [("OPS-7".to_string(), issue("OPS-7"))].into(),
        ..Default::default()
    });
    let out = run(&gw, &input(&["OPS-7"])).unwrap();
    assert_eq!(keys(&out.history), vec!["H-1"]);
}

#[test]
fn history_jql_uses_worklog_author_and_the_window() {
    let gw = FakeGateway::default();
    run(&gw, &input(&[])).unwrap();
    let jql = &gw.calls().jql[1];
    assert!(jql.contains("worklogAuthor = currentUser()"), "{jql}");
    assert!(jql.contains("worklogDate >= 2026-09-14"), "{jql}");
    assert!(jql.contains("worklogDate <= 2026-09-20"), "{jql}");
}

#[test]
fn mine_uses_embedded_worklogs_and_watchlist_is_fetched_per_issue_with_window() {
    let gw = FakeGateway::new(Script {
        me: Some(me("acc-42", "Me")),
        searches: [Ok(vec![issue("A-1")]), Ok(vec![])].into(),
        issues: [("OPS-7".to_string(), issue("OPS-7"))].into(),
        embedded: [("A-1".to_string(), (vec![remote("w1", "A-1", (2026, 9, 16), 3600)], 1))].into(),
        worklogs: [("OPS-7".to_string(), vec![remote("w9", "OPS-7", (2026, 9, 15), 900)])].into(),
        ..Default::default()
    });
    let out = run(&gw, &input(&["OPS-7"])).unwrap();
    assert_eq!(out.account_id, "acc-42");
    let mut ids: Vec<&str> = out.worklogs.iter().map(|w| w.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["w1", "w9"]);
    // only the watched issue needed a per-issue call, and it carried the window
    let expected: Vec<(IssueKey, String, Option<Window>)> = vec![(key("OPS-7"), "acc-42".to_string(), Some(window()))];
    assert_eq!(gw.calls().my_worklogs, expected);
    assert_eq!(out.window, window());
}

#[test]
fn embedded_worklogs_outside_the_window_are_dropped() {
    let gw = FakeGateway::new(Script {
        searches: [Ok(vec![issue("A-1")]), Ok(vec![])].into(),
        embedded: [(
            "A-1".to_string(),
            (vec![remote("old", "A-1", (2025, 1, 5), 3600), remote("in", "A-1", (2026, 9, 17), 3600)], 2),
        )]
        .into(),
        ..Default::default()
    });
    let out = run(&gw, &input(&[])).unwrap();
    let ids: Vec<&str> = out.worklogs.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, vec!["in"]);
}

#[test]
fn issue_with_more_worklogs_than_jira_embeds_is_refetched_with_window() {
    // Jira embeds at most 20; total 25 means the page is incomplete.
    let gw = FakeGateway::new(Script {
        searches: [Ok(vec![issue("BIG-1")]), Ok(vec![])].into(),
        embedded: [("BIG-1".to_string(), (vec![remote("stale", "BIG-1", (2026, 9, 16), 60)], 25))].into(),
        worklogs: [("BIG-1".to_string(), vec![remote("fresh", "BIG-1", (2026, 9, 16), 3600)])].into(),
        ..Default::default()
    });
    let out = run(&gw, &input(&[])).unwrap();
    let ids: Vec<&str> = out.worklogs.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, vec!["fresh"], "embedded page ignored when incomplete");
    assert_eq!(gw.calls().my_worklogs, vec![(key("BIG-1"), "acc-1".to_string(), Some(window()))]);
}
