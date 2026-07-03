# Deployment

End-to-end deploy to the Soroban testnet.

## Prerequisites

- Rust toolchain (see `rust-toolchain.toml`)
- `cargo install --locked soroban-cli --version 21.0.0`
- A funded testnet identity (`soroban keys fund <identity> --network testnet`)

## Steps

1. **Build** — `cargo build --target wasm32-unknown-unknown --release --package <crate>`
2. **Optimise** — `soroban contract optimize --wasm target/wasm32-unknown-unknown/release/<crate>.wasm`
3. **Deploy** — `soroban contract deploy --wasm <crate>.optimized.wasm --source <identity> --network testnet`
4. **Initialise** — `soroban contract invoke --id <CONTRACT_ID> --source <identity> --network testnet -- initialize ...`

## Mainnet

Same pattern, `--network mainnet`. Audit before going live.

## See also

- `scripts/verify-build.sh` — smoke check across every workspace member
