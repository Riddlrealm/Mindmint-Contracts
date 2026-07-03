# ADR-0029: Upgrade compatibility window

## Status
Accepted.

## Decision
A deprecated method stays callable for one full MINOR version after deprecation notice (via `#[deprecated]` plus event emission). Removal is a MAJOR bump.

## Consequences
- Downstream SDKs have migration runway.
- Old paths remain attack surface during the window.
