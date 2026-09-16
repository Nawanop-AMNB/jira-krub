use super::search::{MAX_RESULTS, run};
use crate::application::ports::GatewayErrorKind;
use crate::application::test_support::{FakeGateway, Script, gateway_error, issue};

#[test]
fn query_shorter_than_two_chars_returns_empty_without_calling_the_gateway() {
    let gw = FakeGateway::new(Script { searches: [Ok(vec![issue("A-1")])].into(), ..Default::default() });
    assert!(run(&gw, " k ").unwrap().is_empty());
    assert!(gw.calls().jql.is_empty());
}

#[test]
fn key_shaped_query_falls_back_to_text_search_when_primary_jql_fails() {
    let gw = FakeGateway::new(Script {
        searches: [
            Err(gateway_error(GatewayErrorKind::Other, "search: HTTP 400 no project KAN")),
            Ok(vec![issue("X-1")]),
        ]
        .into(),
        ..Default::default()
    });
    let out = run(&gw, "kan-1").unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].key.as_str(), "X-1");
    assert_eq!(
        *gw.calls().jql,
        vec![
            "(key = KAN-1 OR project = KAN) ORDER BY updated DESC".to_string(),
            "text ~ \"kan-1*\" ORDER BY updated DESC".to_string(),
        ]
    );
}

#[test]
fn text_query_error_propagates() {
    let gw = FakeGateway::new(Script {
        searches: [Err(gateway_error(GatewayErrorKind::Network, "search: offline"))].into(),
        ..Default::default()
    });
    let err = run(&gw, "sso outage").unwrap_err();
    assert!(err.to_string().contains("offline"));
    assert_eq!(gw.calls().jql.len(), 1);
}

#[test]
fn results_are_ranked_exact_then_prefix_then_rest_and_truncated() {
    let server_order = ["ZZ-1", "KAN-10", "KAN-12", "KAN-1", "KAN-2", "KAN-3", "KAN-4"];
    let gw = FakeGateway::new(Script {
        searches: [Ok(server_order.iter().map(|k| issue(k)).collect())].into(),
        ..Default::default()
    });
    let out = run(&gw, "kan-1").unwrap();
    let keys: Vec<&str> = out.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(keys.len(), MAX_RESULTS);
    assert_eq!(keys, vec!["KAN-1", "KAN-10", "KAN-12", "ZZ-1", "KAN-2"]);
}
