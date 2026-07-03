#!/usr/bin/env bash
set -euo pipefail
# Cross-ref check: every \[label\]\(file.md\) link in docs/*.md must resolve.
grep -rEon '\]\(([a-zA-Z0-9_./-]+\.md)\)' docs/ 2>/dev/null \
  | sed -E 's/.*\((.+)\)/\1/' | sort -u | while read -r f; do
    if [ ! -f "docs/$f" ] && [ ! -f "$f" ]; then
      echo "BROKEN: docs link -> $f"
    fi
done
