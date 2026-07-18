/// Parsed SPL Token / Token-2022 mint account data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MintInfo {
    pub address: String,
    pub mint_authority: Option<String>,
    pub freeze_authority: Option<String>,
    pub supply: u64,
    pub decimals: u8,
    pub is_initialized: bool,
}

/// Token-2022 extension parsed from TLV data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum Extension {
    TransferFee {
        fee_basis_points: u16,
        max_fee: u64,
    },
    TransferHook {
        program_id: String,
    },
    PermanentDelegate {
        delegate: String,
    },
    NonTransferable,
    CloseAuthority {
        authority: String,
    },
    DefaultAccountState {
        state: String,
    },
    InterestBearing {
        rate_authority: String,
        rate: i16,
    },
    /// Unknown extension type — we skip it rather than fail
    Unknown {
        type_id: u16,
    },
}

/// Top holder from getTokenLargestAccounts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TokenLargestAccount {
    pub address: String,
    pub amount: u64,
}

/// Liquidity pool info from a DEX aggregator.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LpInfo {
    pub tvl_usd: f64,
    pub pool_age_hours: f64,
    pub dex: String,
}

/// Risk level for a token.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum RiskScore {
    Green,
    Amber,
    Red,
}

impl std::fmt::Display for RiskScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskScore::Green => write!(f, "green"),
            RiskScore::Amber => write!(f, "amber"),
            RiskScore::Red => write!(f, "red"),
        }
    }
}

/// Result of a token risk assessment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RiskAssessment {
    pub risk: RiskScore,
    pub reasons: Vec<String>,
    pub mint: String,
}

/// Raw account data from RPC.
#[derive(Debug, Clone)]
pub struct AccountData {
    pub data: Vec<u8>,
    pub owner: String,
    pub lamports: u64,
}

/// A parsed token account.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TokenAccount {
    pub mint: String,
    pub owner: String,
    pub amount: u64,
    pub delegate: Option<String>,
    pub state: String,
}

/// Latest blockhash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blockhash(pub String);

/// Solana Pay request parameters.
#[derive(Debug, Clone)]
pub struct PayRequest {
    pub recipient: String,
    pub amount: f64,
    pub mint: String,
    pub memo: Option<String>,
    pub reference: Option<String>,
}

/// Result of building a Solana Pay URL.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PayResult {
    pub url: String,
    pub summary: String,
    pub risk: String,
    pub qr_ready: bool,
}

/// Payment watch status.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status")]
pub enum WatchResult {
    #[serde(rename = "paid")]
    Paid {
        invoice: String,
        amount: u64,
        mint: String,
        mint_symbol: String,
        payer: String,
        signature: String,
        confirmed: bool,
    },
    #[serde(rename = "watching")]
    Watching { message: String },
    #[serde(rename = "timeout")]
    Timeout { message: String },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Configuration section injected by the host.
pub type ConfigSection = std::collections::HashMap<String, String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_score_ordering() {
        assert!(RiskScore::Green < RiskScore::Amber);
        assert!(RiskScore::Amber < RiskScore::Red);
        assert_eq!(RiskScore::Red.to_string(), "red");
        assert_eq!(RiskScore::Green.to_string(), "green");
    }

    #[test]
    fn watch_result_round_trips() {
        let paid = WatchResult::Paid {
            invoice: "inv-1".into(),
            amount: 25,
            mint: "Mint1111".into(),
            mint_symbol: "USDC".into(),
            payer: "Payer1111".into(),
            signature: "sig".into(),
            confirmed: true,
        };
        let json = serde_json::to_string(&paid).unwrap();
        assert!(json.contains("\"status\":\"paid\""));
        let back: WatchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back, paid);
    }
}
