# ADR-0027: Rate-limit algorithm choice

## Status
Accepted.

## Decision
Per-principal rate limiting uses a fixed-window counter (cost: O(1) storage per principal, O(1) computation per call). Quotas are admin-configurable.

## Consequences
- Predictable cost.
- Burst tolerance at window edges.
