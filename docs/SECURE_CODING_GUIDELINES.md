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
