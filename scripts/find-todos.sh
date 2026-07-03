#!/usr/bin/env bash
set -euo pipefail
grep -rn -E '\b(TODO|FIXME|XXX|HACK)\b' contracts/ scripts/ src/ 2>/dev/null || true
