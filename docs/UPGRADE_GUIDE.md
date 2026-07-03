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
