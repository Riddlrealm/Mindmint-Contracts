# Fix #51 — Replace placeholder security handles with concrete owners

## Why it matters
Real handle = real triage; placeholder = silent failures. The previous
`SECURITY.md` and `CODEOWNERS` used a placeholder email (`security@mindmint.example`)
and a placeholder owner (`@Riddlrealm`). Reports sent to these addresses are
never received, so vulnerabilities are discovered late or never.

## Changes
### `SECURITY.md`
- Replaced the placeholder inbox `security@mindmint.example` with the concrete
  `security@mindmint.io` address.
- Added an explicit encouragement to use a dedicated `security@` address so
  reports always reach the right team (placeholder/non-routed addresses cause
  silent failures).
- Added a **Response Timeline** section that links to the existing
  `docs/SECURITY_RESPONSE_TIMELINE.md` (SEV1–SEV3 acknowledge/triage/mitigation
  /post-mortem targets), satisfying the requirement to link that doc.

### `CODEOWNERS`
- Replaced the `@Riddlrealm` placeholder with the real maintainer handle
  `@Phantomcall` across all owned paths.
- Added mandated ownership lines for `/SECURITY.md` and `/CODEOWNERS` so the
  security policy and ownership file are themselves owned by the maintainer
  team, matching the "mandate teams" acceptance criterion.
- Removed the stale "replace the placeholder" note.

## Acceptance criteria checklist
- [x] `SECURITY.md` updated.
- [x] `docs/SECURITY_RESPONSE_TIMELINE.md` linked from `SECURITY.md`.
- [x] `CODEOWNERS` reflects mandate teams (real handle, security files owned).
- [x] Concrete handle and encouraged `security@` email address.

## Labels
`area:security`, `kind:docs`, `priority:P1`

## CI status — known pre-existing infra break (unrelated to this PR)
The `CI / build` and `clippy / clippy` workflows are RED for **all** workspace
pull requests, including this docs-only one. This is a pre-existing, repo-wide
dependency break in the test-only `soroban-env-host 21.2.1` harness pulled by
`soroban-sdk 21.7.7`: `ed25519-dalek 3.0.0` requires `rand_core 0.10` while its
`ChaCha20Rng` (via `rand 0.8.7`) implements `rand_core 0.6.4`, so
`cargo check --workspace --all-targets` / `cargo clippy --workspace --all-targets`
fail with `error[E0277]: ChaCha20Rng: ed25519_dalek::rand_core::CryptoRng is not satisfied`.
`Cargo.lock` is gitignored, so CI regenerates the broken resolution every run.
The changes in this PR are correct (docs / `CODEOWNERS` only). Separate follow-up:
bump `soroban-sdk` or commit a `Cargo.lock` pinning the `rand_core 0.6`-compatible
crypto stack.

closes #51
