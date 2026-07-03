# Dependency upgrade process

How we ship upstream version bumps.

1. Dependabot opens a PR or a maintainer files one.
2. Review the changelog; tag breaking changes.
3. Re-run coverage and benchmarks.
4. Merge with normal review if compatibility is preserved.
5. Wrap MAJOR bumps in a release cycle per `docs/RELEASE_CHECKLIST.md`.
