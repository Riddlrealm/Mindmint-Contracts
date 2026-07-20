# Event handling completeness audit

Issue #32's indexer design depends on every contract state transition emitting an event that can be replayed off-chain. This audit adds a reproducible generator and records the current per-method gaps so they can be remediated as contract bugs.

## Scope

Flagship crates are the contracts highlighted in the repository overview:

- `achievement_nft`
- `reward_token`
- `puzzle_verification`
- `guild`
- `referral`
- `insurance`

The generator can also scan every `contracts/*/src/lib.rs` crate to support broader remediation.

## Generator

Run the per-method report from the repository root:

```bash
./scripts/gen-event-coverage.sh --flagship-only
```

For a full workspace report, omit `--flagship-only`:

```bash
./scripts/gen-event-coverage.sh
```

The legacy wrapper remains available:

```bash
./scripts/check-event-emission-coverage.sh --flagship-only
```

The generator classifies a public method as state-changing when its body writes, updates, or removes contract storage, or performs transfer-like token operations. A method is counted as covered when that same public method body calls `env.events().publish(...)`. The heuristic is intentionally conservative: nested helper emissions are not credited to the caller unless the public method body publishes directly, so reviewed false positives should be closed only after confirming the indexer can observe a state-transition event.

## Current flagship coverage

Generated on 2026-07-20 with `./scripts/gen-event-coverage.sh --flagship-only`.

| Crate | State-changing public methods | Methods with events | Coverage | Missing event emissions |
| --- | ---: | ---: | ---: | --- |
| `achievement_nft` | 4 | 2 | 50.0% | `initialize`, `mark_puzzle_completed` |
| `guild` | 11 | 0 | 0.0% | `initialize`, `deposit`, `withdraw`, `vote_withdrawal`, `execute_withdrawal`, `add_resource`, `add_achievement`, `create_proposal`, `vote`, `record_competition`, `disband` |
| `insurance` | 16 | 0 | 0.0% | `initialize`, `purchase_policy`, `renew_policy`, `cancel_policy`, `submit_claim`, `review_claim`, `process_payout`, `add_to_pool`, `withdraw_from_pool`, `flag_user`, `unflag_user`, `update_premium_rates`, `update_coverage_limits`, `update_fraud_params`, `set_paused`, `emergency_withdraw` |
| `puzzle_verification` | 3 | 1 | 33.3% | `initialize`, `set_puzzle` |
| `referral` | 5 | 5 | 100.0% | — |
| `reward_token` | 11 | 0 | 0.0% | `initialize`, `authorize_minter`, `revoke_minter`, `set_burn_controller`, `mint`, `distribute_rewards`, `transfer`, `approve`, `transfer_from`, `spend_for_unlock`, `burn` |

Overall flagship coverage: **8/50 (16.0%)**.

## Bug backlog

Track each missing emission as a contract bug against issue #32. The target for acceptance is **at least 95% flagship coverage**, which currently means at least 48 of the 50 audited state-changing methods must emit directly observable events.

### `achievement_nft`

- `initialize`
- `mark_puzzle_completed`

### `guild`

- `initialize`
- `deposit`
- `withdraw`
- `vote_withdrawal`
- `execute_withdrawal`
- `add_resource`
- `add_achievement`
- `create_proposal`
- `vote`
- `record_competition`
- `disband`

### `insurance`

- `initialize`
- `purchase_policy`
- `renew_policy`
- `cancel_policy`
- `submit_claim`
- `review_claim`
- `process_payout`
- `add_to_pool`
- `withdraw_from_pool`
- `flag_user`
- `unflag_user`
- `update_premium_rates`
- `update_coverage_limits`
- `update_fraud_params`
- `set_paused`
- `emergency_withdraw`

### `puzzle_verification`

- `initialize`
- `set_puzzle`

### `referral`

No missing event emissions detected in state-changing public methods.

### `reward_token`

- `initialize`
- `authorize_minter`
- `revoke_minter`
- `set_burn_controller`
- `mint`
- `distribute_rewards`
- `transfer`
- `approve`
- `transfer_from`
- `spend_for_unlock`
- `burn`

## Remediation guidance

1. Add namespaced events following ADR-0010 and `docs/EVENT_DECODING.md`.
2. Re-run `./scripts/gen-event-coverage.sh --flagship-only --min-coverage 95` after each remediation batch.
3. Update this audit with the new report and remove closed bug entries.
