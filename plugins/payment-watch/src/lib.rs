// plugins/payment-watch/src/lib.rs
// Thin wasm shim — pure logic lives in the core crate / watch.rs
pub mod watch;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0"],
    });

    use std::collections::HashMap;

    use crate::watch::watch_payment;
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw_solana_core::types::WatchResult;
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };
    use zeroclaw::plugin::types::JsonString;

    struct PaymentWatch;

    const PLUGIN_NAME: &str = "payment-watch";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
    const TOOL_NAME: &str = "watch_payment";

    #[derive(serde::Deserialize)]
    struct ExecuteArgs {
        recipient: String,
        mint: String,
        expected_amount: u64,
        reference: Option<String>,
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    impl PluginInfo for PaymentWatch {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }

        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for PaymentWatch {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Watch a recipient address for an expected SPL token payment. Returns 'paid' with \
             payer details when the payment lands, or 'watching' if still waiting. Designed \
             to be called via SOP (cron) until the payment arrives or timeout."
                .to_string()
        }

        fn parameters_schema() -> JsonString {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "recipient": {
                        "type": "string",
                        "description": "The Solana address to watch for incoming payments."
                    },
                    "mint": {
                        "type": "string",
                        "description": "Expected SPL token mint address."
                    },
                    "expected_amount": {
                        "type": "integer",
                        "description": "Expected amount in smallest unit (e.g., lamports)."
                    },
                    "reference": {
                        "type": "string",
                        "description": "Optional memo/reference to match in the transaction."
                    }
                },
                "required": ["recipient", "mint", "expected_amount"]
            })
            .to_string()
            .into()
        }

        fn execute(args: JsonString) -> Result<ToolResult, String> {
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

            let result = watch_payment(
                &parsed.recipient,
                &parsed.mint,
                parsed.expected_amount,
                parsed.reference.as_deref(),
                &parsed.config,
            );

            let output = serde_json::to_string(&result)
                .unwrap_or_else(|_| "{\"status\":\"error\",\"message\":\"serialization failed\"}".to_string());

            match &result {
                WatchResult::Paid { .. } => {
                    emit(PluginAction::Complete, PluginOutcome::Success, "payment detected");
                }
                WatchResult::Watching { .. } => {
                    emit(PluginAction::Read, PluginOutcome::Success, "still watching");
                }
                WatchResult::Timeout { .. } => {
                    emit(PluginAction::Timeout, PluginOutcome::Failure, "watch timeout");
                }
                WatchResult::Error { .. } => {
                    emit(PluginAction::Fail, PluginOutcome::Failure, "watch error");
                }
            }

            Ok(ToolResult {
                success: true,
                output,
                error: None,
            })
        }
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str) {
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "payment_watch::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs: None,
                message: message.to_string(),
            },
        );
    }

    export!(PaymentWatch);
}
