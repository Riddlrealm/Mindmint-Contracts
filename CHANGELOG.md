# Changelog

All notable changes to Mindmint are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com) and the project adheres to [Semantic Versioning](https://semver.org).

## [1.0.0] - 2026-07-03

### Changed

- Project renamed from the prior working name to **Mindmint**.
- All top-level deployment guides, PR descriptions, scattered notes, deploy scripts and the legacy `scripts/` and `.stellar/` directories removed.
- Doc references to the prior name updated throughout `README.md` and contract documentation.

### Notes

- All on-chain contract source code is unchanged; only project metadata and documentation were updated.
- The git history was rewritten to remove the prior name from commit messages. Force-push was used to publish the new history.

## [Unreleased]

### Added

- MIT `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`.
- GitHub Actions workflows under `.github/workflows/` for `cargo check`, `cargo fmt`, `cargo clippy`, and `cargo doc`.
- Project infrastructure: `CODEOWNERS`, `rust-toolchain.toml`, `deny.toml`, `typos.toml`, `.editorconfig`, `.gitattributes`, `.gitmessage`, `.git-blame-ignore-revs`, `.env.example`, `dependabot.yml`.
- Issue templates: bug report, feature request, question, plus template chooser `config.yml`.
- Documentation index under `docs/`: ARCHITECTURE, QUICK_START, DEPLOYMENT, SECURITY_MODEL, CONTRACT_REFERENCE, TROUBLESHOOTING, TESTING, FAQ, MIGRATION, GLOSSARY, RELEASE_PROCESS.
- Helper scripts under `scripts/`: `verify-build.sh`, `check-formatting.sh`, `lint.sh`, `find-todos.sh`, `setup-dev.sh`.
- Top-level `tests/README.md` and `tests/example_invocation.md`.

### Changed

- Root `Cargo.toml` gained a conservative `[workspace.lints.rust]` section.
- `Makefile` refactored: dead `puzzle_factory.wasm` target removed and replaced with a parameterised `deploy-testnet CONTRACT=<crate>` target.
- Root `README.md` gained `## Acknowledgements` and `## Build & Test` sections.

### Fixed

- `CODEOWNERS` no longer references a non-existent team handle.
- `.github/FUNDING.yml` placeholder replaced with a valid empty configuration.
- `.github/ISSUE_TEMPLATE/config.yml` no longer points at a potentially disabled `/discussions` URL.
