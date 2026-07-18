use crate::types::{Extension, LpInfo, MintInfo, RiskAssessment, RiskScore, TokenLargestAccount};

/// Well-known issuer mints whose active mint/freeze authority is expected
/// (held by a regulated issuer) rather than a rug signal. USDC + USDT.
pub const DEFAULT_TRUSTED_ISSUERS: &[&str] = &[
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", // USDC
    "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB", // USDT
];

/// Configuration for risk thresholds.
pub struct RiskConfig {
    pub max_holders_pct: f64,
    pub max_transfer_fee_pct: f64,
    pub min_tvl_usd: f64,
    /// Mints on this allowlist skip the authority + no-LP hard-reds (score
    /// stays Green with an informational note). Other signals still apply.
    pub trusted_issuers: Vec<String>,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_holders_pct: 50.0,
            max_transfer_fee_pct: 5.0,
            min_tvl_usd: 10_000.0,
            trusted_issuers: DEFAULT_TRUSTED_ISSUERS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

impl RiskConfig {
    pub fn from_section(section: &std::collections::HashMap<String, String>) -> Self {
        let max_holders_pct = section
            .get("max_holders_pct")
            .and_then(|v| v.parse().ok())
            .unwrap_or(50.0);
        let max_transfer_fee_pct = section
            .get("max_transfer_fee_pct")
            .and_then(|v| v.parse().ok())
            .unwrap_or(5.0);
        let min_tvl_usd = section
            .get("min_tvl_usd")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000.0);
        // Comma-separated override; falls back to the built-in allowlist.
        let trusted_issuers = section
            .get("trusted_issuers")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_else(|| {
                DEFAULT_TRUSTED_ISSUERS
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            });
        Self {
            max_holders_pct,
            max_transfer_fee_pct,
            min_tvl_usd,
            trusted_issuers,
        }
    }
}

