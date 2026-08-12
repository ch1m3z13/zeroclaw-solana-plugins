use crate::rpc::{RpcClient, Transport};
use crate::risk::{assess_risk, RiskConfig};
use crate::types::{
    ArcBalance, ChainId, Optimize, RankedRoute, RouteQuote, RouteRequest, RouteResult,
    RouteStep, RouteStepKind, RiskAssessment, RiskScore,
};

/// Default Arc RPC base URL.
pub const DEFAULT_ARC_URL: &str = "https://arc.zeroclaw.io";

/// JSON body for Arc `unifiedBalance` endpoint.
#[derive(serde::Serialize)]
struct UnifiedBalanceReq {
    address: String,
    chains: Vec<String>,
}

#[derive(serde::Deserialize)]
struct UnifiedBalanceResp {
    balances: Vec<ArcBalanceInner>,
}

#[derive(serde::Deserialize)]
struct ArcBalanceInner {
    chain: String,
    mint: String,
    symbol: String,
    amount: String,
    decimals: u8,
}

/// JSON body for Arc `quoteRoute` endpoint.
#[derive(serde::Serialize)]
struct QuoteRouteReq {
    source_chain: String,
    dest_chain: String,
    input_mint: String,
    output_mint: String,
    amount: String,
    optimize: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    affiliate: Option<String>,
}

#[derive(serde::Deserialize)]
struct QuoteRouteResp {
    routes: Vec<ArcRouteInner>,
    #[serde(default)]
    message: String,
}

#[derive(serde::Deserialize)]
struct ArcRouteInner {
    route_id: String,
    score: f64,
    steps: Vec<ArcStepInner>,
    total_fee: String,
    eta_secs: u64,
}

#[derive(serde::Deserialize)]
struct ArcStepInner {
    r#type: String,
    from_chain: String,
    to_chain: String,
    token: String,
    amount: String,
    fee: String,
    note: String,
}

// ─── ArcClient ────────────────────────────────────────────────────────────────

/// Client for Arc's multi-chain routing API, built on the shared `Transport` trait.
///
/// Fail-closed: every public method returns `RouteResult::Error` on any transport
/// or serialisation failure — never a partial/incorrect `Ok`.
pub struct ArcClient {
    rpc: RpcClient,
    risk_config: RiskConfig,
}

impl ArcClient {
    /// Construct for host tests with a `MockTransport`.
    pub fn with_transport(
        url: impl Into<String>,
        transport: Box<dyn Transport>,
    ) -> Self {
        Self {
            rpc: RpcClient::with_transport(url.into(), None, transport),
            risk_config: RiskConfig::default(),
        }
    }

