#!/usr/bin/env bash
set -euo pipefail
# Surface every `#[cfg(feature = ...)]` for review.
grep -RIn 'cfg(feature' contracts/ 2>/dev/null | sort || true
