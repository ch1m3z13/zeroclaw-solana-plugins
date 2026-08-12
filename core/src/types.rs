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

/// Supported chain identifiers for multi-chain routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainId {
    Solana,
    Ethereum,
    Base,
    Arbitrum,
    Optimism,
    Polygon,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainId::Solana => write!(f, "solana"),
            ChainId::Ethereum => write!(f, "ethereum"),
            ChainId::Base => write!(f, "base"),
            ChainId::Arbitrum => write!(f, "arbitrum"),
            ChainId::Optimism => write!(f, "optimism"),
            ChainId::Polygon => write!(f, "polygon"),
            ChainId::Unknown => write!(f, "unknown"),
        }
    }
}

/// Balance of a token on a specific chain, returned by Arc `unifiedBalance`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ArcBalance {
    /// Chain this balance is on (e.g. "solana", "ethereum").
    pub chain: String,
    /// Token mint or contract address on that chain.
    pub mint: String,
    /// Human-readable symbol (e.g. "USDC", "SOL").
    pub symbol: String,
    /// Balance in the token's base units.
    pub amount: u64,
    /// Number of decimals used by the token.
    pub decimals: u8,
}

/// A single step in a proposed payment route.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RouteStep {
    /// Zero-indexed step number.
    pub index: usize,
    /// Kind of routing step this is.
    pub kind: RouteStepKind,
    /// Source chain for this leg.
    pub from_chain: String,
    /// Destination chain for this leg.
    pub to_chain: String,
    /// Token symbol transferred in this step.
    pub token: String,
    /// Estimated or quoted amount that moves through this step (base units).
    pub amount: u64,
    /// Step-level fee in base units of `token`
    pub fee: u64,
    /// Human-readable note.
    pub note: String,
}

/// The kind of work a single route step performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteStepKind {
    /// Native transfer on a single chain (no swap, no bridge).
    NativeTransfer,
    /// DEX swap on a single chain.
    Swap,
    /// Cross-chain bridge leg.
    Bridge,
}

/// A fully-quoted route returned by Arc, ranked by the requested optimization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RankedRoute {
    /// 1-based rank (1 = best).
    pub rank: u32,
    /// Score used for ranking — lower is better for fee, higher for speed; only
    /// comparable within a single quote response.
    pub score: f64,
    /// Ordered steps required to execute this route.
    pub steps: Vec<RouteStep>,
    /// Total fee across all steps in base units of the input token.
    pub total_fee: u64,
    /// Estimated settlement time in seconds (best-effort from Arc).
    pub eta_secs: u64,
    /// Arc's route identifier (opaque, for support/tracing).
    pub route_id: String,
}

/// Optimization target for route selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Optimize {
    /// Minimize total fee paid.
    Fee,
    /// Minimize time-to-settlement.
    Speed,
    /// Balance fee and speed.
    Balanced,
}

/// Parameters for an Arc route quote.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RouteQuote {
    /// Source chain the sender holds funds on.
    pub source_chain: String,
    /// Destination chain the recipient expects funds on.
    pub dest_chain: String,
    /// Input token mint on `source_chain`.
    pub input_mint: String,
    /// Output token mint on `dest_chain` (may equal `input_mint` for native transfer).
    pub output_mint: String,
    /// Amount to send in base units of `input_mint`.
    pub amount: u64,
    /// Optimization target.
    pub optimize: Optimize,
    /// Optional Arc sub-affiliate path (opaque; passed through verbatim).
    pub affiliate: Option<String>,
}

/// The overall result of an Arc route build/quote call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RouteResult {
    /// Success: one or more ranked routes returned.
    Ok { routes: Vec<RankedRoute> },
    /// No routes available for the requested pair/amount.
    NoRoute { message: String },
    /// Arc or transport error — fail-closed.
    Error { message: String },
}

/// Parameters supplied by the caller requesting a route build.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RouteRequest {
    /// Wallet address the payment originates from (for balance lookup + quoting).
    pub from_address: String,
    /// Wallet address the payment is destined for.
    pub to_address: String,
    /// Route quote + routing parameters.
    pub quote: RouteQuote,
    /// If true the caller wants an unsigned route proposal (T1). If false,
    /// the caller would receive executable transaction instructions (T2 — not
    /// exposed by this plugin; custody stays T1).
    pub propose_only: bool,
    /// Optional risk hint: when present the Solana leg reuses the existing
    /// `risk::assess_risk` result. Non-Solana legs gate on an allowlist.
    pub solana_risk_hint: Option<String>,
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
