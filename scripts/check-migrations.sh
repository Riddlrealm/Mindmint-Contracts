#!/usr/bin/env bash
set -euo pipefail
# Warn if a contract uses DataKey but has no migrator module.
found=0
for d in contracts/*/src; do
  [ -d "$d" ] || continue
  if grep -RIl 'DataKey' "$d" >/dev/null 2>&1; then
    if [ ! -f "$d/../src/migrator.rs" ] && [ ! -f "$d/migrator.rs" ]; then
      echo "$d: storage layout found, no migrator.rs"
      found=1
    fi
  fi
done
exit $found
