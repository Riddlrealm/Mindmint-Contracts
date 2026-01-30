# Quest Service - Smart Contracts

Soroban smart contracts powering Quest Service, a logic puzzle game on the Stellar blockchain.

## 🔐 Contracts Overview

### Achievement NFT Contract
Mints and manages NFT achievements for puzzle completion milestones.

### Reward Token Contract
Manages custom token rewards and puzzle unlocks.

### Puzzle Verification Contract

Verifies puzzle solutions and triggers rewards.

### Guild Contract
Manages guild membership, treasury, voting, and inter-guild competitions.

### Referral Contract
Tracks referral relationships and distributes rewards to both referrers and referees. Features include:
- Unique referral code generation
- Dual reward distribution (referrer + referee)
- Referral limits per user
- Anti-gaming mechanisms (prevents self-referrals, duplicate registrations)
- Comprehensive statistics tracking
- Event emissions for all referral activities

### Insurance Contract
Protects player assets (NFTs, tokens) against loss through premium-based insurance. Features include:
- Multiple coverage types (NFT, Token, Combined)
- Dynamic premium calculation
- Policy purchase, renewal, and cancellation
- Claim submission and review system
- Admin-reviewed payout processing
- Fraud detection with cooldowns and frequency limits
- Premium pool management
- Prorated refunds on cancellation

## 🛠️ Tech Stack

* **Language**: Rust
* **Framework**: Soroban SDK
* **Network**: Stellar (Testnet/Mainnet)
* **Testing**: Soroban CLI, Rust tests

## 📦 Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add wasm target
rustup target add wasm32-unknown-unknown

# Install Soroban CLI
cargo install --locked soroban-cli --version 21.0.0
```

## 🚀 Quick Start

```bash
# Build all contracts
soroban contract build

# Run tests
cargo test

# Build optimized contracts
soroban contract optimize --wasm target/wasm32-unknown-unknown/release/*.wasm
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific contract tests
cargo test --package achievement-nft
cargo test --package insurance

# Run with output
cargo test -- --nocapture
```

## 📁 Project Structure

```
quest-service-contracts/
├── contracts/
│   ├── achievement_nft/     # NFT achievement contract
│   ├── reward_token/        # Token reward contract
│   ├── puzzle_verification/ # Puzzle verification contract
│   ├── guild/               # Guild management contract
│   ├── referral/            # Referral tracking and rewards contract
│   └── insurance/           # Asset insurance and protection contract
├── tests/                   # Integration tests
├── scripts/                 # Deployment scripts
├── Cargo.toml              # Workspace configuration
└── README.md
```

## 🚢 Deployment

### Deploy to Testnet

```bash
# Configure network
soroban network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"

# Generate identity
soroban keys generate deployer --network testnet

# Fund account
soroban keys fund deployer --network testnet

# Deploy contract
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/achievement_nft.wasm \
  --source deployer \
  --network testnet
```

## 📄 License

This project is licensed under the **MIT License**.

## 🔗 Related Repositories

* [Quest Service Backend](https://github.com/MindFlowInteractive/quest-service)
* [Quest Service Frontend](https://github.com/yourusername/quest-service-frontend)
