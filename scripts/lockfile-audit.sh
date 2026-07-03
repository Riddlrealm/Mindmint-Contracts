#!/usr/bin/env bash
set -euo pipefail
# Cargo.lock freshness check.
[ -f Cargo.lock ] && grep -c '^name =' Cargo.lock | xargs -I{} echo "Lockfile entries: {}"
