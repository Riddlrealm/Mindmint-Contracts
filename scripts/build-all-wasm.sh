#!/usr/bin/env bash
set -euo pipefail
cargo build --workspace --target wasm32-unknown-unknown --release
