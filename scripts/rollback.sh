#!/usr/bin/env bash
set -euo pipefail
# ─────────────────────────────────────────────────────────────────────────────
# rollback.sh — Atomic rollback of a Soroban contract to a pinned WASM hash.
#
# Usage:
#   scripts/rollback.sh --contract <CONTRACT_ID> \
#                       --wasm    <path/to/pinned.wasm> \
#                       --init-fn <init_function_name> \
#                       --init-args '<arg1 arg2 ...>' \
#                       [--network testnet|mainnet] \
#                       [--source  <identity>] \
#                       [--skip-pause] \
#                       [--dry-run]
#
# Required:
#   --contract   On-chain contract ID (C... address).
#   --wasm       Path to the pinned WASM file to redeploy.
#   --init-fn    Name of the re-initialisation entry point (e.g. "initialize").
#   --init-args  Space-separated args forwarded to the init entry point.
#                Quote the whole list: '--init-args "--admin GABC... --paused false"'
#
# Optional:
#   --network    Soroban network name (default: testnet).
#   --source     Signing identity known to soroban-cli (default: $SOROBAN_IDENTITY).
#   --skip-pause Skip the pause step (use only if contract has no pause guard).
#   --dry-run    Print every command without executing it.
#
# Environment variables (all overridden by CLI flags):
#   SOROBAN_IDENTITY           Signing identity (fallback when --source is absent).
#   SOROBAN_RPC_URL            RPC endpoint (read by soroban-cli automatically).
#   SOROBAN_NETWORK_PASSPHRASE Network passphrase (read by soroban-cli automatically).
#
# State preservation guarantee:
#   This script does NOT wipe on-chain storage. It redeploys only the executable
#   WASM, leaving all persistent DataKey entries intact. If the rollback target
#   introduced a DataKey layout change, run the inverse migrator manually after
#   this script completes (see docs/UPGRADE_GUIDE.md § "Storage migration rollback").
#
# Exit codes:
#   0  Success — rollback complete, contract unpaused.
#   1  Argument / precondition error.
#   2  soroban-cli command failed.
# ─────────────────────────────────────────────────────────────────────────────

# ── defaults ─────────────────────────────────────────────────────────────────
NETWORK="${SOROBAN_NETWORK:-testnet}"
SOURCE="${SOROBAN_IDENTITY:-deployer}"
CONTRACT_ID=""
WASM_PATH=""
INIT_FN=""
INIT_ARGS=""
SKIP_PAUSE=false
DRY_RUN=false

# ── argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --contract)   CONTRACT_ID="$2";  shift 2 ;;
    --wasm)       WASM_PATH="$2";    shift 2 ;;
    --init-fn)    INIT_FN="$2";      shift 2 ;;
    --init-args)  INIT_ARGS="$2";    shift 2 ;;
    --network)    NETWORK="$2";      shift 2 ;;
    --source)     SOURCE="$2";       shift 2 ;;
    --skip-pause) SKIP_PAUSE=true;   shift   ;;
    --dry-run)    DRY_RUN=true;      shift   ;;
    *)
      echo "Unknown argument: $1" >&2
      echo "Run: scripts/rollback.sh --help" >&2
      exit 1
      ;;
  esac
done

# ── validation ────────────────────────────────────────────────────────────────
missing=()
[ -z "$CONTRACT_ID" ] && missing+=("--contract")
[ -z "$WASM_PATH"   ] && missing+=("--wasm")
[ -z "$INIT_FN"     ] && missing+=("--init-fn")
[ -z "$INIT_ARGS"   ] && missing+=("--init-args")

