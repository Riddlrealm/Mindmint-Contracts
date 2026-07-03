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
