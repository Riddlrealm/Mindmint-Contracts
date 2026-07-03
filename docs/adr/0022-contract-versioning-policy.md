# ADR-0022: Contract versioning policy

## Status
Accepted.

## Decision
Every contract exposes a `version()` method returning a semantic-version string. Storage layouts bump the `MAJOR`; method behaviour bumps `MINOR`; bug fixes bump `PATCH`.

## Consequences
- Discoverability across deployments.
- Initial implementation cost on every contract.
