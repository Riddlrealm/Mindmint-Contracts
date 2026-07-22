#!/usr/bin/env bash
set -euo pipefail
cargo clippy --workspace --all-targets -- -D warnings
echo "Clippy OK."

# Run the AST-based lint-driver (aggregates all scripts/check-*.sh rules)
if command -v cargo &>/dev/null && [ -f tools/lint-driver/Cargo.toml ]; then
  echo ""
  echo "Running lint-driver..."
  cargo run --manifest-path tools/lint-driver/Cargo.toml -- --all
  echo "Lint-driver OK."
fi
