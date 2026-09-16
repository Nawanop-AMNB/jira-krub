use super::sync::{SyncInput, run};
use crate::application::test_support::{FakeGateway, Script, issue, key, me, remote};
use crate::domain::IssueKey;
use chrono::NaiveDate;

fn input(watchlist: &[&str]) -> SyncInput {
    SyncInput {
        jql: "assignee = currentUser()".into(),
        watchlist: watchlist.iter().map(|k| key(k)).collect(),
        history_from: NaiveDate::from_ymd_opt(2026, 9, 14).unwrap(),
    }
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
fn history_jql_uses_worklog_author_and_the_from_date() {
    let gw = FakeGateway::default();
    run(&gw, &input(&[])).unwrap();
    let jql = &gw.calls().jql[1];
    assert!(jql.contains("worklogAuthor = currentUser()"), "{jql}");
    assert!(jql.contains("worklogDate >= 2026-09-14"), "{jql}");
}

#[test]
fn worklogs_are_fetched_for_mine_watched_and_history_with_my_account_id() {
    let gw = FakeGateway::new(Script {
        me: Some(me("acc-42", "Me")),
        searches: [Ok(vec![issue("A-1")]), Ok(vec![issue("H-1")])].into(),
        issues: [("OPS-7".to_string(), issue("OPS-7"))].into(),
        worklogs: [
            ("A-1".to_string(), vec![remote("w1", "A-1", (2026, 9, 16), 3600)]),
            ("H-1".to_string(), vec![remote("w2", "H-1", (2026, 9, 15), 900)]),
        ]
        .into(),
        ..Default::default()
    });
    let out = run(&gw, &input(&["OPS-7"])).unwrap();
    assert_eq!(out.account_id, "acc-42");
    assert_eq!(out.display_name, "Me");
    let ids: Vec<&str> = out.worklogs.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, vec!["w1", "w2"]);
    let expected: Vec<(IssueKey, String)> =
        ["A-1", "OPS-7", "H-1"].iter().map(|k| (key(k), "acc-42".to_string())).collect();
    assert_eq!(gw.calls().my_worklogs, expected);
}
