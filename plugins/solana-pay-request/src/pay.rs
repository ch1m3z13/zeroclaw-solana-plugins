// plugins/solana-pay-request/src/pay.rs

use zeroclaw_solana_core::{
    pay,
    types::{ConfigSection, PayRequest, PayResult},
};

/// Generate a Solana Pay transfer-request URL, gated by token risk check.
///
/// Flow:
/// 1. If risk_check_enabled, call risk scoring on the mint
/// 2. If risk = red → return error
/// 3. If risk = amber → include warning
/// 4. If risk = green → proceed
/// 5. Build and return the solana: URL
pub fn generate_pay_request(
    req: &PayRequest,
    config: &ConfigSection,
) -> Result<PayResult, String> {
    let risk_check_enabled = config
        .get("risk_check_enabled")
        .map(|v| v != "false")
        .unwrap_or(true);

    let mut risk_level = "green".to_string();
    let mut risk_warning = String::new();

    // 1. Risk gate (delegates to the token-risk-check plugin's logic)
    if risk_check_enabled {
        match token_risk_check::risk::check_token_risk(&req.mint, config) {
            Ok(assessment) => {
                risk_level = assessment.risk.to_string();
                match assessment.risk {
                    zeroclaw_solana_core::types::RiskScore::Red => {
                        return Err(format!(
                            "Token {} is RED risk: {}. Refusing to generate payment request.",
                            req.mint,
                            assessment.reasons.join("; ")
                        ));
                    }
                    zeroclaw_solana_core::types::RiskScore::Amber => {
                        risk_warning = format!(
                            " WARNING: Token has amber risk — {}.",
                            assessment.reasons.join("; ")
                        );
                    }
                    _ => {}
                }
            }
            Err(e) => {
                // Fail closed: don't generate request if risk check fails
                return Err(format!(
                    "Risk check failed for {}: {}. Cannot generate payment request.",
                    req.mint, e
                ));
            }
        }
    }

    // 2. Check allowed_mints if configured
    if let Some(allowed) = config.get("allowed_mints") {
        if !allowed.is_empty() {
            let allowed_list: Vec<&str> = allowed.split(',').map(|s| s.trim()).collect();
            if !allowed_list.contains(&req.mint.as_str()) {
                return Err(format!(
                    "Mint {} is not in the allowed mints list: {}",
                    req.mint, allowed
                ));
            }
        }
    }

    // 3. Build Solana Pay URL
    let mut result = pay::build_pay_url(req)?;
    result.risk = risk_level.clone();

    // 4. Append risk warning to summary if amber
    if !risk_warning.is_empty() {
        result.summary.push_str(&risk_warning);
    } else {
        result.summary = format!("Risk check: {}. {}", risk_level.to_uppercase(), result.summary);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use super::*;

    fn valid_recipient() -> String {
        "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU".into()
    }

    fn valid_mint() -> String {
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into()
    }

    fn config_with_risk_disabled() -> ConfigSection {
        let mut c = HashMap::new();
        c.insert("risk_check_enabled".into(), "false".into());
        c
    }

    #[test]
    fn pay_request_no_risk_check() {
        let req = PayRequest {
            recipient: valid_recipient(),
            amount: 25.0,
            mint: valid_mint(),
            memo: Some("Test payment".into()),
            reference: None,
        };
        let config = config_with_risk_disabled();
        let result = generate_pay_request(&req, &config).unwrap();
        assert!(result.url.starts_with("solana:"));
        assert!(result.qr_ready);
    }

    #[test]
    fn pay_request_blocked_mint() {
        let req = PayRequest {
            recipient: valid_recipient(),
            amount: 10.0,
            mint: valid_mint(),
            memo: None,
            reference: None,
        };
        let mut config = config_with_risk_disabled();
        config.insert(
            "allowed_mints".into(),
            "So11111111111111111111111111111111111111112".into(),
        );
        let result = generate_pay_request(&req, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the allowed mints"));
    }

    #[test]
    fn pay_request_invalid_recipient() {
        let req = PayRequest {
            recipient: "bad-key".into(),
            amount: 10.0,
            mint: valid_mint(),
            memo: None,
            reference: None,
        };
        let config = config_with_risk_disabled();
        assert!(generate_pay_request(&req, &config).is_err());
    }

    #[test]
    fn pay_request_zero_amount() {
        let req = PayRequest {
            recipient: valid_recipient(),
            amount: 0.0,
            mint: valid_mint(),
            memo: None,
            reference: None,
        };
        let config = config_with_risk_disabled();
        assert!(generate_pay_request(&req, &config).is_err());
    }

    #[test]
    fn pay_request_summary_has_risk_check_label() {
        let req = PayRequest {
            recipient: valid_recipient(),
            amount: 25.0,
            mint: valid_mint(),
            memo: Some("x".into()),
            reference: None,
        };
        let config = config_with_risk_disabled();
        let result = generate_pay_request(&req, &config).unwrap();
        assert!(result.summary.starts_with("Risk check: GREEN"));
    }
}
