# ADR-0021: Storage migration strategy

## Status
Accepted.

## Decision
When a contract changes its `DataKey` layout, a `migrator` module upgrades the storage in-place. Migrations are dry-run pre-deployed to testnet for ≥ 7 days.

## Consequences
- Users don't lose state across upgrades.
- Migration code is security-critical.
