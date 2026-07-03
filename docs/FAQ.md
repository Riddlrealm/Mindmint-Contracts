# FAQ

## How do I add a new contract?

1. Create `contracts/<name>/` with `Cargo.toml` and `src/lib.rs`.
2. Add `contracts/<name>` to root `Cargo.toml`'s `[workspace] members`.
3. Add a `README.md` to `contracts/<name>/`.
4. Run `cargo check --workspace --all-targets` to confirm.

## How do I rename a contract?

- Rename the directory and update the package name in its `Cargo.toml`.
- Update root `Cargo.toml`'s `[workspace] members`.
- Update any cross-contract references in `cross_contract`.

## Where do I find the deployed addresses?

After a successful deploy, the CLI prints the contract ID. Use `soroban contract inspect --id <CONTRACT_ID> --network testnet` to dump its metadata.

## Why are some tests under `cross_contract`?

`cross_contract` is the integration test harness; its tests span multiple crates and exercise the cross-contract invocation patterns.

## How do I pause or unpause a contract?

Call the contract's `set_paused` (or equivalent) method as the admin. See `docs/SECURITY_MODEL.md` for the broader picture.
