#!/usr/bin/env bash
set -euo pipefail
for d in contracts/*/src; do
  [ -d "$d" ] || continue
  n=$(grep -RIn '#\[test\]' "$d" 2>/dev/null | wc -l)
  printf '%s\t%d\n' "$d" "$n"
done
