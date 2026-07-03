# Third-party integrations

Inventory of off-chain components that interact with on-chain contracts.

| Component | Direction | Auth | Failover |
|---|---|---|---|
| Indexer | read | none | re-derive from RPC |
| Relayer | write | deployer key | pause contract on miss |
| Bridge | both | multisig | halt one side |
