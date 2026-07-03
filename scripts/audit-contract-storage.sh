#!/usr/bin/env bash
set -euo pipefail
# List every DataKey variant across the workspace.
grep -RIn 'enum DataKey' contracts/ | sort
