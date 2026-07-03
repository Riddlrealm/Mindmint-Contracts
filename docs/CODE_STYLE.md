# Code style

## Formatting

- `cargo fmt --all` is the source of truth.
- The CI workflow `.github/workflows/fmt.yml` enforces it.

## Naming

- Crates: `snake_case` directory, `kebab-case` package name (e.g. `puzzle_verification`).
- Types: `PascalCase`.
- Functions / variables: `snake_case`.
- Constants: `SCREAMING_SNAKE_CASE`.

## Comments

- Module-level `//!` doc on every contract.
- `# Errors` / `# Panics` sections on public functions.
