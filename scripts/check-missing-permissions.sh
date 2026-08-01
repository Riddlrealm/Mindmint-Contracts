#!/usr/bin/env bash
set -euo pipefail
# Per-function permission check (issue #19). AST-based via tools/check-permissions.
#
# Replaces the old per-file grep, which passed an entire contract on a single
# require_auth match anywhere in the file — masking mutating pub fns that lacked
# their own authorization. The AST checker inspects each pub fn individually.
#
# Scope: enforces the contracts listed in tools/check-permissions (currently
# `guild`, the subject of #19). A one-off workspace-wide run surfaced ~140
# pre-existing candidates across other contracts; triaging those is separate
# follow-up work. Clippy warn-levels for the script are tracked in #46.
cargo run --quiet --manifest-path tools/check-permissions/Cargo.toml
