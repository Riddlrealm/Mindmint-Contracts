#!/usr/bin/env bash
set -euo pipefail
grep -RIl 'soroban-sdk' contracts/ | while read -r f; do
  printf '%s : ' "$f"
  grep -E 'soroban-sdk' "$f" | head -1
done
