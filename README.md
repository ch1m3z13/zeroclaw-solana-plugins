# Zeroclaw Solana Plugins

Three WASM tool plugins for the [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)
AI agent runtime, enabling safe Solana payments and token intelligence.

**Submission for**: [Build Solana-native plugins for Zeroclaw](https://superteam.fun/earn/listing/zeroclaw/)
**Tracks**: A (Payments) + D (Onchain Intelligence) + E (Shared Core)

## What's included

| Plugin | Tier | Track | What it does |
|--------|------|-------|-------------|
| [`token-risk-check`](plugins/token-risk-check/) | T0 | D | Check SPL token risk: mint authority, freeze authority, Token-2022 extensions, holder concentration, LP status |
| [`solana-pay-request`](plugins/solana-pay-request/) | T1 | A | Generate Solana Pay transfer-request URLs with built-in risk gating |
| [`payment-watch`](plugins/payment-watch/) | T0 | A | Watch an address for expected SPL token payments, report when they land |

All three share a pure-Rust core crate ([`core/`](core/)) that handles RPC, mint decoding,
risk scoring, and URL construction with zero wasm dependencies.

## The combined pitch

**A payment terminal that can't be scammed.**

1. DM your agent *"charge table 4 for 25 USDC"*
2. `solana-pay-request` calls `token-risk-check` on USDC → green
3. QR code appears in chat with a `solana:` URL
4. Customer scans → wallet builds tx with fresh blockhash → pays
5. `payment-watch` detects the payment → agent confirms: "Invoice #412 paid"

The risk-checker is the safety gate. Every payment goes through it. Dynamic risk
scoring beats hardcoded allowlists.

## Architecture

```
core/                    # Pure Rust, no wasm deps
├── rpc.rs              # JSON-RPC over pluggable transport (waki on wasm, mock on host)
├── mint.rs             # SPL Token + Token-2022 TLV decoding
├── risk.rs             # Risk scoring engine (green/amber/red)
├── pay.rs              # Solana Pay URL construction
├── tx.rs               # Legacy transaction assembly
├── encoding.rs         # bs58/base64 helpers
├── accounts.rs         # ATA derivation + pubkey validation
└── types.rs            # Shared types

plugins/
├── token-risk-check/   # T0 — reads on-chain data, returns risk assessment
├── solana-pay-request/ # T1 — generates solana: URL, gates on risk
└── payment-watch/      # T0 — polls for payment confirmation
```

Each plugin follows the `redact-text` reference layout:
```
src/
├── [domain].rs    # pure logic, no wasm deps — host-testable
└── lib.rs         # thin #[cfg(target_family = "wasm")] shim
tests/             # host-run integration tests
manifest.toml      # name, version, wasm_path, capabilities, permissions
README.md          # what it does, config keys, custody tier, threat model
```

## Build

```bash
# Host tests (no wasm toolchain needed)
cd core && cargo test
cd plugins/token-risk-check && cargo test
cd plugins/solana-pay-request && cargo test
cd plugins/payment-watch && cargo test

# WASM components
rustup target add wasm32-wasip2
cd plugins/token-risk-check && cargo build --target wasm32-wasip2 --release
cd plugins/solana-pay-request && cargo build --target wasm32-wasip2 --release
cd plugins/payment-watch && cargo build --target wasm32-wasip2 --release
```

> **WIT note:** this repo vendors a minimal local `wit/v0/tool-plugin.wit` placeholder so
> the components build offline. Vendor the official `zeroclaw-plugins` WIT before opening a PR —
> the runtime's real ABI may differ from this stand-in.

## Custody design

| Tier | Plugins | What | Why safe |
|------|---------|------|----------|
| **T0** | token-risk-check, payment-watch | Read-only | No keys, no signing, no fund movement |
| **T1** | solana-pay-request | Build URL | Returns URL only — payer's wallet builds and signs at scan time |

T2 (sign and submit) is not implemented. The best pattern is "the agent proposes,
a Squads multisig disposes" — the plugin builds the transaction, submits as a
multisig proposal, and a human approves from their phone.

## Threat model

### Prompt injection

Every plugin has been tested against prompt injection attacks. The results are in
each plugin's README. Summary:

- **token-risk-check**: Cannot be told to return a specific risk result. Deterministic
  from on-chain data. No override parameter.
- **solana-pay-request**: Cannot bypass risk check. Hardcoded in `execute()` — no skip
  parameter exists. Fail-closed on RPC errors.
- **payment-watch**: Cannot fabricate payments. Queries real on-chain transaction data.
  Fail-closed on RPC errors.

### Fail-closed principle

All plugins fail to a safe state:
- RPC errors → return error/red, never green/paid
- Invalid input → return error, never partial results
- Unknown tokens → refuse to process, never assume safe

## What fought us on wasm32-wasip2

- `solana-sdk` / `solana-client` won't compile for wasm32-wasip2 inside a WIT component
- Used `waki` (blocking `wasi:http`) + `serde_json` + `bs58` instead. No `solana-sdk`
  dependency at all — mint decoding and transaction assembly are hand-rolled from raw
  on-chain bytes.
- Token-2022 TLV parsing hand-rolled (spl-token-2022 won't compile clean for wasm)
- Transaction assembly uses manual instruction encoding
- Base58/base64 helpers implemented in `core/encoding.rs` (no external dep needed)
- `RpcClient::new` is wasm-only (uses `waki`); host tests inject `MockTransport` via
  `RpcClient::with_transport`

## License

MIT
