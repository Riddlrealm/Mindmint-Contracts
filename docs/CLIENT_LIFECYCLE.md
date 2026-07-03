# Client lifecycle

1. Discover deployed address via the indexer.
2. Subscribe to events for the lifetime of the integration.
3. Pin to a contract version range and re-test on MAJOR bumps.
4. Tear-down: drop event subscriptions, archive local state.
