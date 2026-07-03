# ADR-0011: Storage layout conventions

## Status
Accepted.

## Decision
Storage keys are namespaced under `DataKey::Storage*` enums. Each variant documents the type of value it stores and whether it's per-instance or global.

## Consequences
- Easier to audit storage usage.
- Migrators are straightforward to write.
