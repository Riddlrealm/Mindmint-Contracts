#!/usr/bin/env bash
set -euo pipefail
command -v cargo-llvm-cov >/dev/null 2>&1 || cargo install cargo-llvm-cov --locked
cargo llvm-cov --workspace --html
