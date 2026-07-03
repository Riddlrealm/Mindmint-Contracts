#!/usr/bin/env bash
set -euo pipefail
# Quick GFM sanity: tab-triggered lines, trailing whitespace on blank sections.
for f in README.md CONTRIBUTING.md CHANGELOG.md SECURITY.md CODE_OF_CONDUCT.md; do
  echo "--- $f ---"
  head -3 "$f"
done
