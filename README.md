# ZeroClaw Solana Plugins

**Open-source WASM tool plugins that make Solana payments safe-by-default for AI agents.**
Built for the [Solana Foundation Nigeria Grants](https://superteam.fun/earn/grants/solana-foundation-nigeria-grants/)
(Superteam Nigeria) — advancing **Payments** (Solana Pay P2P + commerce) and
**Developer Tooling** for a more censorship-resistant Solana.

> Four WASM tool plugins for the [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)
> AI agent runtime. Pure-Rust core, zero wasm dependencies, host-testable with mock RPC,
> fail-closed, and custody-aware (T1: propose-only, never sign).

## Why this matters

AI agents are starting to move money. The default is dangerous: an agent that can
sign transactions can be prompted into sending funds to a scam token, a drained wallet,
or a malicious contract. This project is a **payment safety layer** that sits between
the agent and the chain:

- **Every payment is risk-gated.** Tokens are scored green/amber/red from on-chain data
  (mint authority, freeze authority, Token-2022 extensions, holder concentration, LP status).
- **The agent never holds the keys.** Plugins return *unsigned proposals* only. A human or a
  Squads multisig disposes. Custody stays T1 — "the agent proposes, the multisig disposes."
- **Fail-closed.** RPC errors, unknown tokens, and red risk scores all surface as errors —
  never a fabricated success. No "skip check" parameter exists anywhere (prompt-injection safe).

## The four plugins

| Plugin | Tier | What it does |
|--------|------|-------------|
| [`token-risk-check`](plugins/token-risk-check/) | T0 | Check SPL token risk from on-chain data |
| [`solana-pay-request`](plugins/solana-pay-request/) | T1 | Generate Solana Pay URLs, gated on risk |
| [`payment-watch`](plugins/payment-watch/) | T0 | Watch an address for expected payments |
| [`arc-pay-router`](plugins/arc-pay-router/) | T1 | Propose multi-chain routes via Arc (Solana ↔ EVM) |

## The combined pitch: a payment terminal that can't be scammed

1. DM your agent *"charge table 4 for 25 USDC"*
2. `solana-pay-request` calls `token-risk-check` on USDC → green
3. A QR code appears in chat with a `solana:` URL
4. Customer scans → their wallet builds the tx with a fresh blockhash → pays
5. `payment-watch` detects the payment → agent confirms: *"Invoice #412 paid"*

`arc-pay-router` extends this to multi-chain: route SOL on Solana to USDC on Ethereum
through Arc's unified balance + quote API, still gated by the same risk engine on the
Solana leg and still T1 (propose-only).

## Architecture

```
core/                    # Pure Rust, no wasm deps
├── rpc.rs              # JSON-RPC over pluggable transport (waki on wasm, mock on host)
├── mint.rs             # SPL Token + Token-2022 TLV decoding
├── risk.rs             # Risk scoring engine (green/amber/red)
├── pay.rs              # Solana Pay URL construction
├── tx.rs               # Transaction assembly
├── encoding.rs         # bs58/base64 helpers
├── accounts.rs         # ATA derivation + pubkey validation
├── arc.rs              # Arc multi-chain client (new in this grant cycle)
└── types.rs            # Shared types

plugins/
├── token-risk-check/   # T0 — reads on-chain data, returns risk
├── solana-pay-request/ # T1 — generates solana: URL, gates on risk
├── payment-watch/      # T0 — polls for payment confirmation
└── arc-pay-router/     # T1 — multi-chain route proposal via Arc
```

Each plugin follows the `redact-text` reference layout:
```
src/[domain].rs    # pure logic, no wasm deps — host-testable
src/lib.rs         # thin #[cfg(target_family = "wasm")] shim
tests/             # host-run integration tests
manifest.toml      # name, version, wasm_path, capabilities, permissions
README.md          # what it does, config keys, custody tier, threat model
```

## Build & test

```bash
# Host tests — no wasm toolchain or network needed
cd core && cargo test --offline
cd plugins/token-risk-check && cargo test --offline
cd plugins/solana-pay-request && cargo test --offline
cd plugins/payment-watch && cargo test --offline
cd plugins/arc-pay-router && cargo test --offline

# WASM components
rustup target add wasm32-wasip2
cd plugins/<plugin> && cargo build --target wasm32-wasip2 --release
```

CI runs the host tests on every push (see `.github/workflows/ci.yml`).

## Custody design

| Tier | Plugins | What | Why safe |
|------|---------|------|----------|
| **T0** | token-risk-check, payment-watch | Read-only | No keys, no signing, no fund movement |
| **T1** | solana-pay-request, arc-pay-router | Build URL / propose route | Returns unsigned artifact only — wallet/multisig signs |

T2 (sign and submit) is intentionally not implemented. The best pattern is
"the agent proposes, a Squads multisig disposes."

## Open source

MIT licensed. This project is being developed in the open as part of the
Solana Foundation Nigeria Grants program. See [GRANT.md](GRANT.md) for the
milestone plan and funding use.

## What fought us on wasm32-wasip2

- `solana-sdk` / `solana-client` won't compile for wasm32-wasip2 inside a WIT component.
- Used `waki` (blocking `wasi:http`) + `serde_json` + `bs58` instead. No `solana-sdk`
  dependency at all — mint decoding and transaction assembly are hand-rolled from raw
  on-chain bytes.
- Token-2022 TLV parsing hand-rolled (spl-token-2022 won't compile clean for wasm).
- `RpcClient::new` is wasm-only (uses `waki`); host tests inject `MockTransport` via
  `RpcClient::with_transport`.

## License

MIT
