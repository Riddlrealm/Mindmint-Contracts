# ADR-0012: Pause implementation pattern

## Status
Accepted.

## Decision
Each high-value contract exports a `set_paused(paused: bool)` admin-only method. The contract panics with `ContractPaused` on every state-changing entry when paused.

## Consequences
- Uniform pause behaviour.
- Off-chain indexers must respect the `ContractPaused` event.
