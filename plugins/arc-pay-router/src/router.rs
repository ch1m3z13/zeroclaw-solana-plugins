// plugins/arc-pay-router/src/router.rs
// Pure routing logic — no wasm deps. Tests run on the host with MockTransport.

pub use zeroclaw_solana_core::arc::ArcClient;
use zeroclaw_solana_core::arc::DEFAULT_ARC_URL;
pub use zeroclaw_solana_core::types::RouteResult;
pub use zeroclaw_solana_core::types::{ChainId, Optimize, RouteQuote, RouteRequest};

/// Build and return the best eligible route for `req`.
///
/// Fail-closed: any Arc or risk failure is surfaced as `RouteResult::Error`.
/// Only the top-ranked route is returned (T1 — propose-only).
pub fn build_route(client: &ArcClient, req: &RouteRequest) -> RouteResult {
    client.build_route(req)
}

/// Convenience: quote routes for a pair without a full `RouteRequest`.
pub fn quote(
    client: &ArcClient,
    source: &str,
    dest: &str,
    input_mint: &str,
    output_mint: &str,
    amount: u64,
    optimize: Optimize,
) -> RouteResult {
    let q = RouteQuote {
        source_chain: source.into(),
        dest_chain: dest.into(),
        input_mint: input_mint.into(),
        output_mint: output_mint.into(),
        amount,
        optimize,
        affiliate: None,
    };
    client.quote_route(&q)
}

/// Convenience: fetch unified balances across the EVM and Solana families.
pub fn balances(client: &ArcClient, address: &str) -> RouteResult {
    let chains = vec![
        ChainId::Solana,
        ChainId::Ethereum,
        ChainId::Base,
        ChainId::Arbitrum,
        ChainId::Optimism,
        ChainId::Polygon,
    ];
    client.unified_balance(address, &chains)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_solana_core::rpc::MockTransport;

    fn mock_client() -> ArcClient {
        let handler = move |_url: &str, _key: Option<&str>, req: &[u8]| {
            let parsed: serde_json::Value = serde_json::from_slice(req).unwrap();
            let method = parsed["method"].as_str().unwrap_or("");
            if method == "arc_quoteRoute" {
                Ok(serde_json::to_vec(&serde_json::json!({
                    "jsonrpc": "2.0", "id": 1,
                    "result": {
                        "routes": [{
                            "route_id": "R-1",
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
                }))
                .unwrap())
            }
        };
        ArcClient::with_transport(
            String::from("http://mock"),
            Box::new(MockTransport {
                handler: Box::new(handler),
            }),
        )
    }

    #[test]
    fn quote_returns_routes() {
        let client = mock_client();
        let req = RouteRequest {
            from_address: "A".into(),
            to_address: "B".into(),
            quote: RouteQuote {
                source_chain: "solana".into(),
                dest_chain: "ethereum".into(),
                input_mint: "M".into(),
                output_mint: "N".into(),
                amount: 1,
                optimize: Optimize::Fee,
                affiliate: None,
            },
            propose_only: true,
            solana_risk_hint: Some("green:".into()),
        };
        match client.build_route(&req) {
            RouteResult::Ok { routes } => assert_eq!(routes.len(), 1),
            other => panic!("expected Ok, got {other:?}"),
        }
    }
}
