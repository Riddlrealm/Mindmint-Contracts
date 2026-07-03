# TypeScript type generation

Re-generate client bindings from a contract's `lib.rs` via:

```bash
soroban contract bindings typescript --wasm target/...wasm --output ./bindings
```

Commit the generated tree under `bindings/<crate>/`.
