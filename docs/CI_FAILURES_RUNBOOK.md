# CI failures runbook

What to do when CI is red on `main`.

1. Check `git status` of the failing branch.
2. Bisect with `git bisect start && cargo test`.
3. If a flake, mark with `[ci-flake]` and re-queue.
4. If a real regression, open an incident and link from the runbook.
5. Never `git push --force` to fix CI on `main` — revert the offending commit.
6. **Widen or narrow the clippy deny ladder.** `.github/workflows/clippy.yml`
   denies only `clippy::correctness`. If a `correctness` failure spans
   more than ~5 contracts in one push, it is a real signal: stop and
   review before patching. If, on the other hand, you are tempted to
   add `-D <lint>` from a wider category back to the workflow just to
   recover signal on a particular contract, suppress that line with
   `#[allow(<lint>)]` plus an inline rationale instead -- do not widen
   the workspace-wide deny list. See the inline comment in
   `.github/workflows/clippy.yml` for the current rationale.
