# ADR-0004: Workspace lint policy

## Status

Accepted.

## Decision

Apply a small set of conservative rust_2018_idioms warnings to the entire workspace. Avoid blanket `forbid`/`deny` on `unsafe_code` because the Soroban SDK and FFI shims rely on it.

## Consequences

- Workspace-wide lint hygiene without breaking existing crates.
