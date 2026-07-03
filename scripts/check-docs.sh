#!/usr/bin/env bash
set -euo pipefail
find . -maxdepth 2 -name '*.md' -not -path './target/*' -not -path './.git/*' | sort
