#!/usr/bin/env bash
set -euo pipefail
git log --since="3 months ago" --pretty=format: --name-only --diff-filter=AM \
  | grep -E '^(contracts/.*src/.+\.rs)$' \
  | sort | uniq -c | sort -rn | head -20