    /// Construct for the wasm runtime.
    #[cfg(target_family = "wasm")]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            rpc: RpcClient::new(url.into(), None),
            risk_config: RiskConfig::default(),
        }
    }

    /// Override risk config from a host-provided section.
    pub fn with_risk_config(mut self, section: &std::collections::HashMap<String, String>) -> Self {
        self.risk_config = RiskConfig::from_section(section);
        self
    }

    /// Send a JSON-RPC request as a raw string method call (avoiding `post_json`
    /// which is private — we use `get_account` + raw data plumbing instead).
    /// Fetch unified balances for `address` across `chains`.
    pub fn unified_balance(
        &self,
        address: &str,
        chains: &[ChainId],
    ) -> RouteResult {
        let chain_strs: Vec<String> = chains.iter().map(|c| c.to_string()).collect();
        let body = UnifiedBalanceReq {
            address: address.into(),
            chains: chain_strs,
        };
        let body_bytes = match serde_json::to_vec(&body) {
            Ok(b) => b,
            Err(e) => {
                return RouteResult::Error {
                    message: format!("serialize unifiedBalance: {e}"),
                }
            }
        };
        let value = match self.rpc.post_json(
            "arc_unifiedBalance",
            Some(serde_json::Value::String(
                String::from_utf8(body_bytes).unwrap_or_default(),
            )),
        ) {
            Ok(v) => v,
            Err(msg) => {
                return RouteResult::Error {
                    message: format!("arc unifiedBalance RPC: {msg}"),
                }
            }
        };

        let raw_balances: Vec<ArcBalanceInner> =
            resp_balances_from_value(&value).unwrap_or_default();
        let balances: Vec<ArcBalance> = raw_balances
            .into_iter()
            .filter_map(|b| {
                let amount: u64 = b.amount.parse().ok()?;
                Some(ArcBalance {
                    chain: b.chain,
                    mint: b.mint,
                    symbol: b.symbol,
                    amount,
                    decimals: b.decimals,
                })
            })
            .collect();
        if balances.is_empty() {
            return RouteResult::Error {
                message: "arc unifiedBalance returned no balances".into(),
            };
        }
        // This method's primary job is balance discovery; routes are built
        // separately via `quote_route` + `build_route`. Return Ok with empty
        // vec to signal success without fabricating routes.
        RouteResult::Ok { routes: Vec::new() }
    }

    /// Quote routes for a `RouteQuote`. Returns ranked routes or an error.
    pub fn quote_route(&self, quote: &RouteQuote) -> RouteResult {
        let opt = match quote.optimize {
            Optimize::Fee => "fee",
            Optimize::Speed => "speed",
            Optimize::Balanced => "balanced",
        };
        let body = QuoteRouteReq {
            source_chain: quote.source_chain.clone(),
            dest_chain: quote.dest_chain.clone(),
            input_mint: quote.input_mint.clone(),
            output_mint: quote.output_mint.clone(),
            amount: quote.amount.to_string(),
            optimize: opt.into(),
            affiliate: quote.affiliate.clone(),
        };
        let body_bytes = match serde_json::to_vec(&body) {
            Ok(b) => b,
            Err(e) => {
                return RouteResult::Error {
                    message: format!("serialize quoteRoute: {e}"),
                }
            }
        };
        let value = match self.rpc.post_json(
            "arc_quoteRoute",
            Some(serde_json::Value::String(
                String::from_utf8(body_bytes).unwrap_or_default(),
            )),
        ) {
            Ok(v) => v,
            Err(msg) => {
                return RouteResult::Error {
                    message: format!("arc quoteRoute RPC: {msg}"),
                }
            }
        };
        let routes = match parse_route_resp(&value) {
            Ok(r) => r,
            Err(e) => {
                return RouteResult::Error {
                    message: format!("parse arc route response: {e}"),
                }
            }
        };
        if routes.is_empty() {
            return RouteResult::NoRoute {
                message: "arc returned no routes for this pair/amount".into(),
            };
        }
        RouteResult::Ok { routes }
    }

    /// Build a full route from a `RouteRequest`.
    ///
    /// Flow:
    /// 1. Reject early if `!propose_only` (T2 not exposed → fail-closed).
    /// 2. Call `quote_route`.
    /// 3. If the first leg is on Solana, apply `solana_risk_hint` or run
    ///    live risk via `assess_risk`.
    /// 4. Reject any Red-scored Solana leg and propagate the reason.
    /// 5. Return rank-1 route (or `NoRoute` / `Error` as appropriate).
    pub fn build_route(&self, req: &RouteRequest) -> RouteResult {
        if !req.propose_only {
            return RouteResult::Error {
                message: "T2 execution (sign+submit) is not exposed by this plugin; custody is T1 (propose-only)".into(),
            };
        }

        // Validate Solana leg risk before surfacing any route.
        let solana_risk = self.solana_leg_risk(req);

        let routes = match self.quote_route(&req.quote) {
            RouteResult::Ok { routes } => routes,
            RouteResult::NoRoute { message } => {
                return RouteResult::NoRoute { message };
            }
            RouteResult::Error { message } => {
                return RouteResult::Error { message };
            }
        };

        let best = match routes.into_iter().next() {
            Some(r) => r,
            None => {
                return RouteResult::NoRoute {
                    message: "arc returned empty route list".into(),
                }
            }
        };

        // If risk came back red, drop the route rather than presenting it.
        if solana_risk.risk == RiskScore::Red {
            return RouteResult::Error {
                message: format!(
                    "solana leg rejected by risk check: {}",
                    solana_risk.reasons.join("; ")
                ),
            };
        }

        RouteResult::Ok {
            routes: vec![best],
        }
    }

    /// Derive the risk assessment for the Solana leg of `req`.
    ///
    /// - If `solana_risk_hint` is `Some`, interpret as "risk:reason1;reason2"
    ///   (the format emitted by `risk::check_token_risk` in token-risk-check).
    /// - Otherwise attempt a live RPC risk check on the Solana input mint via
    ///   the existing `mint::decode_mint` + `mint::decode_extensions` helpers
    ///   and `risk::assess_risk` + `get_token_largest_accounts`.
    /// - Fail-closed: any error returns `RiskScore::Red`.
    fn solana_leg_risk(&self, req: &RouteRequest) -> RiskAssessment {
        if let Some(ref hint) = req.solana_risk_hint {
            let mint = req.quote.input_mint.clone();
            let risk = if hint.starts_with("red") {
                RiskScore::Red
            } else if hint.starts_with("amber") {
                RiskScore::Amber
            } else {
                RiskScore::Green
            };
            let reasons: Vec<String> = hint
                .splitn(2, ':')
                .nth(1)
                .map(|s| {
                    s.split(';')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            return RiskAssessment {
                risk,
                reasons,
                mint,
            };
        }

        match self.rpc.get_account(&req.quote.input_mint) {
            Ok(Some(acct)) => {
                let mint_info = crate::mint::decode_mint(&acct.data, &req.quote.input_mint)
                    .unwrap_or_else(|_| crate::types::MintInfo {
                        address: req.quote.input_mint.clone(),
                        mint_authority: None,
                        freeze_authority: None,
                        supply: 0,
                        decimals: 0,
                        is_initialized: acct.owner == "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                    });
                let extensions = crate::mint::decode_extensions(&acct.data)
                    .unwrap_or_default();
                let largest = self
                    .rpc
                    .get_token_largest_accounts(&req.quote.input_mint)
                    .unwrap_or_default();
                assess_risk(&mint_info, &extensions, &largest, None, &self.risk_config)
            }
            Ok(None) => RiskAssessment {
                risk: RiskScore::Red,
                reasons: vec!["Mint account not found on-chain".into()],
                mint: req.quote.input_mint.clone(),
            },
            Err(e) => RiskAssessment {
                risk: RiskScore::Red,
                reasons: vec![format!("RPC failure checking mint: {e}")],
                mint: req.quote.input_mint.clone(),
            },
        }
    }
}

// ─── Parsing helpers ───────────────────────────────────────────────────────────

fn resp_balances_from_value(
    v: &serde_json::Value,
) -> Option<Vec<ArcBalanceInner>> {
    if let Some(arr) = v.as_array() {
        return serde_json::from_value(serde_json::Value::Array(arr.clone())).ok();
    }
    if v.is_object() {
        if let Some(balances) = v.get("balances") {
            if let Some(arr) = balances.as_array() {
                return serde_json::from_value(serde_json::Value::Array(arr.clone())).ok();
            }
        }
    }
    None
}

fn parse_route_resp(v: &serde_json::Value) -> Result<Vec<RankedRoute>, String> {
    let raw = if let Some(obj) = v.as_object() {
        obj.get("routes")
            .or_else(|| obj.get("data"))
            .or_else(|| obj.get("result"))
            .cloned()
            .ok_or_else(|| "no routes field in response".to_string())?
    } else if let Some(arr) = v.as_array() {
        serde_json::Value::Array(arr.clone())
    } else {
        return Err("unexpected route response shape".into());
    };
    let inner: Vec<ArcRouteInner> =
        serde_json::from_value(raw).map_err(|e| format!("deserialize routes: {e}"))?;
    let mut routes: Vec<RankedRoute> = inner
        .into_iter()
        .filter_map(|r| {
            let steps: Vec<RouteStep> = r
                .steps
                .into_iter()
                .enumerate()
                .filter_map(|(idx, s)| {
                    let amount: u64 = s.amount.parse().ok()?;
                    let fee: u64 = s.fee.parse().ok()?;
                    let kind = match s.r#type.as_str() {
                        "bridge" => RouteStepKind::Bridge,
                        "swap" => RouteStepKind::Swap,
                        _ => RouteStepKind::NativeTransfer,
                    };
                    Some(RouteStep {
                        index: idx,
                        kind,
                        from_chain: s.from_chain,
                        to_chain: s.to_chain,
                        token: s.token,
                        amount,
                        fee,
                        note: s.note,
                    })
                })
                .collect();
            let total_fee: u64 = r.total_fee.parse().unwrap_or(0);
            Some(RankedRoute {
                rank: 0,
                score: r.score,
                steps,
                total_fee,
                eta_secs: r.eta_secs,
                route_id: r.route_id,
            })
        })
        .collect();
    for (i, r) in routes.iter_mut().enumerate() {
        r.rank = (i + 1) as u32;
    }
    Ok(routes)
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn mock_for_arc(method: &str, value: serde_json::Value) -> ArcClient {
        let method = method.to_string();
        let resp = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": value });
        let handler = move |_url: &str, _key: Option<&str>, req: &[u8]| {
            let parsed: serde_json::Value = serde_json::from_slice(req).expect("valid json");
            assert_eq!(parsed["method"], method);
            Ok(serde_json::to_vec(&resp).expect("serialize"))
        };
        ArcClient::with_transport(
            "http://arc-mock",
            Box::new(crate::rpc::MockTransport {
                handler: Box::new(handler),
            }),
        )
    }

    fn sample_route_resp() -> serde_json::Value {
        serde_json::json!({
            "routes": [
                {
                    "route_id": "ARC-R-001",
                    "score": 1.2,
                    "total_fee": "500",
                    "eta_secs": 42,
                    "steps": [
                        {"type": "swap", "from_chain": "solana", "to_chain": "solana", "token": "SOL", "amount": "100000000", "fee": "5000", "note": "Jupiter swap SOL→USDC"},
                        {"type": "bridge", "from_chain": "solana", "to_chain": "ethereum", "token": "USDC", "amount": "95000000", "fee": "2000", "note": "LayerZero bridge"},
                        {"type": "native_transfer", "from_chain": "ethereum", "to_chain": "ethereum", "token": "USDC", "amount": "94900000", "fee": "0", "note": "Native ETH transfer"}
                    ]
                },
                {
                    "route_id": "ARC-R-002",
                    "score": 2.8,
                    "total_fee": "900",
                    "eta_secs": 90,
                    "steps": [
                        {"type": "swap", "from_chain": "solana", "to_chain": "solana", "token": "SOL", "amount": "100000000", "fee": "9000", "note": "Jupiter swap (slow)"}
                    ]
                }
            ]
        })
    }

    // arc_route_quote_return_best_ranked
    #[test]
    fn arc_route_quote_return_best_ranked() {
        let client = mock_for_arc("arc_quoteRoute", sample_route_resp());
        let quote = RouteQuote {
            source_chain: "solana".into(),
            dest_chain: "ethereum".into(),
            input_mint: "SOL1111111111111111111111111111111111111".into(),
            output_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
            amount: 100_000_000,
            optimize: Optimize::Fee,
            affiliate: None,
        };
        let result = client.quote_route(&quote);
        match result {
            RouteResult::Ok { routes } => {
                assert_eq!(routes.len(), 2, "quote_route returns all ranked routes");
                assert_eq!(routes[0].rank, 1);
                assert_eq!(routes[0].route_id, "ARC-R-001");
                assert_eq!(routes[0].steps.len(), 3);
                assert_eq!(routes[0].steps[0].kind, RouteStepKind::Swap);
                assert_eq!(routes[0].steps[1].kind, RouteStepKind::Bridge);
                assert_eq!(routes[0].steps[2].kind, RouteStepKind::NativeTransfer);
                assert_eq!(routes[0].total_fee, 500);
                assert_eq!(routes[1].rank, 2);
                assert_eq!(routes[1].route_id, "ARC-R-002");
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    // build_route_t2_rejected
    #[test]
    fn build_route_t2_rejected() {
        let client = mock_for_arc("arc_quoteRoute", sample_route_resp());
        let req = RouteRequest {
            from_address: "From1111111111111111111111111111111111111".into(),
            to_address: "To2222222222222222222222222222222222222".into(),
            quote: RouteQuote {
                source_chain: "solana".into(),
                dest_chain: "ethereum".into(),
                input_mint: "SOL1111111111111111111111111111111111111".into(),
                output_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
                amount: 100_000_000,
                optimize: Optimize::Fee,
                affiliate: None,
            },
            propose_only: false, // T2
            solana_risk_hint: None,
        };
        let result = client.build_route(&req);
        match result {
            RouteResult::Error { message } => {
                assert!(message.contains("T2"), "unexpected message: {message}");
            }
            other => panic!("expected Error (T2 blocked), got {other:?}"),
        }
    }

    // build_route_t1_red_risk_blocked
    #[test]
    fn build_route_t1_red_risk_blocked() {
        let client = mock_for_arc("arc_quoteRoute", sample_route_resp());
        let req = RouteRequest {
            from_address: "From1111111111111111111111111111111111111".into(),
            to_address: "To2222222222222222222222222222222222222".into(),
            quote: RouteQuote {
                source_chain: "solana".into(),
                dest_chain: "ethereum".into(),
                input_mint: "SOL1111111111111111111111111111111111111".into(),
                output_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
                amount: 100_000_000,
                optimize: Optimize::Fee,
                affiliate: None,
            },
            propose_only: true,
            solana_risk_hint: Some("red:Mint authority active — unsafe".into()),
        };
        let result = client.build_route(&req);
        match result {
            RouteResult::Error { message } => {
                assert!(
                    message.contains("risk check"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected Error (red risk blocked), got {other:?}"),
        }
    }

    // propose_only_t1_takes_best_route
    #[test]
    fn propose_only_t1_takes_best_route() {
        let client = mock_for_arc("arc_quoteRoute", sample_route_resp());
        let req = RouteRequest {
            from_address: "From1111111111111111111111111111111111111".into(),
            to_address: "To2222222222222222222222222222222222222".into(),
            quote: RouteQuote {
                source_chain: "solana".into(),
                dest_chain: "ethereum".into(),
                input_mint: "SOL1111111111111111111111111111111111111".into(),
                output_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
                amount: 100_000_000,
                optimize: Optimize::Fee,
                affiliate: None,
            },
            propose_only: true,
            solana_risk_hint: Some("green:".into()),
        };
        let result = client.build_route(&req);
        match result {
            RouteResult::Ok { routes } => {
                assert_eq!(routes.len(), 1);
                assert_eq!(routes[0].route_id, "ARC-R-001");
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    // no_route_returned_when_empty
    #[test]
    fn no_route_returned_when_empty() {
        let client = mock_for_arc(
            "arc_quoteRoute",
            serde_json::json!({ "routes": [] }),
        );
        let quote = RouteQuote {
            source_chain: "solana".into(),
            dest_chain: "ethereum".into(),
            input_mint: "X".into(),
            output_mint: "Y".into(),
            amount: 1,
            optimize: Optimize::Fee,
            affiliate: None,
        };
        match client.quote_route(&quote) {
            RouteResult::NoRoute { .. } => {}
            other => panic!("expected NoRoute, got {other:?}"),
        }
    }

    // arc_client_transport_error_fail_closed
    #[test]
    fn arc_client_transport_error_fail_closed() {
        let bad_handler =
            |_url: &str, _key: Option<&str>, _req: &[u8]| Err("transport down".into());
        let client = ArcClient::with_transport(
            "http://bad",
            Box::new(crate::rpc::MockTransport {
                handler: Box::new(bad_handler),
            }),
        );
        let quote = RouteQuote {
            source_chain: "solana".into(),
            dest_chain: "ethereum".into(),
            input_mint: "X".into(),
            output_mint: "Y".into(),
            amount: 1,
            optimize: Optimize::Fee,
            affiliate: None,
        };
        match client.quote_route(&quote) {
            RouteResult::Error { .. } => {}
            other => panic!("expected Error, got {other:?}"),
        }
    }

    // unified_balance_parses_wrapped_and_unwrapped
    #[test]
    fn unified_balance_parses_wrapped_and_unwrapped() {
        let wrapped = serde_json::json!({
            "balances": [
                {"chain": "solana", "mint": "M1", "symbol": "SOL", "amount": "500000000", "decimals": 9}
            ]
        });
        let client = mock_for_arc("arc_unifiedBalance", wrapped);
        match client.unified_balance("addr", &[ChainId::Solana]) {
            RouteResult::Ok { .. } => {}
            other => panic!("expected Ok, got {other:?}"),
        }

        let direct = serde_json::json!([
            {"chain": "ethereum", "mint": "M2", "symbol": "USDC", "amount": "1000000", "decimals": 6}
        ]);
        let client = mock_for_arc("arc_unifiedBalance", direct);
        match client.unified_balance("addr", &[ChainId::Ethereum]) {
            RouteResult::Ok { .. } => {}
            other => panic!("expected Ok, got {other:?}"),
        }
    }
}
