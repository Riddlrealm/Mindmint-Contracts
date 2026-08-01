# Upgrade guide

How to roll out a new contract version.

## Steps

1. Deploy new wasm alongside existing (do not migrate yet).
2. Run shadow-invocation tests against the new contract for 24h.
3. Switch a portion of traffic to the new contract.
4. If SLOs hold for 7d, migrate the rest.
5. Mark old contract deprecated (do not delete).

## Rollback

If the new contract fails SLOs, redirect all traffic back to the old one and pin RPC to the old wasm hash.

---

## Rollback runbook

Use `scripts/rollback.sh` to perform an **atomic, state-preserving** rollback to a
previously deployed WASM.  The script wraps the sequence described below so that no
step is skipped accidentally.

### When to roll back

Trigger a rollback if any of the following conditions hold after a deploy:

- P99 latency exceeds the threshold defined in `docs/SLO_DEFINITIONS.md` for more
  than 15 minutes.
- An unexpected error rate spike appears in the monitoring dashboard
  (`docs/MONITORING.md`).
- A regression is confirmed by the incident triage process
  (`docs/INCIDENT_TRIAGE_RUNBOOK.md`).
- A critical security finding is disclosed that requires reverting to a known-good
  binary (`docs/SECURITY_RESPONSE_TIMELINE.md`).

### Prerequisites

- **`soroban-cli` v21.0.0** installed (`cargo install --locked soroban-cli --version 21.0.0`).
- A funded signing identity known to soroban-cli (`soroban keys fund <identity> --network testnet`).
- The **pinned WASM file** for the rollback target (see "Pinning WASM artefacts" below).
- Admin authority over the affected contract (required by `set_paused` and the init
  entry point).
- `.env` populated from `.env.example` (or exported as shell variables).

### Pinning WASM artefacts

Every release tag should be accompanied by the optimised WASM files.  Store them in a
location that is accessible offline, such as a versioned object-store bucket or a
directory committed to the release branch:

```
releases/
  v1.2.3/
    auction.optimized.wasm
    bridge.optimized.wasm
    ...
```

The WASM hash recorded in `soroban contract inspect` output is the canonical
content-addressed identifier.  Keep a mapping of `git tag → WASM hash` in your
deployment log so you can locate the correct file quickly during an incident.

### Rollback procedure (step by step)

The following sequence is what `scripts/rollback.sh` performs.  You can run the
script directly or follow the manual steps below if tooling is unavailable.

#### Step 1 — Record the pre-rollback state

```bash
soroban contract inspect \
  --id     <CONTRACT_ID> \
  --network testnet
```

Capture the output (WASM hash, storage snapshot) for the post-mortem.

#### Step 2 — Pause the contract

Prevents new transactions from landing while the executable is being swapped.

```bash
soroban contract invoke \
  --id     <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- set_paused --paused true
```

> If the contract does not implement `set_paused`, pass `--skip-pause` to the
> rollback script and accept that a small window of in-flight transactions may
> observe inconsistent state.

#### Step 3 — Optimise the pinned WASM

```bash
soroban contract optimize \
  --wasm     releases/v1.2.2/auction.wasm \
  --wasm-out releases/v1.2.2/auction.optimized.wasm
```

This step is idempotent; skip it if you already have the `.optimized.wasm` file.

#### Step 4 — Upload the WASM blob and update the executable

```bash
# Upload the blob; soroban-cli prints the resulting WASM hash.
WASM_HASH=$(soroban contract upload \
  --wasm    releases/v1.2.2/auction.optimized.wasm \
  --source  deployer \
  --network testnet)

# Point the existing contract ID at the uploaded blob.
# This does NOT touch on-chain storage — state is preserved.
soroban contract deploy \
  --wasm-hash "$WASM_HASH" \
  --source    deployer \
  --network   testnet \
  --alias     <CONTRACT_ID>
```

#### Step 5 — Re-initialise, then unpause

Most contracts guard re-initialisation with an `AlreadyInitialized` check.  Call the
appropriate entry point for your rollback target.  Refer to `docs/CONTRACT_REFERENCE.md`
for the exact signature.

```bash
# Re-initialise (example for the auction contract).
soroban contract invoke \
  --id     <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- initialize --admin <ADMIN_ADDRESS>

# Unpause.
soroban contract invoke \
  --id     <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- set_paused --paused false
```

### Using `scripts/rollback.sh`

The script automates every step above and exits non-zero on any failure, preventing
partial rollbacks.

```bash
# Dry run — prints all commands without executing them.
scripts/rollback.sh \
  --contract  <CONTRACT_ID> \
  --wasm      releases/v1.2.2/auction.wasm \
  --init-fn   initialize \
  --init-args "--admin <ADMIN_ADDRESS>" \
  --network   testnet \
  --dry-run

# Live rollback.
scripts/rollback.sh \
  --contract  <CONTRACT_ID> \
  --wasm      releases/v1.2.2/auction.wasm \
  --init-fn   initialize \
  --init-args "--admin <ADMIN_ADDRESS>" \
  --network   testnet \
  --source    deployer
```

Full flag reference:

| Flag | Required | Description |
|---|---|---|
| `--contract` | ✓ | On-chain contract ID (`C...` address). |
| `--wasm` | ✓ | Path to the pinned `.wasm` file to redeploy. |
| `--init-fn` | ✓ | Re-initialisation entry point name (e.g. `initialize`). |
| `--init-args` | ✓ | Space-separated args forwarded to the init entry point. |
| `--network` | | Soroban network name (default: `testnet`). |
| `--source` | | Signing identity (default: `$SOROBAN_IDENTITY`). |
| `--skip-pause` | | Skip the pause step for contracts without a pause guard. |
| `--dry-run` | | Print commands without executing them. |

### Storage migration rollback

If the failed release also deployed a storage migrator (see ADR-0021 and
`scripts/check-migrations.sh`), the in-place storage layout may differ from what the
rollback WASM expects.  In that case, run the **inverse migrator** after step 5:

```bash
soroban contract invoke \
  --id     <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- migrate_down --target-version <previous_version>
```

Only contracts that explicitly implement `migrate_down` support this path.  If the
rollback target does not, consult the post-mortem process
(`docs/POST_MORTEM_TEMPLATE.md`) and escalate to the on-call engineer.

### After the rollback

1. **Verify on-chain state** — `soroban contract inspect --id <CONTRACT_ID>`.
2. **Restart off-chain indexers** — indexers that were tracking the failed contract
   may have cached stale state.  See `docs/INDEXER_DESIGN.md`.
3. **File a post-mortem** — use `docs/POST_MORTEM_TEMPLATE.md` within 48 hours.
4. **File an incident report** — follow `docs/INCIDENT_TRIAGE_RUNBOOK.md`.
5. **Pin RPC** to the rolled-back WASM hash until the root cause is identified and a
   fixed release is staged through the normal upgrade process above.

### Rollback decision tree

```
Anomaly detected
       │
       ▼
Is it a critical security finding? ──Yes──▶ Pause immediately → rollback → SECURITY_RESPONSE_TIMELINE.md
       │ No
       ▼
Is SLO breached for > 15 min? ──────No───▶ Continue monitoring; open a bug
       │ Yes
       ▼
Is a known-good WASM available? ────No───▶ Escalate to on-call; consider full pause
       │ Yes
       ▼
Run scripts/rollback.sh (--dry-run first)
       │
       ▼
Verify state → restart indexers → file post-mortem
```
