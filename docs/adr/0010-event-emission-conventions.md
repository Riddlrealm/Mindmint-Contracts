# ADR-0010: Event emission conventions

## Status

Accepted.

## Decision

Emit a typed event for every state change. Topic names are namespaced as `<crate>::<event>`.

## Consequences

- Off-chain indexers can subscribe cleanly.
- Slightly larger ledger footprint.
