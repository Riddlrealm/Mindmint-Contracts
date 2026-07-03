# Rollback process

If a release causes a regression in production:

1. Pause the affected contract (`soroban contract invoke --id <id> -- set_paused --paused true`).
2. Re-deploy the previous WASM hash (see `docs/UPGRADE_GUIDE.md`).
3. Restart any off-chain indexers orphaned by the rollback.
4. File a post-mortem against the failed release.
