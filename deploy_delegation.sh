#!/bin/bash
set -e

echo "Building delegation contract..."
cargo build --manifest-path contracts/delegation/Cargo.toml --target wasm32-unknown-unknown --release

echo ""
echo "==== Deployment Instructions ===="
echo "Since automatic signing wasn't configured with existing keys in the workspace, you can deploy by running:"
echo "stellar contract deploy \\"
echo "  --wasm target/wasm32-unknown-unknown/release/delegation.wasm \\"
echo "  --source puzzle_deployer \\"
echo "  --network testnet"
echo ""
echo "If CLI deployment fails, you can use the manual_deploy.sh approach with stellar laboratory."
