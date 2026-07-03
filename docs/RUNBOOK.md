# Runbook

Day-to-day operations for Mindmint deployments.

## Daily checks

- Verify RPC endpoint reachable.
- Spot-check recent deploys for failed transactions.
- Review error logs at debug level.

## Weekly checks

- Ledger freshness.
- Storage usage vs. quota.
- Outstanding incident post-mortems.

## Common operations

| Task | Command |
|---|---|
| Show contract state | `soroban contract inspect --id <id>` |
| Pause a contract | `soroban contract invoke --id <id> -- set_paused --paused true` |
| Drain storage | See `docs/UPGRADE_GUIDE.md` |
