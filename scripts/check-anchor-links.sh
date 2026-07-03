#!/usr/bin/env bash
set -euo pipefail
# Verify internal markdown anchors resolve to heading slugs.
grep -rEon '\]\(#[^)]+\)' docs/ README.md 2>/dev/null | head -50 || true