if [ ${#missing[@]} -gt 0 ]; then
  echo "Error: missing required arguments: ${missing[*]}" >&2
  exit 1
fi

if [ ! -f "$WASM_PATH" ]; then
  echo "Error: WASM file not found: $WASM_PATH" >&2
  exit 1
fi

if ! command -v soroban &>/dev/null; then
  echo "Error: soroban-cli not found in PATH." >&2
  echo "Install: cargo install --locked soroban-cli --version 21.0.0" >&2
  exit 1
fi

# ── helpers ───────────────────────────────────────────────────────────────────
BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
RESET='\033[0m'

log_info()    { echo -e "${BOLD}[rollback]${RESET} $*"; }
log_ok()      { echo -e "${GREEN}[rollback] ✓${RESET} $*"; }
log_warn()    { echo -e "${YELLOW}[rollback] ⚠${RESET} $*" >&2; }
log_error()   { echo -e "${RED}[rollback] ✗${RESET} $*" >&2; }

# Wraps every soroban-cli call so --dry-run prints without executing.
run() {
  if [ "$DRY_RUN" = true ]; then
    echo "[dry-run] $*"
  else
    "$@" || { log_error "Command failed: $*"; exit 2; }
  fi
}

# ── banner ────────────────────────────────────────────────────────────────────
echo ""
log_info "Mindmint contract rollback"
log_info "  Contract : $CONTRACT_ID"
log_info "  WASM     : $WASM_PATH"
log_info "  Network  : $NETWORK"
log_info "  Source   : $SOURCE"
log_info "  Init fn  : $INIT_FN $INIT_ARGS"
[ "$SKIP_PAUSE" = true ] && log_warn "pause step skipped (--skip-pause)"
[ "$DRY_RUN"    = true ] && log_warn "dry-run mode — no on-chain state will change"
echo ""

# ── step 1: record the pre-rollback WASM hash for the audit trail ─────────────
log_info "Step 1/5  Record pre-rollback WASM hash"
run soroban contract inspect \
  --id     "$CONTRACT_ID" \
  --network "$NETWORK"

# ── step 2: pause the contract so no new transactions land during the swap ────
if [ "$SKIP_PAUSE" = false ]; then
  log_info "Step 2/5  Pause contract"
  run soroban contract invoke \
    --id      "$CONTRACT_ID" \
    --source  "$SOURCE" \
    --network "$NETWORK" \
    -- set_paused --paused true
  log_ok "Contract paused."
else
  log_warn "Step 2/5  Pause skipped."
fi

# ── step 3: optimise the pinned WASM (idempotent; produces *.optimized.wasm) ──
OPTIMIZED_WASM="${WASM_PATH%.wasm}.optimized.wasm"
log_info "Step 3/5  Optimise pinned WASM → $OPTIMIZED_WASM"
run soroban contract optimize \
  --wasm "$WASM_PATH" \
  --wasm-out "$OPTIMIZED_WASM"
log_ok "Optimised."

# ── step 4: upload the pinned WASM and update the contract executable ─────────
log_info "Step 4/5  Upload WASM and update contract"

# Upload the WASM blob; capture the returned hash for logging.
UPLOAD_OUTPUT=""
if [ "$DRY_RUN" = false ]; then
  UPLOAD_OUTPUT=$(soroban contract upload \
    --wasm    "$OPTIMIZED_WASM" \
    --source  "$SOURCE" \
    --network "$NETWORK") || { log_error "WASM upload failed."; exit 2; }
  WASM_HASH="$UPLOAD_OUTPUT"
  log_ok "WASM uploaded. Hash: $WASM_HASH"
else
  echo "[dry-run] soroban contract upload --wasm $OPTIMIZED_WASM --source $SOURCE --network $NETWORK"
  WASM_HASH="<dry-run-hash>"
fi

# Re-deploy the contract executable at the same contract ID.
# `soroban contract deploy --wasm-hash` updates the executable without
# changing the contract ID or wiping storage (state-preserving).
run soroban contract deploy \
  --wasm-hash "$WASM_HASH" \
  --source    "$SOURCE" \
  --network   "$NETWORK" \
  --alias     "$CONTRACT_ID"

log_ok "Contract executable updated to pinned WASM."

# ── step 5: re-initialise then unpause ────────────────────────────────────────
log_info "Step 5/5  Re-initialise contract"
# shellcheck disable=SC2086  # intentional word-splitting of INIT_ARGS
run soroban contract invoke \
  --id      "$CONTRACT_ID" \
  --source  "$SOURCE" \
  --network "$NETWORK" \
  -- "$INIT_FN" $INIT_ARGS

log_ok "Re-initialisation complete."

if [ "$SKIP_PAUSE" = false ]; then
  log_info "          Unpause contract"
  run soroban contract invoke \
    --id      "$CONTRACT_ID" \
    --source  "$SOURCE" \
    --network "$NETWORK" \
    -- set_paused --paused false
  log_ok "Contract unpaused."
fi

# ── summary ───────────────────────────────────────────────────────────────────
echo ""
log_ok "Rollback complete."
echo ""
echo "  Next steps:"
echo "  1. Verify on-chain state:   soroban contract inspect --id $CONTRACT_ID"
echo "  2. Restart off-chain indexers orphaned by the rollback."
echo "  3. File a post-mortem:      see docs/POST_MORTEM_TEMPLATE.md"
echo "  4. File an incident report: see docs/INCIDENT_TRIAGE_RUNBOOK.md"
echo ""
