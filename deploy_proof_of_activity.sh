#!/bin/bash

# Proof of Activity Contract Deployment Script
# This script deploys the proof_of_activity contract to the testnet

set -e

echo "🚀 Starting Proof of Activity Contract Deployment..."

# Check if stellar-cli is available
if ! command -v stellar &> /dev/null; then
    echo "❌ Error: stellar-cli not found. Please install Stellar CLI first."
    exit 1
fi

# Check if we have the compiled WASM file
WASM_FILE="target/wasm32-unknown-unknown/release/proof_of_activity.wasm"
if [ ! -f "$WASM_FILE" ]; then
    echo "📦 Building proof_of_activity contract..."
    cargo build --release --target wasm32-unknown-unknown --package proof_of_activity
fi

if [ ! -f "$WASM_FILE" ]; then
    echo "❌ Error: WASM file not found at $WASM_FILE"
    exit 1
fi

echo "✅ WASM file found: $WASM_FILE"

# Set network (testnet by default)
NETWORK="testnet"
if [ "$1" = "mainnet" ]; then
    NETWORK="mainnet"
    echo "⚠️  WARNING: Deploying to mainnet!"
fi

echo "🌐 Deploying to $NETWORK..."

# Deploy the contract
echo "📤 Deploying contract..."
CONTRACT_ID=$(stellar contract deploy \
    --wasm "$WASM_FILE" \
    --network "$NETWORK" \
    --source default)

if [ $? -eq 0 ]; then
    echo "✅ Contract deployed successfully!"
    echo "📋 Contract ID: $CONTRACT_ID"
else
    echo "❌ Contract deployment failed!"
    exit 1
fi

# Initialize the contract
echo "⚙️  Initializing contract..."
ADMIN_ADDRESS=$(stellar address --network "$NETWORK")

stellar contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    --source default \
    -- initialize \
    --admin "$ADMIN_ADDRESS"

if [ $? -eq 0 ]; then
    echo "✅ Contract initialized successfully!"
else
    echo "❌ Contract initialization failed!"
    exit 1
fi

# Verify deployment
echo "🔍 Verifying deployment..."
stellar contract read \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    -- get_activity_score \
    --player "$ADMIN_ADDRESS"

echo ""
echo "🎉 Proof of Activity Contract deployment complete!"
echo ""
echo "📊 Contract Details:"
echo "   Contract ID: $CONTRACT_ID"
echo "   Network: $NETWORK"
echo "   Admin: $ADMIN_ADDRESS"
echo ""
echo "🔧 Usage Examples:"
echo "   # Add an oracle:"
echo "   stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source default -- add_oracle --admin $ADMIN_ADDRESS --oracle <ORACLE_ADDRESS>"
echo ""
echo "   # Record a proof:"
echo "   stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source <ORACLE_ADDRESS> -- record_proof --oracle <ORACLE_ADDRESS> --player <PLAYER_ADDRESS> --activity_type 0 --ref_id \"puzzle_123\" --score 100"
echo ""
echo "   # Get player's total score:"
echo "   stellar contract read --id $CONTRACT_ID --network $NETWORK -- get_activity_score --player <PLAYER_ADDRESS>"
echo ""
echo "   # Get player's proofs:"
echo "   stellar contract read --id $CONTRACT_ID --network $NETWORK -- get_player_proofs --player <PLAYER_ADDRESS> --activity_type 0 --offset 0 --limit 10"

# Save deployment info
DEPLOYMENT_FILE="proof_of_activity_deployment_$NETWORK.txt"
cat > "$DEPLOYMENT_FILE" << EOF
Proof of Activity Contract Deployment Information
===============================================
Date: $(date)
Network: $NETWORK
Contract ID: $CONTRACT_ID
Admin: $ADMIN_ADDRESS
WASM File: $WASM_FILE

Activity Types:
- 0: PuzzleSolved
- 1: TournamentCompleted  
- 2: WaveContributed

Quick Commands:
- Add Oracle: stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source default -- add_oracle --admin $ADMIN_ADDRESS --oracle <ORACLE_ADDRESS>
- Remove Oracle: stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source default -- remove_oracle --admin $ADMIN_ADDRESS --oracle <ORACLE_ADDRESS>
- Record Proof: stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source <ORACLE_ADDRESS> -- record_proof --oracle <ORACLE_ADDRESS> --player <PLAYER_ADDRESS> --activity_type <TYPE> --ref_id <REF_ID> --score <SCORE>
- Get Proof: stellar contract read --id $CONTRACT_ID --network $NETWORK -- get_proof --proof_id <ID>
- Get Player Proofs: stellar contract read --id $CONTRACT_ID --network $NETWORK -- get_player_proofs --player <PLAYER_ADDRESS> --activity_type <TYPE> --offset <OFFSET> --limit <LIMIT>
- Get Activity Score: stellar contract read --id $CONTRACT_ID --network $NETWORK -- get_activity_score --player <PLAYER_ADDRESS>
- Check Oracle: stellar contract read --id $CONTRACT_ID --network $NETWORK -- is_authorized_oracle --oracle <ADDRESS>
EOF

echo "📄 Deployment information saved to: $DEPLOYMENT_FILE"
echo ""
echo "✨ Ready to record tamper-proof activity proofs!"
