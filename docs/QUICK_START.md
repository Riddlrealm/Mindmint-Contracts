# Quick start

Get a contract building in under five minutes.

```bash
git clone https://github.com/Riddlrealm/Mindmint-Contracts.git
cd Mindmint-Contracts
cargo build --target wasm32-unknown-unknown --release --package <contract-name>
```

Replace `<contract-name>` with any crate under `contracts/` (e.g. `puzzle_verification`, `achievement_nft`, `reward_token`).

## Run tests

```bash
cargo test --workspace
cargo test --package <contract-name> -- --nocapture
```

## Next steps

- `docs/DEPLOYMENT.md`            — testnet & mainnet deploys
- `docs/CONTRACT_REFERENCE.md`    — what each contract does
- `docs/ARCHITECTURE.md`          — how the contracts fit together
