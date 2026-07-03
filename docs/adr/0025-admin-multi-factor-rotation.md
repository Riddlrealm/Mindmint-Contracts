# ADR-0025: Admin multi-factor rotation

## Status
Accepted.

## Decision
`set_admin` requires the current admin's auth plus a second-factor signature from a hardware signer stored outside the deployment pipeline.

## Consequences
- A leaked deployer key cannot complete rotation alone.
- Out-of-band security dependencies.
