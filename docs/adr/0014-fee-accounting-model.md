# ADR-0014: Fee accounting model

## Status
Accepted.

## Decision
Each fee-bearing operation debits the caller up-front and refunds on `panic`. Per-method fee tables are version-locked.

## Consequences
- Predictable user cost.
- Refund logic must be covered by tests.
