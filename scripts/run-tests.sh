#!/usr/bin/env bash
set -euo pipefail
cargo test --workspace --all-targets -- --nocapture
