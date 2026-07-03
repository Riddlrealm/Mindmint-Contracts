# Pre-release testing

Test plan run before tagging a release.

1. Build and test every workspace crate (`scripts/verify-build.sh`, `scripts/run-tests.sh`).
2. Walk through the `docs/RELEASE_CHECKLIST.md`.
3. Smoke-test primary entry points on testnet with synthetic addresses.
4. Run the load tests in `scripts/run-benchmarks.sh`.
5. Roll back by re-deploying the previous WASM hash.
