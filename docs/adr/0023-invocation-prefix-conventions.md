# ADR-0023: Invocation prefix conventions

## Status
Accepted.

## Decision
Top-level admin methods are prefixed `admin_*`; helper methods stay unprefixed. SDK-side, methods are coarse-grained (one method per intent).

## Consequences
- Easier to discover admin surface.
- Documentation is consistent.
