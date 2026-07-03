#!/usr/bin/env bash
set -euo pipefail
# A `pub fn` that mutates state should call require_auth() somewhere in the body.
for f in contracts/*/src/lib.rs; do
  if ! grep -q 'require_auth' "$f"; then
    echo "MISSING_AUTH_CHECK: $f"
  fi
done
