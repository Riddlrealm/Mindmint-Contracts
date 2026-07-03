#!/usr/bin/env bash
set -euo pipefail
cargo update --workspace --precise $(cargo metadata --format-version=1 --no-deps | jq -r '.resolve.nodes[]?.dependencies[]?.req' | head -1)
echo "Snapshot locked. Review Cargo.lock diff and commit."
