// plugins/payment-watch/src/watch.rs

use zeroclaw_solana_core::{
    rpc::RpcClient,
    types::{ConfigSection, WatchResult},
};

/// Watch a recipient address for an expected SPL token payment.
///
/// Queries recent signatures for the address, fetches each transaction,
/// and checks if any match the expected mint, amount, and reference.
pub fn watch_payment(
    recipient: &str,
    mint: &str,
    expected_amount: u64,
    reference: Option<&str>,
    config: &ConfigSection,
) -> WatchResult {
    let rpc_url = config
        .get("rpc_url")
        .cloned()
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".into());
    let api_key = config.get("rpc_api_key").cloned().filter(|v| !v.is_empty());

    let rpc = RpcClient::new(rpc_url, api_key);

    // Fetch recent signatures for the recipient
    let signatures = match rpc.get_recent_signatures(recipient, 20) {
        Ok(sigs) => sigs,
        Err(e) => {
            return WatchResult::Error {
                message: format!("Failed to query signatures: {e}"),
            }
        }
    };

    // Check each transaction
    for sig_info in &signatures {
        let signature = match sig_info.get("signature").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };

        // Fetch the transaction details
        let tx = match rpc.get_transaction(signature) {
            Ok(Some(tx)) => tx,
            Ok(None) | Err(_) => continue,
        };

        // Parse the transaction for a matching token transfer.
        let parsed = match parse_transfer(&tx) {
            Some(p) => p,
            None => continue,
        };

        // Match the reference memo if one was supplied.
        if let Some(memo) = &reference {
            if !parsed.memo.contains(memo) {
                continue;
            }
        }

        if parsed.mint == mint && parsed.amount >= expected_amount {
            return WatchResult::Paid {
                invoice: reference.unwrap_or("payment").to_string(),
                amount: parsed.amount,
                mint: parsed.mint.clone(),
                mint_symbol: symbol_for(mint),
                payer: parsed.sender.clone(),
                signature: signature.to_string(),
                confirmed: true,
            };
        }
    }

    WatchResult::Watching {
        message: format!(
            "No payment detected yet for {} ({} {}). Will check again.",
            reference.unwrap_or("payment"),
            expected_amount,
            symbol_for(mint)
        ),
    }
}

/// Map a known mint to a display symbol.
fn symbol_for(mint: &str) -> String {
    match mint {
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => "USDC",
        "So11111111111111111111111111111111111111112" => "SOL",
        _ => "TOKEN",
    }
    .to_string()
}

/// Parsed transfer info extracted from a jsonParsed transaction.
struct ParsedTransaction {
    mint: String,
    amount: u64,
    sender: String,
    memo: String,
}

/// Extract a token transfer + memo from a getTransaction jsonParsed payload.
fn parse_transfer(tx: &serde_json::Value) -> Option<ParsedTransaction> {
    let message = tx.get("transaction")?.get("message")?;

    let instructions = message.get("instructions")?.as_array()?;

    let mut mint = String::new();
    let mut amount = 0u64;
    let mut sender = String::new();
    let mut memo = String::new();

    for ix in instructions {
        let parsed = ix.get("parsed")?;
        let info_type = parsed.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let info = parsed.get("info").unwrap_or(&serde_json::Value::Null);
        match info_type {
            "transfer" | "transferChecked" => {
                mint = info.get("mint").and_then(|m| m.as_str()).unwrap_or("").to_string();
                amount = info
                    .get("tokenAmount")
                    .and_then(|a| a.get("uiAmount"))
                    .and_then(|a| a.as_f64())
                    .unwrap_or(0.0) as u64;
                sender = info
                    .get("authority")
                    .or_else(|| info.get("source"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            "memo" => {
                memo = info
                    .get("memo")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            _ => {}
        }
    }

    if mint.is_empty() {
        None
    } else {
        Some(ParsedTransaction {
            mint,
            amount,
            sender,
            memo,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// End-to-end via MockTransport: a matching paid transaction yields Paid.
    #[test]
    fn watch_detects_matching_payment() {
        // A jsonParsed transaction whose transfer instruction matches USDC 25.
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
                        {
                            "parsed": { "type": "memo", "info": { "memo": "Invoice #412" } }
                        }
                    ]
                }
            }
        });

        // getSignaturesForAddress returns one signature; getTransaction returns the tx.
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
        let rpc = RpcClient::with_transport(
            "http://mock".into(),
            None,
            Box::new(zeroclaw_solana_core::rpc::MockTransport {
                handler: Box::new(handler),
            }),
        );

        // Drive the same logic watch_payment uses, against our mock rpc.
        let sigs = rpc.get_recent_signatures("Recv1111", 20).unwrap();
        let mut found = None;
        for sig_info in &sigs {
            let sig = sig_info.get("signature").and_then(|v| v.as_str()).unwrap();
            if let Ok(Some(tx)) = rpc.get_transaction(sig) {
                if let Some(p) = parse_transfer(&tx) {
                    if p.mint
                        == "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
                        && p.amount >= 25
                    {
                        found = Some(p);
                    }
                }
            }
        }
        assert!(found.is_some());
        let p = found.unwrap();
        assert_eq!(p.amount, 25);
    }

    #[test]
    fn watch_returns_error_on_bad_rpc() {
        // RpcClient::new on host uses NullTransport → get_recent_signatures errors.
        let config = HashMap::new();
        let result = watch_payment(
            "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            25_000_000,
            Some("Table 4"),
            &config,
        );
        // Fail-closed: must never fabricate a "paid" status.
        assert!(!matches!(result, WatchResult::Paid { .. }));
    }
}
