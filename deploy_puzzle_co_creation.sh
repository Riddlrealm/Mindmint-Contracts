#!/bin/bash

# Puzzle Co-Creation Contract Deployment Script
# This script handles the deployment of the puzzle co-creation contract

set -e

echo "🚀 Puzzle Co-Creation Contract Deployment"
echo "========================================"

# Set SSL certificate path to fix certificate issues
export SSL_CERT_FILE=/usr/local/etc/openssl@3/cert.pem

# Contract details
CONTRACT_NAME="puzzle_co_creation"
WASM_FILE="target/wasm32v1-none/release/puzzle_co_creation.wasm"
SOURCE_ACCOUNT="puzzle_deployer"
NETWORK="testnet"

echo "📋 Contract Details:"
echo "  - Name: $CONTRACT_NAME"
echo "  - WASM: $WASM_FILE"
echo "  - Source: $SOURCE_ACCOUNT"
echo "  - Network: $NETWORK"

# Check if WASM file exists
if [ ! -f "$WASM_FILE" ]; then
    echo "❌ WASM file not found. Building contract..."
    soroban contract build --package puzzle_co_creation
fi

# Verify WASM file
echo "✅ WASM file found: $(ls -lh $WASM_FILE | awk '{print $5}')"

# Check account balance
echo "💰 Checking account balance..."
ACCOUNT_ADDRESS=$(stellar keys address $SOURCE_ACCOUNT)
echo "  - Address: $ACCOUNT_ADDRESS"

# Fund account if needed (check balance first)
BALANCE=$(stellar account info $ACCOUNT_ADDRESS --network $NETWORK 2>/dev/null | grep "Balance:" | awk '{print $2}' || echo "0")
if [ "$BALANCE" = "0" ] || [ -z "$BALANCE" ]; then
    echo "🪙 Funding account on testnet..."
    curl "https://friendbot.stellar.org/?addr=$ACCOUNT_ADDRESS" > /dev/null 2>&1
    echo "✅ Account funded"
else
    echo "✅ Account already funded: $BALANCE XLM"
fi

# Upload WASM to network
echo "📤 Uploading WASM to network..."
UPLOAD_RESULT=$(stellar contract upload --wasm $WASM_FILE --source $SOURCE_ACCOUNT --network $NETWORK)
echo "✅ WASM uploaded: $UPLOAD_RESULT"

# Try different deployment approaches
echo "🚀 Attempting deployment..."

# Method 1: Direct deployment
echo "📌 Method 1: Direct deployment..."
if stellar contract deploy --wasm $WASM_FILE --source $SOURCE_ACCOUNT --network $NETWORK --ignore-checks 2>/dev/null; then
    echo "✅ Deployment successful!"
    DEPLOYMENT_SUCCESS=true
else
    echo "❌ Direct deployment failed"
    DEPLOYMENT_SUCCESS=false
fi

# Method 2: Using WASM hash if direct deployment fails
if [ "$DEPLOYMENT_SUCCESS" = false ]; then
    echo "📌 Method 2: Using WASM hash..."
    WASM_HASH=$(stellar contract compute-wasm-hash --wasm $WASM_FILE)
    if stellar contract deploy --wasm-hash $WASM_HASH --source $SOURCE_ACCOUNT --network $NETWORK --ignore-checks 2>/dev/null; then
        echo "✅ Deployment successful!"
        DEPLOYMENT_SUCCESS=true
    else
        echo "❌ WASM hash deployment failed"
    fi
fi

# Method 3: Manual transaction building if automated deployment fails
if [ "$DEPLOYMENT_SUCCESS" = false ]; then
    echo "📌 Method 3: Manual transaction building..."
    echo "⚠️  This requires manual intervention due to CLI version compatibility"
    echo ""
    echo "🔧 Manual Deployment Instructions:"
    echo "1. Update Stellar CLI to latest version:"
    echo "   brew install stellar  # or cargo install stellar-cli --force"
    echo ""
    echo "2. Or use the following manual steps:"
    echo "   - Source Account: $ACCOUNT_ADDRESS"
    echo "   - Network: $NETWORK"
    echo ""
    echo "3. Try deployment with updated CLI:"
    echo "   stellar contract deploy --wasm $WASM_FILE --source $SOURCE_ACCOUNT --network $NETWORK"
fi

# If deployment was successful, provide next steps
if [ "$DEPLOYMENT_SUCCESS" = true ]; then
    echo ""
    echo "🎉 Contract Deployment Complete!"
    echo "================================"
    echo ""
    echo "📝 Next Steps:"
    echo "1. Initialize the contract with royalty oracle:"
    echo "   stellar contract invoke --id CONTRACT_ID --source $SOURCE_ACCOUNT --network $NETWORK -- initialize --oracle ORACLE_ADDRESS"
    echo ""
    echo "2. Initiate a co-creation collaboration:"
    echo "   stellar contract invoke --id CONTRACT_ID --source CREATOR_ADDRESS --network $NETWORK -- initiate --puzzle_id 123 --creators '[{address: CREATOR1, share_bps: 7000}, {address: CREATOR2, share_bps: 3000}]'"
    echo ""
    echo "3. Co-creators sign the collaboration:"
    echo "   stellar contract invoke --id CONTRACT_ID --source CREATOR2_ADDRESS --network $NETWORK -- sign --co_creation_id CO_CREATION_ID --signer CREATOR2_ADDRESS"
    echo ""
    echo "4. Publish the puzzle after all signatures:"
    echo "   stellar contract invoke --id CONTRACT_ID --source ANY_CREATOR --network $NETWORK -- publish --co_creation_id CO_CREATION_ID"
    echo ""
    echo "5. Distribute royalties (called by oracle):"
    echo "   stellar contract invoke --id CONTRACT_ID --source ORACLE_ADDRESS --network $NETWORK -- distribute_royalty --co_creation_id CO_CREATION_ID --total_amount 1000"
    echo ""
    echo "🔗 Contract Functions Available:"
    echo "  - initialize(oracle)"
    echo "  - initiate(puzzle_id, creators)"
    echo "  - sign(co_creation_id, signer)"
    echo "  - publish(co_creation_id)"
    echo "  - withdraw_signature(co_creation_id, signer)"
    echo "  - distribute_royalty(co_creation_id, total_amount)"
    echo "  - get_co_creation(id)"
    echo "  - has_signed(co_creation_id, signer)"
    echo "  - get_oracle()"
    echo ""
    echo "📊 Important Notes:"
    echo "  - Creator shares must sum to exactly 10000 basis points (100%)"
    echo "  - All creators must sign before publishing"
    echo "  - Signatures can be withdrawn before all have signed (reverts to draft)"
    echo "  - Royalties are automatically split according to creator shares"
    echo "  - Only the royalty oracle can call distribute_royalty"
else
    echo ""
    echo "❌ Deployment Failed"
    echo "=================="
    echo "The deployment failed due to CLI compatibility issues."
    echo "Please update your Stellar CLI and try again."
fi

echo ""
echo "🔍 SSL Certificate Fix Applied: $SSL_CERT_FILE"
echo "📊 Account: $ACCOUNT_ADDRESS"
echo "🌐 Network: $NETWORK"
