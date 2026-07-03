#!/usr/bin/env bash
set -euo pipefail
TAG="${1:-}"
[ -n "$TAG" ] || { echo "Usage: scripts/release-tag.sh vX.Y.Z"; exit 1; }
git tag -s "$TAG" -m "$TAG"
git push --tags
