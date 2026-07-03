#!/usr/bin/env bash
set -euo pipefail
# Print markdown links pointing at files we may not have.
grep -rEon '\]\([^)]+\)' docs/ README.md CONTRIBUTING.md SECURITY.md 2>/dev/null \
  | grep -E '\.md\b|\.rs\b|\.toml\b' || true
