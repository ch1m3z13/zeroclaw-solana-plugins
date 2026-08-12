# plugins/arc-pay-router/README.md

## arc-pay-router

ZeroClaw WIT tool plugin: multi-chain payment routing via Arc's `unifiedBalance`
+ `quoteRoute`, with token-risk gating on Solana legs.

## What it does

Given a source address, a destination address, and a route quote (chain pair,
token pair, amount, optimization target), this plugin:

1. Checks the Solana input-mint risk using the same `token-risk-check` signals
   already used by `solana-pay-request` (mint authority, freeze authority,
   extensions, holder concentration, LP status).
2. Queries Arc's API for ranked candidate routes.
3. Returns the best-ranked route as a `RouteResult::Ok { routes: [..] }`.

**Custody is T1** — the plugin returns an unsigned route proposal only.
Execution stays with the host wallet or a Squads multisig ("agent proposes,
multisig disposes").

## Config keys

| Key | Default | Meaning |
|-----|---------|---------|
| `arc_rpc_url` | `https://arc.zeroclaw.io` | Arc RPC endpoint |
| `max_holders_pct` | `50` | Top-holder threshold (forwarded to risk engine) |
| `max_transfer_fee_pct` | `5` | Transfer-fee threshold (forwarded) |
| `min_tvl_usd` | `10000` | Minimum LP TVL (forwarded) |

## Build and test

```bash
cd plugins/arc-pay-router
cargo test --offline
rustup target add wasm32-wasip2
cargo build --target wasm32-wasip2 --release
```

## Threat model

- **Fail-closed**: any Arc or RPC error returns `RouteResult::Error` — no route
  is fabricated.
- **Risk gates Solana legs**: a Red risk score blocks the route before it is
  ever returned.
- **T2 blocked by code**: `build_route` returns `Error` if `propose_only` is
  not `true`; there is no parameter to skip this check.
- **Non-Solana legs**: unknown tokens are not risk-assessed by this plugin; the
  host should add an independent check before executing.

## Custody

| Tier | This plugin | Why safe |
|------|-------------|----------|
| T1 | `arc-pay-router` | Returns an unsigned route proposal only. No keys, no signing. |
