# Troubleshooting

## Build fails with "linking with `rust-lld` failed"

Make sure the WASM target is installed:

```bash
rustup target add wasm32-unknown-unknown
```

## Tests fail with "Contract instance not initialised"

Each test must call the contract's `initialize` (or equivalent setup) in a `setup()` helper. Look at existing tests in the same crate for the pattern.

## Deploy command times out

Verify `SOROBAN_RPC_URL` is reachable and that your identity has been funded:

```bash
soroban keys fund <identity> --network testnet
```

## Cargo workspace complains about a missing member

If a crate under `contracts/` was added but not listed in `[workspace] members`, add it to the root `Cargo.toml`.

## Paused contract refuses admin ops

Check `is_paused()` and call `set_paused(false)` from the admin to resume.
