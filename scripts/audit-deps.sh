#!/usr/bin/env bash
set -euo pipefail
command -v cargo-deny >/dev/null 2>&1 || cargo install cargo-deny --locked
cargo deny check
