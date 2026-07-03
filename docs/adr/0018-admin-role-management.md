# ADR-0018: Admin role management

## Status
Accepted.

## Decision
Admin addresses are stored in contract instance storage under `DataKey::Admin`. `set_admin` requires multi-factor confirmation (see ADR-0025).

## Consequences
- Single address loss is recoverable.
- Recovery flow must be tested.
