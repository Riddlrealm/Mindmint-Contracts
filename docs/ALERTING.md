# Alerting

| Alert | Severity | Runbook |
|---|---|---|
| RPC down ≥ 5 min | SEV1 | `docs/RUNBOOK.md` |
| Failed-tx rate > 1% sustained | SEV1 | `docs/RUNBOOK.md` |
| p95 latency > 2s | SEV2 | `docs/RUNBOOK.md` |
| Storage > 80% quota | SEV3 | `docs/UPGRADE_GUIDE.md` |

## On-call rotation

- Primary: see `CODEOWNERS`.
- Secondary: see `CODEOWNERS`.
