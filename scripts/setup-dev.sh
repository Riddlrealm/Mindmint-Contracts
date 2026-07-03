#!/usr/bin/env bash
set -euo pipefail
rustup target add wasm32-unknown-unknown
command -v cargo-sort    >/dev/null 2>&1 || cargo install cargo-sort    --locked
command -v cargo-machete >/dev/null 2>&1 || cargo install cargo-machete --locked
echo "Dev environment ready."
