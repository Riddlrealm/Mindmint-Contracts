# Branching strategy

- `main` — always releasable; protected.
- Feature branches: `<username>/<short-topic>`.
- Tags: `vX.Y.Z` (SemVer).
- Long-running branches are discouraged; prefer short-lived.

PRs require at least one review (see `CODEOWNERS`) and a green CI.
