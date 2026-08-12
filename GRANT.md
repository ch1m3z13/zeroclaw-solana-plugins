# Grant Application — Solana Foundation Nigeria Grants

**Program:** Solana Foundation Nigeria Grants (Superteam Nigeria)
**Amount requested:** $3,000 USDG
**Applicant:** ch1m3z13 (Nigeria)
**Repo:** ZeroClaw Solana Plugins (this repository)
**License:** MIT (open source)

---

## 1. Problem

AI agents are increasingly being given the ability to move money on Solana. The
default posture is unsafe:

- An agent can be prompt-injected into sending funds to a scam token or drained wallet.
- Most "payment" plugins either skip risk checks or hardcode allowlists that go stale.
- Custody is often conflated — the agent that *proposes* a payment is also the one that
  *signs* it, which is a single point of catastrophic failure.

## 2. Solution

A set of **open-source WASM tool plugins** for the ZeroClaw agent runtime that make
Solana payments safe-by-default:

- **Risk-gated every time.** `token-risk-check` scores any SPL token green/amber/red from
  live on-chain data. `solana-pay-request` hard-fails closed if the token is red — there is
  no skip parameter to abuse.
- **Custody stays T1.** Plugins return *unsigned proposals* (a `solana:` URL or a route
  blueprint). A human or a Squads multisig executes. The agent never holds signing keys.
- **Multi-chain by design.** `arc-pay-router` extends the same safety loop to cross-chain
  routing (Solana ↔ EVM) via Arc's unified balance + quote API, still gated and still T1.
- **Fail-closed, testable.** 66 host-run tests (mock RPC, no network) prove the safety
  properties. Every RPC/decode error surfaces as red/error, never a fabricated success.

This directly serves two grant focus areas: **Payments** (Solana Pay P2P + commerce) and
**Developer Tooling** (drop-in safety for any agent building on Solana), and advances
**censorship resistance** by removing single-signer custody.

## 3. Proof of work (already done)

- ✅ 4 WASM plugins, all building for `wasm32-wasip2`
- ✅ 66 passing host tests across `core/` + 4 plugins (`cargo test --offline` green)
- ✅ Live mainnet validation: `token-risk-check` returns USDC → green against real RPC
- ✅ Vendored, ABI-compatible WIT (`wit/v0/`, pinned to upstream `UPSTREAM_REF`)
- ✅ Thin wasm shim / pure core architecture (zero wasm deps in `core/`)
- ✅ CI workflow running host tests on every push
- ✅ `CONTRIBUTING.md`, `SECURITY.md`, threat models per plugin

## 4. What the $3k funds

The suite is largely built. The grant funds **hardening + a real deployable demo + the
multi-chain finish**:

| Milestone | Deliverable | USDG |
|-----------|-------------|------|
| M1 — Public launch | Repo public, CI green, README + threat models published | 750 (25%) |
| M2 — Live demo | Recorded end-to-end Solana Pay flow (risk gate → QR → pay → watch) on devnet/mainnet | 750 |
| M3 — Multi-chain finish | `arc-pay-router` wired to a live Arc endpoint; cross-chain route proposal demo | 750 |
| M4 — Adoption + docs | Squads-multisig execute path documented; 1 onboarding guide; community update | 750 |

Payout follows the program's 25% / 75% structure: **$750 upfront on approval**, the rest
released as milestones M2–M4 land with weekly community progress updates.

## 5. Open-source commitment

MIT licensed from day one. All code is developed in the open in this repository. No
proprietary binaries, no telemetry, no closed components. The grant funds *public*
infrastructure for the Solana agent ecosystem.

## 6. Why us / why now

We already missed the original ZeroClaw bounty deadline — the engineering is done, the
*packaging* is what was missing. Repackaging for this grant turns finished, tested work
into public infrastructure Nigeria's (and the world's) agent builders can drop in today.
Speed > perfection: the core is proven; the grant makes it real and reachable.

## 7. Links

- Grant listing: https://superteam.fun/earn/grants/solana-foundation-nigeria-grants/
- Repo: (this repository)
- ZeroClaw runtime: https://github.com/zeroclaw-labs/zeroclaw
