#!/usr/bin/env bash
set -euo pipefail
# Each public state-changing fn should emit at least one event.
for f in contracts/*/src/lib.rs; do
  crate=$(basename "$(dirname "$(dirname "$f")")")
  mut=$(grep -cE '^\s*pub fn ' "$f" 2>/dev/null || echo 0)
  emits=$(grep -cE 'env\.events\(\)\.publish' "$f" 2>/dev/null || echo 0)
  printf '%s\tpublic_fns=%s\tevents=%s\n' "$crate" "$mut" "$emits"
done
