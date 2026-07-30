#!/usr/bin/env bash
set -euo pipefail

# Compatibility wrapper for the event coverage audit. The generator owns the
# per-method report; pass --min-coverage 95 to make a local/CI run gating once
# the bugs listed in docs/audits/event-handling-completeness.md are remediated.
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
"$SCRIPT_DIR/gen-event-coverage.sh" "$@"
