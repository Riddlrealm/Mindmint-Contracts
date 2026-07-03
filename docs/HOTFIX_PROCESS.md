# Hotfix process

When a SEV1 lands outside the regular release cadence:

1. Branch from the most recent release tag: `git checkout -b hotfix/<topic> vX.Y.Z`.
2. Apply the smallest possible fix; keep blast radius contained.
3. Cut a PATCH release on `main` (`scripts/release-tag.sh vX.Y.(Z+1)`).
4. Back-port the fix forward to `main` so the next MINOR inherits it.
5. Post a release note referencing `docs/INCIDENT_TRIAGE_RUNBOOK.md`.
