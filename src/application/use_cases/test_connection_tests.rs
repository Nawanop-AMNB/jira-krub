use super::test_connection::{Outcome, run};
use crate::application::ports::GatewayErrorKind;
use crate::application::test_support::{FakeGateway, Script, gateway_error, me};

fn failing(kind: GatewayErrorKind, msg: &str) -> FakeGateway {
    FakeGateway::new(Script { myself_error: Some(gateway_error(kind, msg)), ..Default::default() })
}

#[test]
fn successful_myself_yields_ok_with_display_name() {
    let gw = FakeGateway::new(Script { me: Some(me("acc-1", "Nawanop K.")), ..Default::default() });
    assert!(matches!(run(&gw), Outcome::Ok(m) if m.display_name == "Nawanop K." && m.account_id == "acc-1"));
    assert_eq!(gw.calls().myself, 1);
}

#[test]
fn unauthorized_maps_to_unauthorized() {
    assert!(matches!(run(&failing(GatewayErrorKind::Unauthorized, "myself: HTTP 401")), Outcome::Unauthorized));
}

#[test]
fn not_found_maps_to_not_jira() {
    assert!(matches!(run(&failing(GatewayErrorKind::NotFound, "myself: HTTP 404")), Outcome::NotJira));
}

#[test]
fn network_maps_to_network_with_message() {
    assert!(matches!(run(&failing(GatewayErrorKind::Network, "myself: dns")), Outcome::Network(m) if m == "myself: dns"));
}

#[test]
fn other_maps_to_other_with_message() {
    assert!(matches!(run(&failing(GatewayErrorKind::Other, "myself: HTTP 500")), Outcome::Other(m) if m == "myself: HTTP 500"));
}
