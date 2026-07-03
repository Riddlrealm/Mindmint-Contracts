# ADR-0007: Pause and upgrade strategy

## Status

Accepted.

## Decision

Each high-value contract supports a `set_paused` admin toggle. Upgrades deploy new wasm alongside existing storage and migrate via a versioned migrator module.

## Consequences

- Ability to halt bad behaviour.
- Migration scripts must be maintained.
