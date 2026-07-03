# Capacity planning

How we plan capacity for new features and growth.

## Inputs

- Expected TPS per contract
- Per-account storage footprint
- Event emission volume
- Cross-contract call fan-out

## Approach

1. Estimate per-feature load.
2. Profile under 1x, 2x, 5x load.
3. Compute RPC, network, and storage headroom.
4. Compare against `docs/SLO_DEFINITIONS.md`.
