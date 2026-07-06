# Secure coding guidelines

## Inputs

- Validate every input; reject zero or negative amounts even when not strictly required.
- Bound loops; do not iterate over caller-supplied vectors.

## Authorization

- `require_auth()` on every state-changing entry.
- Admin functions behind admin role, never address equality alone.

## Arithmetic

- Use checked math everywhere totals could overflow.
- Avoid float.

## State

- Atomic state changes with token transfers.
- Document invariants in module-level doc comments.

## Static analysis scope

CI's `clippy` job (`.github/workflows/clippy.yml`) denies only
`clippy::correctness` -- soundness-detector members such as
`iterator_step_by_zero` (zero-step iter panics) and
`iter_next_loop` (`while let Some(_) = iter.next()` loss-of-size_hint
/ hang pattern). The wider `suspicious` and `complexity` families
are advisory at the workflow level. The full list of failure modes
clippy cannot catch -- oracle manipulation, storage-key collisions,
panic-as-control-flow, missing event emission, unchecked
arithmetic overflow, and the like -- lives in
`docs/SECURITY_MODEL.md` "Static analysis gaps" so there is one
canonical source. Reviewers should read that section before
approving any contract change.
