# Release checklist

Run before tagging a release.

- [ ] All tests green on `main`.
- [ ] CHANGELOG entry drafted.
- [ ] Backwards-incompatible changes highlighted in `docs/RELEASE_NOTES_GUIDE.md`.
- [ ] Coverage ≥ ADR-0009 target.
- [ ] No SEV1 / SEV2 open.
- [ ] `cargo audit` (or `cargo deny check`) clean.
- [ ] `scripts/verify-build.sh` succeeds.
- [ ] Tag signed and pushed.
