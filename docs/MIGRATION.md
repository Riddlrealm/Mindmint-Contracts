# Migration guide

How to move between Mindmint versions.

## 0.x → 1.0.0 (project rename)

- Project renamed from the previous working name to **Mindmint**.
- All top-level deployment guides, PR descriptions, scattered notes, deploy scripts, and the legacy `scripts/` and `.stellar/` directories were removed at the v1.0.0 cut.
- Doc references to the prior name were updated throughout `README.md` and contract documentation.
- On-chain contract source code is unchanged.

## Future versions

Each release will update this file with a "Breaking changes" section. Cross-contract consumers should pin to specific tags.
