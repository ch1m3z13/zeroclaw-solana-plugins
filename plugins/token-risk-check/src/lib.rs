// plugins/token-risk-check/src/lib.rs
// Thin wasm shim — pure logic lives in the core crate / risk.rs
pub mod risk;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/v0",
        world: "tool-plugin",
    });

    use std::collections::HashMap;

    use crate::risk::check_token_risk;
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome};

    struct TokenRiskCheck;

    const PLUGIN_NAME: &str = "token-risk-check";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
    const TOOL_NAME: &str = "check_token_risk";

    #[derive(serde::Deserialize)]
    struct ExecuteArgs {
        mint: String,
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    impl PluginInfo for TokenRiskCheck {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }

        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for TokenRiskCheck {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Check the risk of an SPL token mint. Returns red/amber/green with reasons \
             based on: mint authority, freeze authority, Token-2022 extensions (transfer fees, \
             hooks, permanent delegate), holder concentration, and LP status. Use this before \
             accepting or sending payments in any token."
                .to_string()
        }

        fn parameters_schema() -> String {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "mint": {
                        "type": "string",
                        "description": "The SPL token mint address to check."
                    }
                },
                "required": ["mint"]
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

            match check_token_risk(&parsed.mint, &parsed.config) {
                Ok(assessment) => {
                    let output = serde_json::to_string(&assessment).unwrap_or_else(|_| {
                        format!(
                            "{{\"risk\":\"{}\",\"reasons\":[],\"mint\":\"{}\"}}",
                            assessment.risk, assessment.mint
                        )
                    });
                    emit(PluginAction::Complete, PluginOutcome::Success, "risk assessed");
                    Ok(ToolResult {
                        success: true,
                        output,
                        error: None,
                    })
                }
                Err(e) => {
                    // Fail closed: return red on any error
                    let output = format!(
                        "{{\"risk\":\"red\",\"reasons\":[\"Unable to verify token — failing closed\"],\"mint\":\"{}\"}}",
                        parsed.mint
                    );
                    emit(
                        PluginAction::Fail,
                        PluginOutcome::Failure,
                        &format!("risk check failed: {e}"),
                    );
                    Ok(ToolResult {
                        success: true, // success=true so the LLM reads the output
                        output,
                        error: None,
                    })
                }
            }
        }
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str) {
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "token_risk_check::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs: None,
                message: message.to_string(),
            },
        );
    }

    export!(TokenRiskCheck);
}
