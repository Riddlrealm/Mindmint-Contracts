# Incident response

When an incident is reported:

1. **Acknowledge** within 15 minutes.
2. **Triage** — identify scope and severity (SEV1/SEV2/SEV3).
3. **Communicate** — open status channel, post in #incidents.
4. **Mitigate** — pause affected contracts, fail over, or roll back.
5. **Recover** — restore service, validate.
6. **Post-mortem** — write within 5 business days using `docs/POST_MORTEM_TEMPLATE.md`.

## Severity definitions

- **SEV1** — production completely down.
- **SEV2** — major functionality degraded.
- **SEV3** — minor functionality degraded.
