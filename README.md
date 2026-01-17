# Quest Service - Smart Contracts

Soroban smart contracts powering Quest Service, a logic puzzle game on the Stellar blockchain. These contracts handle NFT achievements, token rewards, and puzzle completion verification.

## 🔐 Contracts Overview

### Achievement NFT Contract
Mints and manages NFT achievements for puzzle completion milestones.

**Features:**
* Mint achievement NFTs for completed puzzles
* Track player achievements on-chain
* Metadata storage for achievement details
* Transfer and ownership management

### Reward Token Contract
Manages custom token rewards and puzzle unlocks.

**Features:**
* Distribute tokens for puzzle completion
* Handle token spending for hints and special levels
* Balance tracking and transfers
* Integration with XLM for premium features

### Puzzle Verification Contract
Verifies puzzle solutions and triggers rewards.

**Features:**
* On-chain puzzle state validation
* Solution verification logic
* Reward distribution triggers
* Anti-cheat mechanisms

## 🛠️ Tech Stack

* **Language**: Rust
* **Framework**: Soroban SDK
* **Network**: Stellar (Testnet/Mainnet)
* **Testing**: Soroban CLI, Rust tests
* **Deployment**: Soroban CLI

## 📦 Installation

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add wasm target
rustup target add wasm32-unknown-unknown

# Install Soroban CLI
cargo install --locked soroban-cli
```

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/quest-service-contracts.git

# Navigate to the project directory
cd quest-service-contracts

# Build contracts
soroban contract build

# Run tests
cargo test
```

## 🚀 Deployment

### Deploy to Testnet

```bash
# Configure network
soroban network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"

# Generate identity
soroban keys generate deployer --network testnet

# Fund account (get testnet XLM)
soroban keys fund deployer --network testnet

# Deploy contract
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/achievement_nft.wasm \
  --source deployer \
  --network testnet
```

### Deploy to Mainnet

```bash
# Configure mainnet
soroban network add mainnet \
  --rpc-url https://soroban-mainnet.stellar.org \
  --network-passphrase "Public Global Stellar Network ; September 2015"

# Deploy (ensure account is funded)
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/achievement_nft.wasm \
  --source deployer \
  --network mainnet
```

## 📁 Project Structure

```
quest-service-contracts/
├── contracts/
│   ├── achievement_nft/     # NFT achievement contract
│   │   ├── src/
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   ├── reward_token/        # Token reward contract
│   │   ├── src/
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   └── puzzle_verification/ # Puzzle verification contract
│       ├── src/
│       │   └── lib.rs
│       └── Cargo.toml
├── tests/                   # Integration tests
├── scripts/                 # Deployment scripts
├── Cargo.toml
└── README.md
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific contract tests
cargo test --package achievement_nft

# Run with output
cargo test -- --nocapture

# Test with Soroban CLI
soroban contract invoke \
  --id CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- \
  function_name \
  --arg1 value1
```

## 🔧 Contract Functions

### Achievement NFT Contract

```rust
// Mint achievement NFT
fn mint(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32

// Get achievement details
fn get_achievement(env: Env, token_id: u32) -> Achievement

// Transfer achievement
fn transfer(env: Env, from: Address, to: Address, token_id: u32)
```

### Reward Token Contract

```rust
// Distribute rewards
fn distribute_reward(env: Env, to: Address, amount: i128)

// Spend tokens for unlock
fn spend_tokens(env: Env, from: Address, amount: i128, feature_id: u32) -> bool

// Check balance
fn balance(env: Env, account: Address) -> i128
```

### Puzzle Verification Contract

```rust
// Submit solution
fn submit_solution(env: Env, player: Address, puzzle_id: u32, solution: String) -> bool

// Verify and reward
fn verify_and_reward(env: Env, player: Address, puzzle_id: u32) -> bool

// Get puzzle status
fn get_puzzle_status(env: Env, player: Address, puzzle_id: u32) -> PuzzleStatus
```

## 🔒 Security Considerations

* All contracts implement access control for admin functions
* Solutions are hashed before on-chain verification
* Rate limiting to prevent spam
* Comprehensive input validation
* Audited for common vulnerabilities (reentrancy, overflow, etc.)

## 🌟 Integration with Quest Service

These contracts work seamlessly with:
* **Backend API**: Triggers contract calls for puzzle completion
* **Frontend**: Wallet integration for direct contract interaction
* **Off-chain Database**: Stores detailed puzzle data and player progress

## 🤝 Contributing

Contributions are welcome! Please ensure:

1. All tests pass (`cargo test`)
2. Code follows Rust and Soroban best practices
3. New features include comprehensive tests
4. Documentation is updated

## 📄 License

This project is licensed under the **MIT License**.

## 🔗 Related Repositories

* [Quest Service Backend](https://github.com/MindFlowInteractive/quest-service)
* [Quest Service Frontend](https://github.com/MindFlowInteractive/quest-frontend)

## 📚 Resources

* [Soroban Documentation](https://soroban.stellar.org/docs)
* [Stellar Documentation](https://developers.stellar.org)
* [Soroban Examples](https://github.com/stellar/soroban-examples)

## 💬 Support

For questions or support, please open an issue or join our community discussions.
