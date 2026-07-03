# ADR-0013: Multisig conventions

## Status
Accepted.

## Decision
Critical admin paths go through a multisig wrapper contract, N-of-M, where N and M are configurable and signers are stable.

## Consequences
- Single point of compromise becomes harder.
- Cost: extra cross-contract call per operation.
