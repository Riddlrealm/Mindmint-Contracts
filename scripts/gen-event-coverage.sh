#!/usr/bin/env bash
set -euo pipefail

# Generate a static event-emission coverage report for Soroban contracts.
# A public method is considered state-changing when its body writes/removes/updates
# storage or invokes token/client transfer-like methods. It is covered when the same
# method body publishes at least one Soroban event.

python3 - "$@" <<'PY'
import argparse
import re
from pathlib import Path

FLAGSHIP = [
    "achievement_nft",
    "reward_token",
    "puzzle_verification",
    "guild",
    "referral",
    "insurance",
]

STATE_PATTERNS = [
    re.compile(r"\.storage\(\).*?\.(?:set|remove|update)\s*\(", re.S),
    re.compile(r"\btoken::Client::new\([^;]+\.transfer\s*\(", re.S),
    re.compile(r"\.(?:transfer|transfer_from|mint|burn|burn_from|approve)\s*\(", re.S),
]
EVENT_PATTERN = re.compile(r"\.events\(\)\s*\.\s*publish\s*\(", re.S)
PUB_FN = re.compile(r"(?m)^\s*pub\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")


def extract_body(text, start):
    brace = text.find("{", start)
    if brace < 0:
        return ""
    depth = 0
    for i in range(brace, len(text)):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return text[brace + 1:i]
    return text[brace + 1:]


def is_state_changing(body):
    return any(p.search(body) for p in STATE_PATTERNS)


def main():
    parser = argparse.ArgumentParser(description="Generate per-method event coverage for contracts")
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--flagship-only", action="store_true", help="limit report to flagship crates")
    parser.add_argument("--min-coverage", type=float, default=None, help="exit non-zero if coverage is below this percentage")
    args = parser.parse_args()

    root = Path(args.root)
    crates = sorted(root.glob("contracts/*/src/lib.rs"))
    if args.flagship_only:
        flagship_set = set(FLAGSHIP)
        crates = [p for p in crates if p.parents[1].name in flagship_set]

    rows = []
    total_state = total_covered = 0
    for lib in crates:
        crate = lib.parents[1].name
        text = lib.read_text()
        methods = []
        for m in PUB_FN.finditer(text):
            name = m.group(1)
            body = extract_body(text, m.end())
            if not is_state_changing(body):
                continue
            emits = bool(EVENT_PATTERN.search(body))
            methods.append((name, emits))
            total_state += 1
            total_covered += int(emits)
        covered = sum(1 for _, emits in methods if emits)
        missing = [name for name, emits in methods if not emits]
        coverage = 100.0 if not methods else covered * 100.0 / len(methods)
        rows.append((crate, len(methods), covered, missing, coverage))

    scope = "flagship crates" if args.flagship_only else "all contracts"
    print(f"# Event emission coverage ({scope})")
    print()
    print("| Crate | State-changing public methods | Methods with events | Coverage | Missing event emissions |")
    print("| --- | ---: | ---: | ---: | --- |")
    for crate, count, covered, missing, coverage in rows:
        missing_text = ", ".join(f"`{m}`" for m in missing) if missing else "—"
        print(f"| `{crate}` | {count} | {covered} | {coverage:.1f}% | {missing_text} |")

    overall = 100.0 if total_state == 0 else total_covered * 100.0 / total_state
    print()
    print(f"Overall coverage: {total_covered}/{total_state} ({overall:.1f}%).")

    if args.min_coverage is not None and overall < args.min_coverage:
        raise SystemExit(f"coverage {overall:.1f}% is below required {args.min_coverage:.1f}%")

if __name__ == "__main__":
    main()
PY
