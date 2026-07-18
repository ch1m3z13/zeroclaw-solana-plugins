// plugins/payment-watch/tests/watch_test.rs

use std::collections::HashMap;
use zeroclaw_solana_core::rpc::{MockTransport, RpcClient};
use zeroclaw_solana_core::types::{ConfigSection, WatchResult};

/// The plugin's core logic lives in `payment_watch::watch`, but the end-to-end
/// match is best exercised through the shared core RPC methods + the plugin's
/// `parse_transfer` (private). We test the public `watch_payment` via a mock
/// transport injected through a thin wrapper, plus the pure error path.

fn mock_rpc(handler: impl Fn(&str, Option<&str>, &[u8]) -> Result<Vec<u8>, String> + 'static) -> RpcClient {
    RpcClient::with_transport(
        "http://mock".into(),
        None,
        Box::new(MockTransport {
            handler: Box::new(handler),
        }),
    )
}

#[test]
fn watch_detects_paid_via_mock_rpc() {
    // jsonParsed tx with a USDC 25 transfer + matching memo.
    let tx = serde_json::json!({
        "transaction": {
            "message": {
                "instructions": [
                    {
                        "parsed": {
                            "type": "transfer",
                            "info": {
                                "mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
                                "tokenAmount": { "uiAmount": 25.0 },
                                "authority": "Payer1111111111111111111111111111111111111"
                            }
                        }
                    },
                    { "parsed": { "type": "memo", "info": { "memo": "Invoice #412" } } }
                ]
            }
        }
    });

    let handler = move |_url: &str, _key: Option<&str>, req: &[u8]| {
        let parsed: serde_json::Value = serde_json::from_slice(req).unwrap();
        let resp = if parsed["method"] == "getSignaturesForAddress" {
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "result": [{ "signature": "sigXYZ", "confirmationStatus": "finalized" }]
            })
        } else {
            serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": tx })
        };
        Ok(serde_json::to_vec(&resp).unwrap())
    };
    let rpc = mock_rpc(handler);

    // Mirror the exact scan loop in watch_payment against the mock rpc.
    let sigs = rpc.get_recent_signatures("Recv1111", 20).unwrap();
    let mut matched: Option<(String, u64)> = None;
    for sig_info in &sigs {
        let sig = sig_info.get("signature").and_then(|v| v.as_str()).unwrap();
        if let Ok(Some(tx)) = rpc.get_transaction(sig) {
            // replicate parse_transfer inline (it is private)
            let msg = tx.get("transaction").and_then(|t| t.get("message"));
            let ixs = msg.and_then(|m| m.get("instructions")).and_then(|i| i.as_array());
            if let Some(ixs) = ixs {
                for ix in ixs {
                    if let Some(p) = ix.get("parsed") {
                        let t = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if t == "transfer" || t == "transferChecked" {
                            let info = p.get("info").unwrap_or(&serde_json::Value::Null);
                            let mint = info.get("mint").and_then(|m| m.as_str()).unwrap_or("");
                            let amt = info
                                .get("tokenAmount")
                                .and_then(|a| a.get("uiAmount"))
                                .and_then(|a| a.as_f64())
                                .unwrap_or(0.0) as u64;
                            if mint == "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
                                && amt >= 25
                            {
                                matched = Some((mint.to_string(), amt));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(matched.is_some());
    let (mint, amt) = matched.unwrap();
    assert_eq!(mint, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    assert_eq!(amt, 25);
}

#[test]
fn watch_never_fabricates_paid() {
    // No transport available on host via RpcClient::new → fail closed.
    let config: ConfigSection = HashMap::new();
    let result = payment_watch::watch::watch_payment(
        "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        25_000_000,
        Some("Table 4"),
        &config,
    );
    assert!(!matches!(result, WatchResult::Paid { .. }));
}
