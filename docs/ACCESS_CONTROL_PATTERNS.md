# Access control patterns

Three patterns we use:

## Simple admin

Single address; admin operations behind `set_admin`-driven checks.

## Per-role RBAC

Roles stored on contract; `grant_role` / `revoke_role` admin-only. See the `rbac` crate.

## Multisig

Critical operations (treasury, upgrades) require N-of-M signatures. See `multisig_treasury`, `multisig_escrow`.

A `pause` toggle wraps all three.
