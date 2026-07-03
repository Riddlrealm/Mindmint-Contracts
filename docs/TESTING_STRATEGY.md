# Testing strategy

## Layers

1. **Unit tests** — one `Env` per test, in-crate.
2. **Integration tests** — `cross_contract` exercises multiple crates.
3. **Property tests** — `proptest`-style fuzz for select crates.

## Targets

- ≥ 80% line coverage per contract (see ADR-0009).
- Every negative case is asserted (panic with the documented Error).
