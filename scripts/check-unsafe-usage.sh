#!/usr/bin/env bash
set -euo pipefail
# Print every `unsafe {` block location.
grep -RIn 'unsafe\s*{' contracts/ 2>/dev/null || true
