# ADR-0009: Test coverage targets

## Status

Accepted.

## Decision

Target ≥ 80% line coverage on every contract. CI uses `cargo llvm-cov` and fails if a crate drops below.

## Consequences

- Stable quality bar.
- Coverage is not a correctness proof; we still audit.
