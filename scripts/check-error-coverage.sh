#!/usr/bin/env bash
set -euo pipefail
# Each Error variant should appear in at least one #[should_panic] test.
for f in contracts/*/src/lib.rs; do
  crate=$(basename "$(dirname "$(dirname "$f")")")
  errs=$(grep -oE 'Error::[A-Z][A-Za-z_]+' "$f" | sort -u)
  echo "=== $crate ==="
  echo "$errs" || true
done
