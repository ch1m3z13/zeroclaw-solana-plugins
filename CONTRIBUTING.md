# Contributing

Thanks for your interest in ZeroClaw Solana Plugins. This project is open source
under the MIT license and welcomes contributions that improve safe, censorship-
resistant Solana payments and developer tooling.

## Getting started

```bash
# Host tests (no wasm toolchain required)
cd core && cargo test --offline
cd plugins/token-risk-check && cargo test --offline
cd plugins/solana-pay-request && cargo test --offline
cd plugins/payment-watch && cargo test --offline
cd plugins/arc-pay-router && cargo test --offline

# Build WASM components (requires the wasm32-wasip2 target)
rustup target add wasm32-wasip2
cd plugins/<plugin> && cargo build --target wasm32-wasip2 --release
```

## Architecture constraints

- **Pure core, thin wasm shim.** All logic lives in `core/` (no wasm deps) and in
  each plugin's `src/<domain>.rs`. `src/lib.rs` is a thin
  `#[cfg(target_family = "wasm")]` shim only.
- **Host-run tests, mock RPC.** Tests must run without a wasm toolchain or live
  network. Inject `MockTransport` via `RpcClient::with_transport`.
- **Fail-closed.** Any RPC or decode error returns red/error, never a fabricated
  success. No skip/override parameter exists anywhere.
- **Custody stays T1.** Plugins propose (unsigned) only. Signing/execution is the
  host wallet's or a Squads multisig's job — never the agent's.

## Before opening a PR

1. `cargo test --offline` is green in `core/` and the affected plugin(s).
2. `rustfmt` clean: `cargo fmt --check` (or `rustfmt src/**/*.rs`).
3. No new `unsafe`, no new network deps in `core/` or plugin logic crates.
4. Update the relevant `README.md` + `manifest.toml` if behavior changes.

## Reporting security issues

See [SECURITY.md](SECURITY.md). Do not open public issues for vulnerabilities.
