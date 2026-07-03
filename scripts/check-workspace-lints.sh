#!/usr/bin/env bash
set -euo pipefail
# Force the workspace lints to actually run.
cargo clippy --workspace --all-targets
