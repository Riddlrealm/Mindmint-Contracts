# ADR-0024: Access control default deny

## Status
Accepted.

## Decision
No method allows access by default. Every entry point checks `require_auth()` and an explicit role check before mutating state.

## Consequences
- Zero-trust baseline.
- Slightly more verbose entry points.
