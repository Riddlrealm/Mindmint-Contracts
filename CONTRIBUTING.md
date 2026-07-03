# Contributing to Mindmint

Thanks for your interest in making Mindmint better. This document explains how to set up the project locally, propose changes, and submit pull requests.

## Development Setup

1. Install Rust (`rustup`).
2. Add the WASM target: `rustup target add wasm32-unknown-unknown`.
3. Install the Soroban CLI: `cargo install --locked soroban-cli --version 21.0.0`.
4. Clone the repository and run `cargo check --workspace --all-targets` to confirm you can build everything.

## Making Changes

- Open an issue describing the problem first if the change is non-trivial.
- Fork the repo and create a feature branch off `main`.
- Keep commits small and focused. Use descriptive commit messages.
- Run `cargo fmt`, `cargo clippy`, and `cargo test` before pushing.

## Pull Requests

- Reference the issue the PR addresses.
- Add tests for any new behaviour.
- Update docs when behaviour changes.
- Make sure CI is green.

## Code of Conduct

Please read and follow our `CODE_OF_CONDUCT.md`.

## Reporting Issues

Use the issue templates under `.github/ISSUE_TEMPLATE/`.
