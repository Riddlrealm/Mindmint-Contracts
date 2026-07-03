# ADR-0019: Cross-contract deployment strategy

## Status
Accepted.

## Decision
Cross-contract dependencies are pinned by WASM hash in deploy scripts, never by symbol. Upgrades require a coordinated script that updates every dependent binding atomically.

## Consequences
- Atomic upgrade story.
- More complex release scripts.
