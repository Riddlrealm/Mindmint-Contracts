# ADR-0030: Documentation generation strategy

## Status
Accepted.

## Decision
Public API docs come from crate-level `//!` doc comments and `# Errors`/`# Panics` sections. CI renders to `target/doc` and uploads as an artifact.

## Consequences
- Docs live next to code.
- Doc comments are part of the review surface.
