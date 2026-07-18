use zeroclaw_solana_core::{
    mint, risk,
    rpc::RpcClient,
    types::{ConfigSection, LpInfo, RiskAssessment},
};

/// Check the risk of a token mint by fetching on-chain data and running
/// the risk scoring engine. This is the pure-logic entry point — no wasm deps.
pub fn check_token_risk(
    mint_address: &str,
    config: &ConfigSection,
) -> Result<RiskAssessment, String> {
    let rpc_url = config
        .get("rpc_url")
        .cloned()
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".into());
    let api_key = config.get("rpc_api_key").cloned().filter(|v| !v.is_empty());

    let rpc = RpcClient::new(rpc_url, api_key);
    let risk_config = risk::RiskConfig::from_section(config);

    // 1. Fetch mint account
    let account = rpc
        .get_account(mint_address)
        .map_err(|e| format!("failed to fetch mint: {e}"))?
        .ok_or_else(|| format!("mint account not found: {mint_address}"))?;

    // 2. Decode mint data
    let mint_info = mint::decode_mint(&account.data, mint_address)
        .map_err(|e| format!("failed to decode mint: {e}"))?;

    // 3. Decode Token-2022 extensions
    let extensions = mint::decode_extensions(&account.data).unwrap_or_default();

    // 4. Fetch largest holders
    let largest_accounts = rpc
        .get_token_largest_accounts(mint_address)
        .unwrap_or_default();

    // 5. LP status — attempt Jupiter lookup (wasm), treat failure as "no LP"
    let lp_info = fetch_lp_info(mint_address);

    // 6. Run risk scoring
    Ok(risk::assess_risk(
        &mint_info,
        &extensions,
        &largest_accounts,
        lp_info.as_ref(),
        &risk_config,
    ))
}

/// Attempt to fetch LP info from Jupiter API (wasm only — uses waki).
/// Returns None if the token has no pool or the request fails.
#[cfg(target_family = "wasm")]
fn fetch_lp_info(mint_address: &str) -> Option<LpInfo> {
    use zeroclaw_solana_core::encoding::encode_base58;
    let _ = encode_base58; // keep import surface stable across targets
    let url = format!(
        "https://quote-api.jup.ag/v6/quote?inputMint={}&outputMint=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&amount=1000000",
        mint_address
    );

    let response = waki::Client::new().get(&url).send().ok()?;
    let status = response.status();
    if status != 200 {
        return None;
    }

    let body = response.body().read_to_end().ok()?;
    let json: serde_json::Value = serde_json::from_slice(&body).ok()?;

    // If Jupiter returns a route, a pool exists
    let out_amount = json.get("outAmount")?.as_str()?.parse::<f64>().ok()?;
    if out_amount <= 0.0 {
        return None;
    }

    // Estimate TVL from the quote (rough heuristic)
    Some(LpInfo {
        tvl_usd: out_amount / 1_000_000.0 * 1.0, // rough USDC conversion
        pool_age_hours: 24.0,                    // unknown, assume reasonable
        dex: "jupiter".into(),
    })
}

/// Host builds have no waki — skip the live LP lookup. The scorer treats
/// `None` as "no LP" (red), which is the conservative, fail-closed default.
#[cfg(not(target_family = "wasm"))]
fn fetch_lp_info(_mint_address: &str) -> Option<LpInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn config_defaults() {
        let config = HashMap::new();
        let risk_config = risk::RiskConfig::from_section(&config);
        assert_eq!(risk_config.max_holders_pct, 50.0);
        assert_eq!(risk_config.max_transfer_fee_pct, 5.0);
        assert_eq!(risk_config.min_tvl_usd, 10_000.0);
    }

    #[test]
    fn config_override() {
        let mut config = HashMap::new();
        config.insert("max_holders_pct".into(), "30".into());
        config.insert("min_tvl_usd".into(), "50000".into());
        let risk_config = risk::RiskConfig::from_section(&config);
        assert_eq!(risk_config.max_holders_pct, 30.0);
        assert_eq!(risk_config.min_tvl_usd, 50_000.0);
    }
}
