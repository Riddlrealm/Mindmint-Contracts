# ADR-0026: External service resilience

## Status
Accepted.

## Decision
Cross-contract calls to non-critical external contracts have explicit timeouts via the wrapper contract. Critical paths fail-closed.

## Consequences
- No silent hangs.
- Wrapper contract surface area grows.
