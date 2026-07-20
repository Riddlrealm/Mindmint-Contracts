# Admin permissions matrix

Authorization audit of state-changing entry points, per ADR-0018 (authn on every
state-changing entry) and ADR-0024 (default-deny). Verification is enforced by
`scripts/check-missing-permissions.sh` (per-function AST check, issue #19).

## `guild` (issue #19)

All 13 `pub` entry points already called `require_auth()`; the gaps were one
layer deeper — the role/membership authorization check. Columns: `require_auth`
(caller authenticated), `Role gate` (role assertion, if any), `assert_active`
(rejects a disbanded guild), `Membership` (caller must hold a role).

| Entry point | require_auth | Role gate | assert_active | Membership | Status |
|---|---|---|---|---|---|
| `initialize` | ✅ leader | n/a (bootstraps) | n/a | n/a | ✅ re-init guarded |
| `join` | ✅ user | open by design | ✅ | n/a | ✅ |
| `set_role` | ✅ leader | ✅ `assert_leader` | ✅ | — | ✅ |
| `deposit` | ✅ member | — | ✅ | ✅ **added** | ✅ fixed |
| `withdraw` | ✅ officer | ✅ `assert_officer_or_leader` | ✅ | — | ✅ |
| `vote_withdrawal` | ✅ member | — | ✅ | ✅ `assert_member` | ✅ (helper) |
| `execute_withdrawal` | ✅ executor | — | ✅ | ✅ **added** | ✅ fixed |
| `add_resource` | ✅ officer | ✅ `assert_officer_or_leader` | ✅ | — | ✅ |
| `add_achievement` | ✅ officer | ✅ `assert_officer_or_leader` | ✅ | — | ✅ |
| `create_proposal` | ✅ officer | ✅ `assert_officer_or_leader` | ✅ | — | ✅ |
| `vote` | ✅ member | — | ✅ | ✅ `assert_member` | ✅ (helper) |
| `record_competition` | ✅ leader | ✅ `assert_leader` | ✅ | — | ✅ |
| `disband` | ✅ leader | ✅ `assert_leader` | ✅ **added** | — | ✅ hardened |

### Gaps closed in this audit

1. **`execute_withdrawal` — missing membership gate.** Previously any
   authenticated address could trigger execution of an approved withdrawal.
   Funds go to `proposal.officer` and a vote quorum is already required, so the
   severity is low, but execution is now restricted to members
   (`assert_member`). It gates on **membership, not officer**: the authority
   already lives in the vote quorum; execution is ministerial, and requiring an
   officer would strand guilds whose officer is absent. A positive regression
   test locks in that a plain `Member` can still execute an approved withdrawal.

2. **`deposit` — no membership check (behavior change).** The parameter is named
   `member`, and voting already requires membership, but any address could
   deposit into the treasury. Now gated with `assert_member`. This is an
   intentional, tested behavior change (non-members can no longer deposit),
   reducing dusting / accounting-confusion surface.

3. **`disband` — consistency hardening.** `disband` already rejected a second
   call via an inline `"Already disbanded"` check, so it was not re-invocable.
   It now uses the canonical `assert_active` gate instead, so a call on an
   already-disbanded guild fails with the same `"Guild disbanded"` message as
   every other mutator. Consistency/hardening, not a vulnerability fix.

### Methods open by design

- `join` — anyone may join; membership is intentionally open at the role layer.
- `vote` / `vote_withdrawal` — any member may vote; gated on membership, not on
  officer/leader. Governance power lives in the vote counts, not in who tallies.

### Methodology

Manual audit of all 13 guild entry points plus a per-function AST linter
(`tools/check-permissions`, enforcing `guild`) replacing the previous per-file
`grep`, which green-lit a whole contract on a single `require_auth` match.
Per-role *semantic* assertion across the whole workspace — and the ~140
pre-existing candidate functions a workspace-wide run surfaced in other
contracts — is separate follow-up work beyond the scope of #19.
