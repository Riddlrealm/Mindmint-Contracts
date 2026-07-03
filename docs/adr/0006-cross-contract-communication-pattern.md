# ADR-0006: Cross-contract communication pattern

## Status

Accepted.

## Decision

Contracts call each other directly via Soroban's cross-contract API. No off-chain relays required for trust-minimised composition.

## Consequences

- Strong atomicity guarantees.
- Higher per-call storage cost vs. message-passing.
