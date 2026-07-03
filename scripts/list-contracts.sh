#!/usr/bin/env bash
set -euo pipefail
cargo metadata --format-version=1 --no-deps 2>/dev/null \
  | grep -oE '"name":"[^"]+"' \
  | sed 's/"name":"//;s/"//' \
  | sort
