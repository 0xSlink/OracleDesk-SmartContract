# Security Policy

## Status: testnet-only, unaudited

Everything in this repository is deployed to Stellar **testnet only**. The
contracts have **not** been professionally audited. Do not deploy them to
mainnet, and do not send anything of real value to any address or contract
in this repo. See `docs/STATUS.md` for the current list of what has and has
not been verified.

## Reporting a vulnerability

Please report security issues privately, using
[GitHub Security Advisories](../../security/advisories/new) for this
repository, rather than opening a public issue. This applies to:

- A way to drain, lock, or misdirect funds held by any of the four
  contracts (`market-core`, `resolver`, `treasury`, `reasoning-registry`).
- An authorization bypass (a privileged function callable by the wrong
  party — see `contracts/*/src/lib.rs`'s wrong-caller tests for what's
  already covered).
- A way to make the resolution-commitment scheme (ADR 0001) or the treasury
  cross-contract auth flow (ADR 0002) behave differently from documented.
- A vulnerability in the x402 payment/verification flow that would let a
  trace be served without payment, or without its hash matching the
  on-chain record.

<!-- TODO(maintainer): confirm an expected response/triage timeline before
publishing this repo — do not commit to one without checking. -->

## Scope

In scope: `contracts/`, `x402/`, `agents/`, `packages/bindings/` (the
generation script and its templates, not third-party generated code
itself), and the deployment scripts in `scripts/`.

Out of scope: the separate frontend repository, and any third-party
dependency's own vulnerabilities (report those upstream — `npm audit` /
`cargo audit` findings in a dependency are not this repo's issues unless
this repo's usage of them is what's exploitable).

## Guardian trust model

The resolver contract defines guardian powers as emergency interventions rather
than day-to-day administrative authority. To avoid single-party rug pulls or
arbitrary cancellations:

- **Time-restricted actions**: `cancel_proposal` is restricted to active dispute
  windows only. `guardian_void_market` enforces strict grace periods after
  `close_time` (`REVEAL_GRACE_SECS` = 3 days, `ORACLE_GRACE_SECS` = 1 day,
  `SIGNER_GRACE_SECS` = 7 days) before a guardian void can be executed.
- **Cancellation limits**: A market allows a maximum of 2 guardian cancellations
  (`MAX_CANCELS_PER_MARKET`) to prevent indefinite griefing.
- **Mandatory reason codes and public audit trail**: Every guardian action emits
  a `GuardianAction` event recording a typed `GuardianReason` and timestamp.
