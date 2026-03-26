#!/bin/bash

# Gamification Rewards Contract Deployment Script
# This script builds and deploys the gamification rewards contract to Stellar testnet

set -e

echo "🔨 Building Gamification Rewards Contract..."

# Navigate to contract directory
cd "$(dirname "$0")/contracts/gamification_rewards" || exit 1

# Build the contract
cargo build --target wasm32-unknown-unknown --release

echo "✅ Build complete!"

# Get the WASM file path
WASM_FILE="../../target/wasm32-unknown-unknown/release/gamification_rewards.wasm"

if [ ! -f "$WASM_FILE" ]; then
    echo "❌ Error: WASM file not found at $WASM_FILE"
    exit 1
fi

echo "📦 WASM file ready: $WASM_FILE"

# Check if soroban CLI is installed
if ! command -v soroban &> /dev/null; then
    echo "❌ Error: soroban CLI not found. Please install it first."
    echo "   Installation: https://soroban.stellar.org/docs/getting-started/setup"
    exit 1
fi

echo "🚀 Deploying to Testnet..."

# Deploy to testnet
CONTRACT_ID=$(soroban contract deploy \
    --wasm "$WASM_FILE" \
    --source deployer \
    --network testnet)

echo "✅ Deployment successful!"
echo "📍 Contract ID: $CONTRACT_ID"

# Save contract ID to file
echo "$CONTRACT_ID" > ../../gamification_rewards_contract_id.txt
echo "💾 Contract ID saved to gamification_rewards_contract_id.txt"

echo ""
echo "🎮 Next Steps:"
echo "1. Initialize the contract:"
echo "   soroban contract invoke \\"
echo "     --id $CONTRACT_ID \\"
echo "     -- initialize \\"
echo "     --admin <YOUR_ADMIN_ADDRESS>"
echo ""
echo "2. Set milestone thresholds (optional):"
echo "   soroban contract invoke \\"
echo "     --id $CONTRACT_ID \\"
echo "     -- set_milestone_threshold \\"
echo "     --admin <ADMIN_ADDRESS> \\"
echo "     --level 1 --threshold 10"
echo ""
echo "3. Start recording player actions!"
