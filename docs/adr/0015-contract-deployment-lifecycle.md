# ADR-0015: Contract deployment lifecycle

## Status
Accepted.

## Decision
A contract moves through `authored → built → optimized → deployed → initialised → paused ⋯ running`. CI enforces that every deploy lands an init on testnet within 24 hours.

## Consequences
- Less drift between build artefacts and on-chain state.
