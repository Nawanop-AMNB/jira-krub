use super::sync::{SyncInput, SyncProgress, run, run_with};
use crate::application::ports::{GatewayErrorKind, Window};
use crate::application::test_support::{FakeGateway, Script, gateway_error, issue, key, me, remote};
use crate::domain::IssueKey;
use chrono::NaiveDate;
use std::time::{Duration, Instant};

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

// ---- streamed sync: mine reported early, watchlist/history concurrent ------

#[test]
fn mine_is_reported_before_any_watchlist_or_history_request() {
    let gw = FakeGateway::new(Script {
        searches: [Ok(vec![issue("A-1"), issue("A-2")])].into(),
        issues: [("W-1".to_string(), issue("W-1")), ("W-2".to_string(), issue("W-2"))].into(),
        ..Default::default()
    });

    let mut events: Vec<SyncProgress> = Vec::new();
    let mut snapshot: Option<(usize, Vec<String>, usize, usize)> = None;
    {
        let mut progress = |p: SyncProgress| {
            let calls = gw.calls();
            snapshot = Some((calls.myself, calls.jql.clone(), calls.get_issue.len(), calls.my_worklogs.len()));
            drop(calls);
            events.push(p);
        };
        run_with(&gw, &input(&["W-1", "W-2"]), &mut progress).unwrap();
    }

    assert_eq!(events.len(), 1, "exactly one Mine event");
    let SyncProgress::Mine { mine, .. } = &events[0];
    assert_eq!(keys(mine), vec!["A-1", "A-2"]);

    let (myself, jql, get_issue, my_worklogs) = snapshot.expect("progress fired");
    assert_eq!(myself, 1);
    assert_eq!(jql, vec!["assignee = currentUser()".to_string()], "only the mine search has happened yet");
    assert_eq!(get_issue, 0, "no watchlist request yet");
    assert_eq!(my_worklogs, 0, "no watchlist request yet");
}

#[test]
fn run_with_output_equals_run() {
    fn script() -> Script {
        Script {
            me: Some(me("acc-42", "Me")),
            searches: [Ok(vec![issue("A-1")]), Ok(vec![issue("H-1")])].into(),
            issues: [("OPS-7".to_string(), issue("OPS-7"))].into(),
            worklogs: [("OPS-7".to_string(), vec![remote("w9", "OPS-7", (2026, 9, 15), 900)])].into(),
            embedded: [("A-1".to_string(), (vec![remote("w1", "A-1", (2026, 9, 16), 3600)], 1))].into(),
            ..Default::default()
        }
    }
    let gw1 = FakeGateway::new(script());
    let gw2 = FakeGateway::new(script());

    let out1 = run(&gw1, &input(&["OPS-7"])).unwrap();
    let out2 = run_with(&gw2, &input(&["OPS-7"]), &mut |_| {}).unwrap();

    assert_eq!(out1.account_id, out2.account_id);
    assert_eq!(out1.display_name, out2.display_name);
    assert_eq!(keys(&out1.mine), keys(&out2.mine));
    assert_eq!(keys(&out1.watched), keys(&out2.watched));
    assert_eq!(keys(&out1.history), keys(&out2.history));
    let ids = |out: &crate::application::use_cases::sync::SyncOutput| out.worklogs.iter().map(|w| w.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&out1), ids(&out2));
    assert_eq!(out1.window, out2.window);
}

#[test]
fn watchlist_and_history_are_fetched_concurrently() {
    let gw = FakeGateway::new(Script {
        searches: [Ok(vec![issue("A-1")]), Ok(vec![])].into(),
        issues: [("W-1".to_string(), issue("W-1")), ("W-2".to_string(), issue("W-2")), ("W-3".to_string(), issue("W-3"))].into(),
        delay: Duration::from_millis(30),
        ..Default::default()
    });

    let mut concurrent_start: Option<Instant> = None;
    let mut progress = |_p: SyncProgress| concurrent_start = Some(Instant::now());
    run_with(&gw, &input(&["W-1", "W-2", "W-3"]), &mut progress).unwrap();

    let elapsed = concurrent_start.expect("progress fired").elapsed();
    assert!(gw.peak_in_flight() >= 2, "peak in flight was {}", gw.peak_in_flight());
    // sequential would be 3 keys * 2 calls * 30ms + 1 history search * 30ms = 210ms
    assert!(elapsed < Duration::from_millis(150), "took {elapsed:?}, expected clearly less than 210ms sequential");
}

#[test]
fn never_more_than_four_requests_in_flight() {
    let mut issues = std::collections::HashMap::new();
    let mut watchlist = Vec::new();
    for n in 1..=10 {
        let k = format!("W-{n}");
        issues.insert(k.clone(), issue(&k));
        watchlist.push(k);
    }
    let gw = FakeGateway::new(Script { issues, delay: Duration::from_millis(20), ..Default::default() });
    let watchlist_refs: Vec<&str> = watchlist.iter().map(|s| s.as_str()).collect();

    run_with(&gw, &input(&watchlist_refs), &mut |_| {}).unwrap();

    assert!(gw.peak_in_flight() <= 4, "peak in flight was {}", gw.peak_in_flight());
}

#[test]
fn missing_watchlist_issue_is_still_skipped_and_worklog_error_still_fails() {
    // A missing watchlist issue is skipped without failing the whole sync,
    // even now that watchlist keys are fetched concurrently.
    let gw = FakeGateway::new(Script { issues: [("OPS-7".to_string(), issue("OPS-7"))].into(), ..Default::default() });
    let out = run_with(&gw, &input(&["OPS-7", "GONE-1"]), &mut |_| {}).unwrap();
    assert_eq!(keys(&out.watched), vec!["OPS-7"]);

    // A worklog-fetch error on a watchlist key still fails the whole sync.
    let gw2 = FakeGateway::new(Script {
        issues: [("OPS-7".to_string(), issue("OPS-7"))].into(),
        worklogs_error: Some(gateway_error(GatewayErrorKind::Other, "boom")),
        ..Default::default()
    });
    let err = run_with(&gw2, &input(&["OPS-7"]), &mut |_| {}).unwrap_err();
    assert!(err.to_string().contains("boom"), "{err}");
}
