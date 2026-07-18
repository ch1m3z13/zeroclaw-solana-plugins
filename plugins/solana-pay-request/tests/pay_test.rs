// plugins/solana-pay-request/tests/pay_test.rs

use std::collections::HashMap;
use zeroclaw_solana_core::pay::build_pay_url;
use zeroclaw_solana_core::types::{ConfigSection, PayRequest};

fn config_no_risk() -> ConfigSection {
    let mut c = HashMap::new();
    c.insert("risk_check_enabled".into(), "false".into());
    c
}

#[test]
fn pay_url_basic() {
    let req = PayRequest {
        recipient: "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU".into(),
        amount: 25.0,
        mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
        memo: Some("Invoice #412".into()),
        reference: None,
    };
    let result = build_pay_url(&req).unwrap();
    assert!(result.url.contains("solana:"));
    assert!(result.url.contains("amount=25"));
    assert!(result.url.contains("memo=Invoice%20%23412"));
}

#[test]
fn pay_url_with_reference() {
    let req = PayRequest {
        recipient: "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU".into(),
        amount: 10.0,
        mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
        memo: None,
        reference: Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".into()),
    };
    let result = build_pay_url(&req).unwrap();
    assert!(result.url.contains("reference="));
}

// End-to-end: generate_pay_request with risk check disabled should build a URL.
// (Risk-check-enabled path needs a live RPC, so it's covered by the unit tests
// in pay.rs with risk_check_enabled=false; this guards the gated build path.)
#[test]
fn generate_request_gated_builds_url() {
    use solana_pay_request::pay::generate_pay_request;
    let req = PayRequest {
        recipient: "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU".into(),
        amount: 25.0,
        mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
        memo: Some("Table 4 payment".into()),
        reference: None,
    };
    let config = config_no_risk();
    let result = generate_pay_request(&req, &config).unwrap();
    assert!(result.url.starts_with("solana:"));
    assert!(result.qr_ready);
}
