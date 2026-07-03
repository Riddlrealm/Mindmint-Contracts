# Monitoring

What we monitor and why.

## RPC

- Endpoint availability (1-min probes from 3 regions).
- Per-method latency (p50, p95, p99) and error rate.

## Contracts

- Failed-transaction rate per contract.
- Storage usage vs. quota.
- Event emission rate vs. baseline.

## Pipelines

- CI build duration.
- Deploy success rate.

See also: `docs/ALERTING.md`, `docs/SLO_DEFINITIONS.md`.
