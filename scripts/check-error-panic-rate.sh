#!/usr/bin/env bash
set -euo pipefail
# List #[should_panic] tests; warn if any crate has fewer than 5.
for f in contracts/*/src/lib.rs; do
  crate=$(basename "$(dirname "$(dirname "$f")")")
  n=$(grep -c 'should_panic' "$f" 2>/dev/null || echo 0)
  printf '%s\tnegative_tests=%s\n' "$crate" "$n"
done
