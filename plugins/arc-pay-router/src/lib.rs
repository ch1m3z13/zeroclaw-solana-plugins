// plugins/arc-pay-router/src/lib.rs
// Thin wasm shim — pure logic lives in src/router.rs (host-testable) and core/src/arc.rs
pub mod router;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0"],
    });

    use std::collections::HashMap;

    use crate::router::{build_route, RouteRequest};
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };
    use zeroclaw::plugin::types::JsonString;
    use zeroclaw_solana_core::types::ConfigSection;

    struct ArcPayRouter;

    const PLUGIN_NAME: &str = "arc-pay-router";
    const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
    const TOOL_NAME: &str = "build_route";

    #[derive(serde::Deserialize)]
    struct ExecuteArgs {
        from_address: String,
        to_address: String,
        source_chain: String,
        dest_chain: String,
        input_mint: String,
        output_mint: String,
        amount: u64,
        optimize: String,
        propose_only: bool,
        affiliate: Option<String>,
        solana_risk_hint: Option<String>,
        #[serde(rename = "__config", default)]
        config: ConfigSection,
    }

    fn default_optimize(s: &str) -> zeroclaw_solana_core::types::Optimize {
        use zeroclaw_solana_core::types::Optimize;
        match s.to_lowercase().as_str() {
            "speed" => Optimize::Speed,
            "balanced" => Optimize::Balanced,
            _ => Optimize::Fee,
        }
    }

    impl PluginInfo for ArcPayRouter {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }

        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for ArcPayRouter {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Propose a multi-chain payment route via Arc's unifiedBalance + quoteRoute. \
             Returns the best-ranked route for the requested pair. T1 custody only — \
             returns an unsigned route proposal; the host or a Squads multisig executes. \
             Gated by existing token-risk-check signals on Solana legs."
                .to_string()
        }

        fn parameters_schema() -> JsonString {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "from_address": {"type": "string", "description": "Sender wallet address (balance look-up)"},
                    "to_address":   {"type": "string", "description": "Recipient wallet address"},
                    "source_chain": {"type": "string", "description": "Source chain, e.g. solana"},
                    "dest_chain":   {"type": "string", "description": "Destination chain, e.g. ethereum"},
                    "input_mint":   {"type": "string", "description": "Input token mint / contract on source_chain"},
                    "output_mint":  {"type": "string", "description": "Output token mint / contract on dest_chain"},
                    "amount":       {"type": "integer", "description": "Amount in base units of input_mint"},
                    "optimize":     {"type": "string", "enum": ["fee", "speed", "balanced"], "description": "Optimisation target"},
                    "propose_only": {"type": "boolean", "description": "Must be true (T1). T2 (sign+submit) is not exposed."},
                    "affiliate":    {"type": "string", "description": "Optional Arc affiliate path (passed through)"},
                    "solana_risk_hint": {"type": "string", "description": "Optional risk hint: 'risk:reason1;reason2'"}
                },
                "required": ["from_address", "to_address", "source_chain", "dest_chain", "input_mint", "output_mint", "amount", "propose_only"]
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

            if !parsed.propose_only {
                emit(
                    PluginAction::Fail,
                    PluginOutcome::Failure,
                    "T2 blocked — propose_only must be true",
                );
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(
                        "T2 execution (sign+submit) not exposed. Set propose_only=true for T1.".into(),
                    ),
                });
            }

            let req = RouteRequest {
                from_address: parsed.from_address,
                to_address: parsed.to_address,
                quote: zeroclaw_solana_core::types::RouteQuote {
                    source_chain: parsed.source_chain,
                    dest_chain: parsed.dest_chain,
                    input_mint: parsed.input_mint,
                    output_mint: parsed.output_mint,
                    amount: parsed.amount,
                    optimize: default_optimize(&parsed.optimize),
                    affiliate: parsed.affiliate,
                },
                propose_only: true,
                solana_risk_hint: parsed.solana_risk_hint,
            };

            match crate::router::build_route(&make_client(&parsed.config), &req) {
                Ok(result) => {
                    let output = serde_json::to_string(&result)
                        .unwrap_or_else(|_| format!("{:?}", result));
                    emit(PluginAction::Complete, PluginOutcome::Success, "route proposed");
                    Ok(ToolResult {
                        success: true,
                        output,
                        error: None,
                    })
                }
                Err(e) => {
                    emit(PluginAction::Fail, PluginOutcome::Failure, &format!("route error: {e}"));
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    })
                }
            }
        }
    }

    fn make_client(config: &ConfigSection) -> crate::router::ArcClient {
        use zeroclaw_solana_core::types::ConfigSection as _;
        crate::router::client_from_config(config)
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str) {
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "arc_pay_router::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs: None,
                message: message.to_string(),
            },
        );
    }

    export!(ArcPayRouter);
}
