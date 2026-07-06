# Security model

Defense in depth across the workspace.

## Authentication & authorisation

- Admin-only operations use `require_auth()` against a stored admin address.
- Per-user operations call `require_auth()` on the user.
- Higher-value operations live behind multisig / timelock wrappers (`multisig_*`, `timelock_vault`).

## Pause / emergency

- `emergency_pause` provides a cross-contract kill switch.
- Per-contract pause toggles live in admin storage when relevant.

## Reentrancy & arithmetic

- All arithmetic is checked where overflow could happen.
- External calls are sequenced after state mutation.

## Reporting

See `SECURITY.md` at the repository root for responsible-disclosure contact details.

## Static analysis gaps

CI's `clippy` job (`.github/workflows/clippy.yml`) denies only the
`clippy::correctness` category. The wider `suspicious` and
`complexity` families are advisory at the workflow level. CI is
therefore a compile-time sanity check, NOT a security linter.
The failure modes clippy cannot see, which reviewers therefore
must catch, are:

- Missing event emission for state transitions (see `EVENT_DECODING.md`).
- Oracle manipulation: stale-oracle reads, single-source trust.
- Storage-key collisions: shared `DataKey` arms across contracts
  or per-contract keyed maps leaking between contracts.
- Panic-as-control-flow: relying on `panic!` for normal-path
  branching instead of a `Result` / typed error.

Items intentionally omitted from this list because they're
covered in the sections above: authentication (`require_auth`) in
"Authentication & authorisation", reentrancy in "Reentrancy &
arithmetic", unchecked overflow in "Reentrancy & arithmetic" +
the `checked_*` rule in `docs/SECURE_CODING_GUIDELINES.md`. The
latter is the explicitly-named enforcement: checked math is
required because clippy cannot see unchecked overflow.
