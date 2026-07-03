# ADR-0005: Error handling conventions

## Status

Accepted.

## Decision

Every contract exports a single `Error` enum with `#[contracterror]`. Variants are documented inline.

## Consequences

- Stable error ABI.
- Caller code is straightforward to write.
