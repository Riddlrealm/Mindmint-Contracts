# SLO definitions

## Availability

- 99.9% per calendar month for production RPC.
- 99.5% for non-critical read paths.

## Latency

- p95 < 1.5s for state-changing calls.
- p95 < 500ms for read-only calls.

## Error rate

- < 0.5% for state-changing calls.
- < 0.1% for read-only calls.
