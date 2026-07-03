#!/usr/bin/env bash
set -euo pipefail
echo "Building workspace..."
cargo build --workspace --all-targets
echo "Building WASM..."
cargo build --workspace --target wasm32-unknown-unknown --release
echo "Build OK."
