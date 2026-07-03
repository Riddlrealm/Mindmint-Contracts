#!/usr/bin/env bash
set -euo pipefail
# List pub functions that have no matching #[test] in the same crate.
for f in contracts/*/src/lib.rs; do
  crate=$(basename "$(dirname "$(dirname "$f")")")
  pubfns=$(grep -oE 'pub fn [a-z_]+' "$f" | sort -u)
  tests=$(grep -oE 'fn [a-z_]+' "$f" | sort -u)
  echo "=== $crate ==="
  comm -23 <(echo "$pubfns") <(echo "$tests" | sed 's/^fn /pub fn /') || true
done
