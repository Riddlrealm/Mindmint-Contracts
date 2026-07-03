# Rust client usage

```rust
use soroban_sdk::{Env, Address, Symbol};
let env = Env::default();
let id: Address = env.register(contract::WASM, ());
let res: i128 = env.invoke_contract(&id, &Symbol::new(&env, "get_balance"), ...);
```
