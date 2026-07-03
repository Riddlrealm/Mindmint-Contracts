# ADR-0017: Error code stability policy

## Status
Accepted.

## Decision
`Error` enum discriminant values are stable across MINOR versions. Adding variants is allowed; changing existing variants' discriminant is a MAJOR-version breaking change.

## Consequences
- Caller code remains forward-compatible.
- Audit costs are predictable across releases.
