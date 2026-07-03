# Release process

How Mindmint cuts a release.

## Versioning

Semantic Versioning: MAJOR.MINOR.PATCH.

- **MAJOR** — incompatible contract or instruction changes
- **MINOR** — backwards-compatible new features
- **PATCH** — backwards-compatible bug fixes

## Steps

1. Pick the version per SemVer; bump in `CHANGELOG.md`.
2. Sign and tag: `git tag -s vX.Y.Z -m 'vX.Y.Z'`
3. Push tags: `git push --tags`
4. Build optimised WASM for every crate: `bash scripts/verify-build.sh`
5. Draft release notes by copying `.github/RELEASE_TEMPLATE.md` and filling it in.
6. Publish: GitHub Actions builds the wasm artifacts and attaches them to the release.
