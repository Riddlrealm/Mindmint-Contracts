#!/bin/bash

# Puzzle Pool Staking — testnet deploy helper (issue #148)

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

TESTNET_RPC="https://soroban-testnet.stellar.org"
TESTNET_NETWORK="Test SDF Network ; September 2015"
ROOT_DIR="$(dirname "$0")"
WASM_PATH="$ROOT_DIR/.stellar-artifacts/puzzle_pool_staking.wasm"

print_status() { echo -e "${BLUE}➜${NC} $1"; }
print_success() { echo -e "${GREEN}✓${NC} $1"; }

if ! command -v stellar &> /dev/null; then
  echo -e "${YELLOW}Stellar CLI not found. Install it first.${NC}"
  exit 1
fi

if [ -z "$SOURCE_ACCOUNT" ]; then
  read -p "Enter SOURCE_ACCOUNT: " SOURCE_ACCOUNT
fi
if [ -z "$TOKEN_ADDRESS" ]; then
  read -p "Enter TOKEN_ADDRESS (staking/reward token): " TOKEN_ADDRESS
fi
if [ -z "$ORACLE_ADDRESS" ]; then
  read -p "Enter ORACLE_ADDRESS: " ORACLE_ADDRESS
fi

if [ -z "$SOURCE_ACCOUNT" ] || [ -z "$TOKEN_ADDRESS" ] || [ -z "$ORACLE_ADDRESS" ]; then
  echo "Missing SOURCE_ACCOUNT, TOKEN_ADDRESS, or ORACLE_ADDRESS."
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

print_status "Building puzzle_pool_staking wasm..."
cd "$ROOT_DIR"
stellar contract build --package puzzle_pool_staking --profile release --out-dir .stellar-artifacts
print_success "Build complete"

print_status "Deploying puzzle_pool_staking..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM_PATH" \
  --source "$SOURCE_ACCOUNT" \
  --network testnet)
print_success "Deployed: $CONTRACT_ID"

# Defaults: 7d epoch, 7d unstake lock, 70% solver / 30% staker
WEEK_SECS=$((7 * 24 * 60 * 60))
print_status "Initializing contract..."
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOURCE_ACCOUNT" \
  --network testnet \
  -- initialize \
  --admin "$DEPLOYER_ADDRESS" \
  --oracle "$ORACLE_ADDRESS" \
  --token "$TOKEN_ADDRESS" \
  --epoch_duration_secs "$WEEK_SECS" \
  --unstake_lock_secs "$WEEK_SECS" \
  --solver_share_bps 7000

print_success "Puzzle pool staking initialized"
echo "Contract ID: $CONTRACT_ID"
