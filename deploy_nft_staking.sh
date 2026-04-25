#!/bin/bash

# NFT Staking Contract Deployment Script (Testnet)

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

TESTNET_RPC="https://soroban-testnet.stellar.org"
TESTNET_NETWORK="Test SDF Network ; September 2015"
ROOT_DIR="$(dirname "$0")"
WASM_PATH="$ROOT_DIR/.stellar-artifacts/nft_staking.wasm"

print_status() {
  echo -e "${BLUE}➜${NC} $1"
}

print_success() {
  echo -e "${GREEN}✓${NC} $1"
}

if ! command -v stellar &> /dev/null; then
  echo -e "${YELLOW}Stellar CLI not found. Install it first.${NC}"
  exit 1
fi

if [ -z "$SOURCE_ACCOUNT" ]; then
  read -p "Enter SOURCE_ACCOUNT: " SOURCE_ACCOUNT
fi

if [ -z "$NFT_CONTRACT" ]; then
  read -p "Enter NFT_CONTRACT address: " NFT_CONTRACT
fi

if [ -z "$REWARD_TOKEN" ]; then
  read -p "Enter REWARD_TOKEN address: " REWARD_TOKEN
fi

if [ -z "$SOURCE_ACCOUNT" ] || [ -z "$NFT_CONTRACT" ] || [ -z "$REWARD_TOKEN" ]; then
  echo "Missing SOURCE_ACCOUNT, NFT_CONTRACT, or REWARD_TOKEN."
  exit 1
fi

print_status "Checking testnet network configuration..."
if ! stellar network ls | grep -q "testnet"; then
  stellar network add testnet \
    --rpc-url "$TESTNET_RPC" \
    --network-passphrase "$TESTNET_NETWORK"
fi
print_success "Network is ready"

DEPLOYER_ADDRESS=$(stellar keys address "$SOURCE_ACCOUNT")
print_status "Deployer address: $DEPLOYER_ADDRESS"

print_status "Building nft_staking wasm..."
cd "$ROOT_DIR"
stellar contract build --package nft-staking --profile release --out-dir .stellar-artifacts
print_success "Build complete"

if [ ! -f "$WASM_PATH" ]; then
  echo "❌ Build failed - WASM file not found at $WASM_PATH"
  exit 1
fi

print_status "Deploying nft_staking..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM_PATH" \
  --source "$SOURCE_ACCOUNT" \
  --network testnet)
print_success "Deployed: $CONTRACT_ID"

print_status "Initializing contract..."
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOURCE_ACCOUNT" \
  --network testnet \
  -- initialize \
  --admin "$DEPLOYER_ADDRESS" \
  --nft-contract "$NFT_CONTRACT" \
  --reward-token "$REWARD_TOKEN"
print_success "Initialized"

echo ""
echo "Contract ID: $CONTRACT_ID"
echo ""
echo "Next steps:"
echo "1) Authorize this staking contract as a minter on the reward token contract:"
echo "   stellar contract invoke --id $REWARD_TOKEN --source $SOURCE_ACCOUNT --network testnet \\"
echo "     -- authorize_minter --minter $CONTRACT_ID"
echo ""
echo "2) Configure reward rates per rarity tier:"
echo "   stellar contract invoke --id $CONTRACT_ID --source $SOURCE_ACCOUNT --network testnet \\"
echo "     -- set_rarity_config --admin $DEPLOYER_ADDRESS --rarity 1 --tokens-per-ledger 5"
echo ""
echo "3) Map token ids to rarity tiers (required before staking):"
echo "   stellar contract invoke --id $CONTRACT_ID --source $SOURCE_ACCOUNT --network testnet \\"
echo "     -- set_token_rarity --admin $DEPLOYER_ADDRESS --token-id 1 --rarity 1"

