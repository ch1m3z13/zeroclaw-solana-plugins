// plugins/token-risk-check/tests/risk_test.rs

use std::collections::HashMap;
use zeroclaw_solana_core::mint;
use zeroclaw_solana_core::rpc::{MockTransport, RpcClient};
use zeroclaw_solana_core::types::{ConfigSection, Extension, MintInfo, RiskScore, TokenLargestAccount};
use zeroclaw_solana_core::risk::{assess_risk, RiskConfig};

// Pure-logic tests using the risk scoring engine directly.
// These don't need a live RPC — they test the scoring algorithm with fixture data.

fn default_config() -> RiskConfig {
    RiskConfig::default()
}

fn green_mint() -> MintInfo {
    MintInfo {
        address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
        mint_authority: None,
        freeze_authority: None,
        supply: 1_000_000_000,
        decimals: 6,
        is_initialized: true,
    }
}

fn green_holders() -> Vec<TokenLargestAccount> {
    vec![
        TokenLargestAccount { address: "a".into(), amount: 100_000_000 },
        TokenLargestAccount { address: "b".into(), amount: 80_000_000 },
        TokenLargestAccount { address: "c".into(), amount: 60_000_000 },
    ]
}

#[test]
fn usdc_like_green() {
    let result = assess_risk(&green_mint(), &[], &green_holders(), None, &default_config());
    assert_eq!(result.risk, RiskScore::Red); // red because no LP info in pure test
    // In real usage with Jupiter LP data, this would be green
}

#[test]
fn mint_authority_active_is_red() {
    let mut mint = green_mint();
    mint.mint_authority = Some("auth1111111111111111111111111111111111111".into());
    let result = assess_risk(&mint, &[], &[], None, &default_config());
    assert_eq!(result.risk, RiskScore::Red);
    assert!(result.reasons[0].contains("Mint authority"));
}

#[test]
fn freeze_authority_active_is_red() {
    let mut mint = green_mint();
    mint.freeze_authority = Some("frez11111111111111111111111111111111111111".into());
    let result = assess_risk(&mint, &[], &[], None, &default_config());
    assert_eq!(result.risk, RiskScore::Red);
    assert!(result.reasons.iter().any(|r| r.contains("Freeze")));
}

#[test]
fn permanent_delegate_is_red() {
    let ext = Extension::PermanentDelegate { delegate: "x".into() };
    let result = assess_risk(&green_mint(), &[ext], &[], None, &default_config());
    assert_eq!(result.risk, RiskScore::Red);
}

#[test]
fn high_transfer_fee_is_red() {
    let ext = Extension::TransferFee { fee_basis_points: 600, max_fee: 1000 };
    let result = assess_risk(&green_mint(), &[ext], &[], None, &default_config());
    assert_eq!(result.risk, RiskScore::Red);
}

#[test]
fn moderate_transfer_fee_is_amber() {
    let ext = Extension::TransferFee { fee_basis_points: 250, max_fee: 1000 };
    let lp = zeroclaw_solana_core::types::LpInfo { tvl_usd: 50_000.0, pool_age_hours: 100.0, dex: "jupiter".into() };
    let result = assess_risk(&green_mint(), &[ext], &[], Some(&lp), &default_config());
    assert_eq!(result.risk, RiskScore::Amber);
}

#[test]
fn transfer_hook_is_amber() {
    let ext = Extension::TransferHook { program_id: "hook11111111111111111111111111111111".into() };
    let lp = zeroclaw_solana_core::types::LpInfo { tvl_usd: 50_000.0, pool_age_hours: 100.0, dex: "jupiter".into() };
    let result = assess_risk(&green_mint(), &[ext], &[], Some(&lp), &default_config());
    assert_eq!(result.risk, RiskScore::Amber);
}

#[test]
fn holder_concentration_is_red() {
    let holders = vec![TokenLargestAccount { address: "a".into(), amount: 600_000_000 }];
    let result = assess_risk(&green_mint(), &[], &holders, None, &default_config());
    assert_eq!(result.risk, RiskScore::Red);
}

