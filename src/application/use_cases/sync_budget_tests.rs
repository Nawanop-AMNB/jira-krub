//! R7 request budget and author filtering: the embedded worklog page is used
//! whenever it is complete, and every search says whose worklogs it wants.

use super::sync::{SyncInput, run};
use crate::application::ports::{EMBED_LIMIT, Window};
use crate::application::test_support::{FakeGateway, Script, issue, key, me, remote};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn window() -> Window {
    Window { from: d(2026, 9, 14), to: d(2026, 9, 20) }
}

fn input() -> SyncInput {
    SyncInput { jql: "assignee = currentUser()".into(), watchlist: Vec::new(), window: window() }
}

/// One issue whose search result embeds `total` worklogs, plus a per-issue
/// answer the sync would only see if it decided to re-fetch.
fn gateway_with_total(total: u64) -> FakeGateway {
    FakeGateway::new(Script {
        me: Some(me("acc-42", "Me")),
        searches: [Ok(vec![issue("A-1")]), Ok(vec![])].into(),
        embedded: [("A-1".to_string(), (vec![remote("embedded", "A-1", (2026, 9, 16), 3600)], total))].into(),
        worklogs: [("A-1".to_string(), vec![remote("refetched", "A-1", (2026, 9, 16), 3600)])].into(),
        ..Default::default()
    })
}

#[test]
fn an_issue_with_exactly_the_embedded_limit_of_worklogs_is_not_refetched() {
    // `total == EMBED_LIMIT` means Jira sent the whole list; a per-issue call
    // would be a wasted request on every sync.
    let gw = gateway_with_total(EMBED_LIMIT);
    let out = run(&gw, &input()).unwrap();
    let ids: Vec<&str> = out.worklogs.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, vec!["embedded"]);
    assert!(gw.calls().my_worklogs.is_empty(), "the embedded page was complete: {:?}", gw.calls().my_worklogs);
}

#[test]
fn an_issue_one_worklog_over_the_embedded_limit_is_refetched() {
    let gw = gateway_with_total(EMBED_LIMIT + 1);
    let out = run(&gw, &input()).unwrap();
    let ids: Vec<&str> = out.worklogs.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, vec!["refetched"], "an incomplete page must not be trusted");
    assert_eq!(gw.calls().my_worklogs.len(), 1);
}

#[test]
fn every_worklog_bearing_search_asks_for_my_account_id_only() {
    let gw = FakeGateway::new(Script {
        me: Some(me("acc-42", "Me")),
        searches: [Ok(vec![issue("A-1")]), Ok(vec![])].into(),
        ..Default::default()
    });
    run(&gw, &input()).unwrap();
    let ids = gw.calls().search_account_ids.clone();
    assert_eq!(ids, vec!["acc-42".to_string(), "acc-42".to_string()], "mine and history both filter to me");
}

#[test]
fn a_watchlist_fetch_asks_for_my_account_id_only() {
    let gw = FakeGateway::new(Script {
        me: Some(me("acc-42", "Me")),
        issues: [("OPS-7".to_string(), issue("OPS-7"))].into(),
        ..Default::default()
    });
    let mut input = input();
    input.watchlist = vec![key("OPS-7")];
    run(&gw, &input).unwrap();
    let accounts: Vec<String> = gw.calls().my_worklogs.iter().map(|(_, a, _)| a.clone()).collect();
    assert_eq!(accounts, vec!["acc-42".to_string()], "a shared ticket must not bring in other people's time");
}
