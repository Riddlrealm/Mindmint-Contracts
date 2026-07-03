# tests/

This directory is intended for workspace-level integration tests and example invocations. To make a test crate pick up these files, add `tests/` to an existing crate or create a new one and reference it from its `Cargo.toml`.

## Conventions

- One test file per contract integration scenario.
- Use Soroban's `Env` for any contract-side assertions.
- Keep tests hermetic — no network calls, no fixed timestamps unless documented.
