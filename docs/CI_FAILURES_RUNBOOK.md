# CI failures runbook

What to do when CI is red on `main`.

1. Check `git status` of the failing branch.
2. Bisect with `git bisect start && cargo test`.
3. If a flake, mark with `[ci-flake]` and re-queue.
4. If a real regression, open an incident and link from the runbook.
5. Never `git push --force` to fix CI on `main` — revert the offending commit.
