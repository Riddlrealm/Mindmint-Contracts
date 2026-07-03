#!/usr/bin/env bash
set -euo pipefail
# Print docs/*.md older than 6 months without a recent update.
find docs/ -name '*.md' -type f | while read -r f; do
  age=$(stat -c %Y "$f")
  now=$(date +%s)
  diff=$(( (now - age) / 86400 ))
  [ "$diff" -gt 180 ] && echo "STALE: $f ($diff days old)"
done
