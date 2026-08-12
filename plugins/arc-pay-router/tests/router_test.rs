// plugins/arc-pay-router/tests/router_test.rs
// Host-run tests for arc-pay-router: ArcClient over MockTransport.

use arc_pay_router::router::{quote, ArcClient};
use zeroclaw_solana_core::arc::DEFAULT_ARC_URL;
use zeroclaw_solana_core::rpc::MockTransport;
use zeroclaw_solana_core::types::Optimize;

fn mock_client() -> ArcClient {
    let handler = move |_url: &str, _key: Option<&str>, req: &[u8]| {
        let parsed: serde_json::Value = serde_json::from_slice(req).unwrap();
        let method = parsed["method"].as_str().unwrap_or("");
        if method == "arc_quoteRoute" {
            Ok(serde_json::to_vec(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "result": {
                    "routes": [{
                        "route_id": "ARC-TEST-1",
                        "score": 1.0,
                        "steps": [{"type": "native_transfer", "from_chain": "solana", "to_chain": "solana", "token": "SOL", "amount": "1", "fee": "0", "note": "native"}],
                        "total_fee": "0",
                        "eta_secs": 1
                    }]
                }
            })).unwrap())
        } else {
            Ok(serde_json::to_vec(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": {"balances": []}
            })).unwrap())
        }
    };
    ArcClient::with_transport(
        String::from(DEFAULT_ARC_URL),
        Box::new(MockTransport {
            handler: Box::new(handler),
        }),
    )
}

#[test]
fn quote_returns_route() {
    let client = mock_client();
    let result = quote(
        &client,
        "solana",
        "ethereum",
        "SOL1111111111111111111111111111111111111",
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        100_000_000,
        Optimize::Fee,
    );
    match result {
        arc_pay_router::router::RouteResult::Ok { routes } => {
            assert_eq!(routes.len(), 1);
            assert_eq!(routes[0].route_id, "ARC-TEST-1");
            assert_eq!(routes[0].steps.len(), 1);
            assert_eq!(routes[0].steps[0].token, "SOL");
        }
        other => panic!("expected Ok, got {:?}", other),
    }
}

#[test]
fn transport_error_returns_error() {
    let bad = |_url: &str, _key: Option<&str>, _req: &[u8]| Err("down".into());
    let client = ArcClient::with_transport(
        String::from(DEFAULT_ARC_URL),
        Box::new(MockTransport {
            handler: Box::new(bad),
        }),
    );
    let result = quote(
        &client,
        "solana",
        "ethereum",
        "X",
        "Y",
        1,
        Optimize::Fee,
    );
    match result {
        arc_pay_router::router::RouteResult::Error { message: _ } => {}
        other => panic!("expected Error, got {:?}", other),
    }
}

#[test]
fn empty_routes_returns_no_route() {
    let handler = move |_url: &str, _key: Option<&str>, _req: &[u8]| {
        Ok(serde_json::to_vec(&serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": {"routes": []}
        })).unwrap())
    };
    let client = ArcClient::with_transport(
        String::from(DEFAULT_ARC_URL),
        Box::new(MockTransport {
            handler: Box::new(handler),
        }),
    );
    let result = quote(
        &client,
        "solana",
        "ethereum",
        "X",
        "Y",
        1,
        Optimize::Fee,
    );
    match result {
        arc_pay_router::router::RouteResult::NoRoute { .. } => {}
        other => panic!("expected NoRoute, got {:?}", other),
    }
}