/// Assess the risk of a given SPL mint based on on-chain data.
///
/// Returns a RiskAssessment with risk level (green/amber/red) and up to 2 reasons.
pub fn assess_risk(
    mint_info: &MintInfo,
    extensions: &[Extension],
    largest_accounts: &[TokenLargestAccount],
    lp_info: Option<&LpInfo>,
    config: &RiskConfig,
) -> RiskAssessment {
    let mut score = RiskScore::Green;
    let mut reasons = Vec::new();

    let is_trusted = config
        .trusted_issuers
        .iter()
        .any(|m| m == &mint_info.address);

    // 1. Mint authority — hard red if active, unless a trusted issuer
    if mint_info.mint_authority.is_some() {
        if is_trusted {
            reasons.push("Mint/freeze authority held by a known issuer — expected".to_string());
        } else {
            score = RiskScore::Red;
            reasons
                .push("Mint authority is active — supply can be inflated at any time".to_string());
        }
    }

    // 2. Freeze authority — hard red if active, unless a trusted issuer
    if mint_info.freeze_authority.is_some() && !is_trusted {
        score = RiskScore::Red;
        reasons
            .push("Freeze authority is active — tokens can be frozen by third party".to_string());
    }

    // 3. Token-2022 extensions
    for ext in extensions {
        match ext {
            Extension::PermanentDelegate { .. } => {
                score = RiskScore::Red;
                reasons.push(
                    "Permanent delegate extension — tokens can be taken without consent"
                        .to_string(),
                );
            }
            Extension::TransferFee {
                fee_basis_points, ..
            } => {
                let fee_pct = *fee_basis_points as f64 / 100.0;
                if fee_pct > config.max_transfer_fee_pct {
                    score = RiskScore::Red;
                    reasons.push(format!(
                        "Transfer fee {:.1}% exceeds safe threshold of {:.0}%",
                        fee_pct, config.max_transfer_fee_pct
                    ));
                } else if fee_pct > 1.0 {
                    score = score.max(RiskScore::Amber);
                    reasons.push(format!(
                        "Transfer fee {:.1}% — verify this is expected",
                        fee_pct
                    ));
                }
            }
            Extension::TransferHook { program_id } => {
                score = score.max(RiskScore::Amber);
                reasons.push(format!(
                    "Transfer hook present ({}) — external program executes on every transfer",
                    &program_id[..8.min(program_id.len())]
                ));
            }
            Extension::NonTransferable => {
                score = RiskScore::Red;
                reasons.push("Non-transferable token — cannot be sent or received".to_string());
            }
            _ => {}
        }
    }

    // 4. Holder concentration
    if !largest_accounts.is_empty() && mint_info.supply > 0 {
        let top1_pct = largest_accounts[0].amount as f64 / mint_info.supply as f64 * 100.0;
        if top1_pct > config.max_holders_pct {
            score = RiskScore::Red;
            reasons.push(format!("Top holder owns {:.0}% of supply", top1_pct));
        } else if top1_pct > config.max_holders_pct * 0.6 {
            score = score.max(RiskScore::Amber);
            reasons.push(format!("Top holder owns {:.0}% of supply", top1_pct));
        }
    }

    // 5. LP status
    if let Some(lp) = lp_info {
        if lp.tvl_usd < config.min_tvl_usd {
            score = score.max(RiskScore::Amber);
            reasons.push(format!("Low liquidity: ${:.0} TVL", lp.tvl_usd));
        }
    } else if !is_trusted {
        score = RiskScore::Red;
        reasons.push("No liquidity pool found — token may be untradeable".to_string());
    }

    // 6. Supply sanity
    if mint_info.supply == 0 {
        score = RiskScore::Red;
        reasons.push("Zero supply — no tokens in circulation".to_string());
    } else if mint_info.supply > 1_000_000_000_000 && !is_trusted {
        score = score.max(RiskScore::Amber);
        reasons.push("Extremely high supply — likely meme/scam token".to_string());
    }

    // Cap reasons at 2 for token efficiency
    reasons.truncate(2);

    RiskAssessment {
        risk: score,
        reasons,
        mint: mint_info.address.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn green_mint() -> MintInfo {
        MintInfo {
            address: "So1anaTokenMint1111111111111111111111111111".to_string(),
            mint_authority: None,
            freeze_authority: None,
            supply: 1_000_000_000,
            decimals: 6,
            is_initialized: true,
        }
    }

    #[test]
    fn green_path() {
        let mint = green_mint();
        let holders = vec![
            TokenLargestAccount {
                address: "a".into(),
                amount: 100_000_000,
            },
            TokenLargestAccount {
                address: "b".into(),
                amount: 80_000_000,
            },
        ];
        let lp = LpInfo {
            tvl_usd: 50_000.0,
            pool_age_hours: 100.0,
            dex: "jupiter".into(),
        };
        let result = assess_risk(&mint, &[], &holders, Some(&lp), &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Green);
        assert!(result.reasons.is_empty());
    }

    #[test]
    fn red_on_mint_authority() {
        let mut mint = green_mint();
        mint.mint_authority = Some("authority11111111111111111111111111111111".into());
        let result = assess_risk(&mint, &[], &[], None, &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Red);
        assert!(result.reasons[0].contains("Mint authority"));
    }

    #[test]
    fn red_on_freeze_authority() {
        let mut mint = green_mint();
        mint.freeze_authority = Some("freeze1111111111111111111111111111111111".into());
        let result = assess_risk(&mint, &[], &[], None, &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Red);
        assert!(result
            .reasons
            .iter()
            .any(|r| r.contains("Freeze authority")));
    }

    #[test]
    fn trusted_issuer_green_despite_authorities() {
        // USDC-like: active mint + freeze authority, no LP info, but allowlisted.
        let mut mint = green_mint();
        mint.address = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into();
        mint.mint_authority = Some("circle11111111111111111111111111111111111".into());
        mint.freeze_authority = Some("circle11111111111111111111111111111111111".into());
        let result = assess_risk(&mint, &[], &[], None, &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Green);
        assert!(result.reasons.iter().any(|r| r.contains("known issuer")));
    }

    #[test]
    fn trusted_issuer_still_red_on_real_signal() {
        // Allowlisted mint but with a genuine danger (permanent delegate) still reds.
        let mut mint = green_mint();
        mint.address = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into();
        mint.mint_authority = Some("circle11111111111111111111111111111111111".into());
        let ext = Extension::PermanentDelegate {
            delegate: "x".into(),
        };
        let result = assess_risk(&mint, &[ext], &[], None, &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Red);
    }

    #[test]
    fn red_on_permanent_delegate() {
        let ext = Extension::PermanentDelegate {
            delegate: "x".into(),
        };
        let result = assess_risk(&green_mint(), &[ext], &[], None, &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Red);
        assert!(result
            .reasons
            .iter()
            .any(|r| r.contains("Permanent delegate")));
    }

    #[test]
    fn amber_on_high_transfer_fee() {
        let ext = Extension::TransferFee {
            fee_basis_points: 350,
            max_fee: 1000,
        };
        let lp = LpInfo {
            tvl_usd: 50_000.0,
            pool_age_hours: 100.0,
            dex: "jupiter".into(),
        };
        let result = assess_risk(
            &green_mint(),
            &[ext],
            &[],
            Some(&lp),
            &RiskConfig::default(),
        );
        assert_eq!(result.risk, RiskScore::Amber);
        assert!(result.reasons.iter().any(|r| r.contains("Transfer fee")));
    }

    #[test]
    fn red_on_extreme_transfer_fee() {
        let ext = Extension::TransferFee {
            fee_basis_points: 600,
            max_fee: 1000,
        };
        let lp = LpInfo {
            tvl_usd: 50_000.0,
            pool_age_hours: 100.0,
            dex: "jupiter".into(),
        };
        let result = assess_risk(
            &green_mint(),
            &[ext],
            &[],
            Some(&lp),
            &RiskConfig::default(),
        );
        assert_eq!(result.risk, RiskScore::Red);
    }

    #[test]
    fn amber_on_transfer_hook() {
        let ext = Extension::TransferHook {
            program_id: "hook11111111111111111111111111111111".into(),
        };
        let lp = LpInfo {
            tvl_usd: 50_000.0,
            pool_age_hours: 100.0,
            dex: "jupiter".into(),
        };
        let result = assess_risk(
            &green_mint(),
            &[ext],
            &[],
            Some(&lp),
            &RiskConfig::default(),
        );
        assert_eq!(result.risk, RiskScore::Amber);
    }

    #[test]
    fn red_on_holder_concentration() {
        let holders = vec![TokenLargestAccount {
            address: "a".into(),
            amount: 600_000_000,
        }];
        let result = assess_risk(&green_mint(), &[], &holders, None, &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Red);
        assert!(result.reasons.iter().any(|r| r.contains("Top holder")));
    }

    #[test]
    fn red_on_no_lp() {
        let result = assess_risk(&green_mint(), &[], &[], None, &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Red);
        assert!(result.reasons.iter().any(|r| r.contains("No liquidity")));
    }

    #[test]
    fn amber_on_low_lp() {
        let lp = LpInfo {
            tvl_usd: 5_000.0,
            pool_age_hours: 10.0,
            dex: "jupiter".into(),
        };
        let result = assess_risk(&green_mint(), &[], &[], Some(&lp), &RiskConfig::default());
        assert_eq!(result.risk, RiskScore::Amber);
    }

    #[test]
    fn reasons_capped_at_two() {
        let mut mint = green_mint();
        mint.mint_authority = Some("a".into());
        mint.freeze_authority = Some("b".into());
        let ext = Extension::PermanentDelegate {
            delegate: "c".into(),
        };
        let result = assess_risk(&mint, &[ext], &[], None, &RiskConfig::default());
        assert!(result.reasons.len() <= 2);
    }
}
