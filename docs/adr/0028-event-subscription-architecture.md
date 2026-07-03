# ADR-0028: Event subscription architecture

## Status
Accepted.

## Decision
Off-chain indexers subscribe to RPC event streams via a typed topic filter (`<crate>::<event>`). Backpressure is handled by the indexer, not on-chain.

## Consequences
- Indexer throughput is the bottleneck.
- Indexer schema becomes a public contract.
