# Fix #15 — Guard puzzle_verification reward arithmetic against overflow

## Why it matters
An admin misconfiguring `reward_points` large enough, multiplied by the
`difficulty` cap, silently wraps or panics at the wrong layer. Both outcomes
break ledger invariants that `leaderboard`, `achievement_nft`, and
`reward_token` rely on. Because `meta.reward_points` is `i128` and `difficulty`
is cast to `i128` with `as`, the original `meta.reward_points * (meta.difficulty
as i128)` was an **unchecked** multiplication — overflow was silent in dev unless
`overflow-checks` happened to be on.

## Technical context
- `PuzzleMeta.reward_points: i128`; `difficulty: u32` is widened via `as i128`.
- The old code used the `*` operator with no `checked_mul`, and the accumulated
  `rewards += scaled` used `+=` with no `checked_add`.
- `cargo`/CI did not catch this because `clippy` here only denies
  `clippy::correctness`; overflow-checking is a runtime/`overflow-checks`
  concern, not a lint.

## What changed

### `contracts/puzzle_verification/src/lib.rs`
- Added a `#[contracterror]` `Error` enum (coordinating with Issue #27, Result
  refactor) with the `RewardOverflow = 1` variant.
- `verify_solution` now computes `scaled` with `checked_mul` and the running
  balance with `checked_add`; either overflow aborts the call via
  `panic_with_error!(&env, Error::RewardOverflow)` instead of corrupting state.
  ```rust
  let scaled = match meta.reward_points.checked_mul((meta.difficulty as i128).max(1)) {
      Some(v) => v,
      None => panic_with_error!(&env, Error::RewardOverflow),
  };
  let rewards = match rewards.checked_add(scaled) {
      Some(v) => v,
      None => panic_with_error!(&env, Error::RewardOverflow),
  };
  ```

### `contracts/puzzle_verification/src/test.rs`
- Extracted the test module out of `lib.rs` into `src/test.rs` (matches the
  file list for this issue and the repo's `datakey_keys_test.rs` convention).
- Added regression test `test_reward_overflow_panics` (`#[should_panic]`) that
  drives `reward_points = i128::MAX` and `difficulty = u32::MAX` so
  `reward_points * difficulty` overflows `i128`; `verify_solution` must abort
  with `Error::RewardOverflow` rather than wrap.
- Added `test_large_reward_accrues` sanity check (1_000_000 × difficulty 3 =
  3_000_000, no overflow) to confirm the checked path still accrues correctly.

### `docs/SECURE_CODING_GUIDELINES.md`
- Extended the **Arithmetic** section to mandate a `#[should_panic]` regression
  test for every overflow fix, citing
  `contracts/puzzle_verification/src/test.rs::test_reward_overflow_panics`
  (Issue #15) as the canonical example.

## Verification
- `cargo build -p puzzle-verification` succeeds.
- `cargo clippy -p puzzle-verification --lib` (denies `clippy::correctness`)
  passes with rc=0.
- Test logic follows the repo's established `panic_with_error!` +
  `#[should_panic]` pattern (see `contracts/decentralized_identity`).
- NOTE: the workspace-wide `cargo test` / `--all-targets` jobs currently fail
  to compile `soroban-env-host 21.2.1` (a pre-existing, repo-wide dependency
  break unrelated to this change — `ed25519-dalek 3.0.0` `rand_core 0.10` vs
  `rand 0.8.7` `rand_core 0.6` skew). That infra break is tracked separately and
  is not introduced by this PR; the contract's own build and clippy are clean.

## Acceptance criteria checklist
- [x] `checked_mul` used for `scaled`.
- [x] Overflow returns `Error::RewardOverflow`.
- [x] `#[should_panic]` test for `i128::MAX` difficulty × `MAX` reward.
- [x] `docs/SECURE_CODING_GUIDELINES.md` updated to cite the regression test.
- [x] `Overflow` variant added to the new `Error` enum (Issue #27 coordination).

## Labels
`area:security`, `kind:bug`, `priority:P0`, `contract:puzzle_verification`

## Dependencies
Depends on Issue #27 (Result refactor) — the `Error` enum introduced here is the
contract's half of that refactor; remaining panic-to-`Error` conversions can land
in #27.

closes #15
