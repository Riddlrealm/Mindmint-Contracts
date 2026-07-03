#!/usr/bin/env bash
set -euo pipefail
cargo bench --workspace || true
