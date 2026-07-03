#!/usr/bin/env bash
set -euo pipefail
total=0
for kind in TODO FIXME XXX HACK; do
  n=$(grep -RIn -E "\b${kind}\b" contracts/ scripts/ src/ 2>/dev/null | wc -l || true)
  printf '%-6s\t%d\n' "$kind" "$n"
  total=$((total + n))
done
echo "TOTAL\t$total"
