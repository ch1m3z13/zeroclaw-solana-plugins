# Security Policy

ZeroClaw Solana Plugins are a payment-safety toolkit. Security bugs — especially
anything that could let an attacker bypass the risk gate, forge a payment, or move
funds without explicit custody — are taken seriously.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security problems.

Instead, email the maintainer or open a private security advisory on GitHub.
Include:

- Affected plugin(s) and version/commit
- Steps to reproduce
- Expected vs. actual behavior
- Any known exploit impact

We aim to acknowledge within 72 hours and propose a fix timeline within 7 days.

## Threat model (summary)

| Vector | Mitigation |
|--------|------------|
| Prompt injection | No plugin accepts a "skip check" / "return safe" parameter. Risk results are deterministic from on-chain data. |
| RPC / bridge manipulation | Fail-closed: errors surface as red/error, never green. |
| Unauthorized signing | Custody is T1. Plugins return unsigned proposals only; execution requires the host wallet or a Squads multisig. |
| Supply-chain | Minimal deps; `core/` has zero wasm/network dependencies; `solana-sdk` is intentionally excluded. |

See each plugin's `README.md` for its full threat model.
