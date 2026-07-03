# Testing

The workspace uses Cargo's standard test runner plus Soroban's `Env` for contract-side tests.

## Running tests

```bash
cargo test --workspace                                       # everything
cargo test --package puzzle_verification                     # one crate
cargo test --package puzzle_verification -- --nocapture      # verbose
```

## Conventions

- One `Env` per test, registered in a `setup()` helper.
- Reuse helpers from `testutils` (`#[cfg(test)]`) where available.
- Negative cases assert the contract panics with the documented `Error::*` variant.

## Coverage

Run `cargo llvm-cov --workspace` if you have `cargo-llvm-cov` installed.
