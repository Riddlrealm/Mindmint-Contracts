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

closes #51
