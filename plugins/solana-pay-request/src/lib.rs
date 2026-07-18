// plugins/solana-pay-request/src/lib.rs
// Thin wasm shim — pure logic lives in the core crate / pay.rs
pub mod pay;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/v0",
        world: "tool-plugin",
    });

    use std::collections::HashMap;

    use crate::pay::generate_pay_request;
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };

    struct SolanaPayRequest;

    const PLUGIN_NAME: &str = "solana-pay-request";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
    const TOOL_NAME: &str = "generate_pay_request";

    #[derive(serde::Deserialize)]
    struct ExecuteArgs {
        recipient: String,
        amount: f64,
        #[serde(default = "default_mint")]
        mint: String,
        memo: Option<String>,
        reference: Option<String>,
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    fn default_mint() -> String {
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into()
    }

    impl PluginInfo for SolanaPayRequest {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }

        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for SolanaPayRequest {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Generate a Solana Pay transfer-request URL for a payment. Automatically checks \
             token risk before generating the request. Returns a solana: URL and QR-ready payload. \
             The payer's wallet builds the transaction at scan time with a fresh blockhash."
                .to_string()
        }

        fn parameters_schema() -> String {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "recipient": {
                        "type": "string",
                        "description": "Solana wallet address to receive payment."
                    },
                    "amount": {
                        "type": "number",
                        "description": "Amount in the smallest unit (e.g., lamports for SOL, base units for USDC)."
                    },
                    "mint": {
                        "type": "string",
                        "description": "SPL token mint address. Defaults to USDC."
                    },
                    "memo": {
                        "type": "string",
                        "description": "Optional memo for the transaction (e.g., 'Table 4 payment')."
                    },
                    "reference": {
                        "type": "string",
                        "description": "Optional reference address for tracking."
                    }
                },
                "required": ["recipient", "amount"]
            })
            .to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            let parsed: ExecuteArgs = match serde_json::from_str(&args) {
                Ok(a) => a,
                Err(e) => {
                    emit(PluginAction::Fail, PluginOutcome::Failure, "invalid arguments");
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("invalid arguments: {e}")),
                    });
                }
            };

            let req = zeroclaw_solana_core::types::PayRequest {
                recipient: parsed.recipient,
                amount: parsed.amount,
                mint: parsed.mint,
                memo: parsed.memo,
                reference: parsed.reference,
            };

            match generate_pay_request(&req, &parsed.config) {
                Ok(result) => {
                    let output = serde_json::to_string(&result).unwrap_or_else(|_| {
                        format!(
                            "{{\"url\":\"{}\",\"summary\":\"{}\"}}",
                            result.url, result.summary
                        )
                    });
                    emit(PluginAction::Complete, PluginOutcome::Success, "pay request generated");
                    Ok(ToolResult {
                        success: true,
                        output,
                        error: None,
                    })
                }
                Err(e) => {
                    emit(
                        PluginAction::Fail,
                        PluginOutcome::Failure,
                        &format!("pay request failed: {e}"),
                    );
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    })
                }
            }
        }
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str) {
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "solana_pay_request::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs: None,
                message: message.to_string(),
            },
        );
    }

    export!(SolanaPayRequest);
}