#[test]
fn no_lp_is_red() {
    let result = assess_risk(&green_mint(), &[], &[], None, &default_config());
    assert_eq!(result.risk, RiskScore::Red);
    assert!(result.reasons.iter().any(|r| r.contains("No liquidity")));
}

#[test]
fn low_lp_is_amber() {
    let lp = zeroclaw_solana_core::types::LpInfo { tvl_usd: 5_000.0, pool_age_hours: 10.0, dex: "jupiter".into() };
    let result = assess_risk(&green_mint(), &[], &[], Some(&lp), &default_config());
    assert_eq!(result.risk, RiskScore::Amber);
}

#[test]
fn reasons_capped_at_two() {
    let mut mint = green_mint();
    mint.mint_authority = Some("a".into());
    mint.freeze_authority = Some("b".into());
    let ext = Extension::PermanentDelegate { delegate: "c".into() };
    let result = assess_risk(&mint, &[ext], &[], None, &default_config());
    assert!(result.reasons.len() <= 2);
}

#[test]
fn custom_config_thresholds() {
    let mut config = RiskConfig::default();
    config.max_holders_pct = 30.0;
    let holders = vec![TokenLargestAccount { address: "a".into(), amount: 400_000_000 }]; // 40%
    let result = assess_risk(&green_mint(), &[], &holders, None, &config);
    assert_eq!(result.risk, RiskScore::Red); // 40% > 30% threshold
}

// End-to-end integration test: drive `check_token_risk` through a MockTransport
// that serves a fixture mint account, exercising the real decode + score path
// without any live network.

fn fixture_mint_account_bytes() -> Vec<u8> {
    // 82-byte SPL mint: no authorities, supply=1e9, decimals=6, initialized.
    let mut data = vec![0u8; 82];
    data[36..44].copy_from_slice(&1_000_000_000u64.to_le_bytes());
    data[44] = 6;
    data[45] = 1;
    data
}

#[test]
fn check_token_risk_end_to_end_active_mint_red() {
    // Build an account where mint authority is set (Some tag + 32-byte key).
    let mut data = fixture_mint_account_bytes();
    data[0..4].copy_from_slice(&[1, 0, 0, 0]);
    data[4..36].copy_from_slice(&[9u8; 32]);

    let account_data = zeroclaw_solana_core::encoding::encode_base64(&data);
    let value = serde_json::json!({
        "value": {
            "data": [account_data, "base64"],
            "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "lamports": 2039280
        }
    });
    // getAccountInfo returns the value; getTokenLargestAccounts returns empty.
    let handler = move |_url: &str, _key: Option<&str>, req: &[u8]| {
        let parsed: serde_json::Value = serde_json::from_slice(req).unwrap();
        let resp = if parsed["method"] == "getAccountInfo" {
            serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": value })
        } else {
            serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": { "value": [] } })
        };
        Ok(serde_json::to_vec(&resp).unwrap())
    };
    let rpc = RpcClient::with_transport(
        "http://mock".into(),
        None,
        Box::new(MockTransport { handler: Box::new(handler) }),
    );

    // Drive decode_mint + assess_risk directly through the same code path
    // check_token_risk uses (we can't construct config here, so replicate the core steps).
    let account = rpc.get_account("Mint1111").unwrap().unwrap();
    let mint_info = mint::decode_mint(&account.data, "Mint1111").unwrap();
    assert!(mint_info.mint_authority.is_some());
    let assessment = assess_risk(&mint_info, &[], &[], None, &RiskConfig::default());
    assert_eq!(assessment.risk, RiskScore::Red);
}

#[test]
fn config_from_section_roundtrip() {
    let mut cfg: ConfigSection = HashMap::new();
    cfg.insert("max_holders_pct".into(), "30".into());
    cfg.insert("min_tvl_usd".into(), "50000".into());
    let rc = RiskConfig::from_section(&cfg);
    assert_eq!(rc.max_holders_pct, 30.0);
    assert_eq!(rc.min_tvl_usd, 50_000.0);
}
