#!/usr/bin/env python3
"""DataKey migration generator for Mindmint contracts.

Issue #41 / ADR-0011: every contract must namespace its storage under a
documented ``enum DataKey`` instead of raw ``symbol_short!(...)`` / ``Symbol::new(...)``
keys. Raw short keys (e.g. ``"admin"``, ``"config"``) collide across proxies and
cause silent storage corruption (Issue #25).

This generator scans the workspace, finds contracts that still use raw storage
keys, and emits a per-contract migration plan: a proposed ``DataKey`` enum
derived from the distinct raw keys it discovers, plus the set of keys that need
to be rewritten. Contracts that already declare a ``DataKey`` enum are reported
as already compliant.

Usage:
    scripts/generate_datakey_migration.py            # print plan to stdout
    scripts/generate_datakey_migration.py --write    # also write scripts/datakey_migration_plan.md
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONTRACTS_DIR = ROOT / "contracts"

# Files that contain test-only code; raw keys there are not storage collisions.
SKIP_FILES = {"test.rs", "tests.rs"}

RAW_KEY_RE = re.compile(r"""(symbol_short!|Symbol::new)\s*\(\s*["']([^"']+)["']""")
COMPOSITE_RE = re.compile(r"""\(\s*symbol_short!\s*\(\s*["']([^"']+)["']\s*\)\s*,\s*([^)]+)\)""")

# Heuristic map from raw key string -> (PascalCase variant, inner key type hint)
KNOWN_VARIANTS = {
    "admin": ("Admin", None),
    "em_admin": ("EmergencyAdmin", None),
    "paused": ("Paused", None),
    "config": ("Config", None),
    "signers": ("Signers", None),
    "audits": ("AuditLogs", None),
    "roles": ("UserRoles", "Address"),
    "perms": ("RolePermissions", "Symbol"),
    "parent": ("RoleParent", "Symbol"),
    "dispute": ("Dispute", "Symbol"),
    "history": ("History", "Symbol"),
    "cache": ("Cache", "Symbol"),
    "emergency": ("Emergency", None),
    "event": ("Event", "u64"),
    "ticket": ("Ticket", "u64"),
    "holder": ("HolderTickets", "Address"),
    "attend": ("Attendance", "u64"),
    "balance": ("Balance", None),
    "or_cfg": ("Config", None),
    "oracles": ("Oracles", None),
    "proof_cnt": ("ProofCounter", None),
    "or_cnt": ("NextProofId", None),
    "proof": ("Proof", "u64"),
    "cnt": ("ActivityCount", "Address, u32"),
    "score": ("ActivityScore", "Address"),
    "cert": ("Cert", "u64"),
    "own_cert": ("OwnerCerts", "Address"),
    "p_minted": ("PuzzleMinted", "String, Address"),
    "tok_cnt": ("TokenCount", None),
}


def to_pascal(token: str) -> str:
    """Convert a snake/short key token to a PascalCase variant name."""
    if token in KNOWN_VARIANTS:
        return KNOWN_VARIANTS[token][0]
    parts = re.split(r"[_\s]+", token.lower())
    return "".join(p.capitalize() for p in parts if p)


def inner_type(token: str) -> str | None:
    if token in KNOWN_VARIANTS:
        return KNOWN_VARIANTS[token][1]
    return None


def scan_contract(src_dir: Path):
    """Return (has_datakey, raw_keys, composite_keys) for a contract src dir."""
    raw_keys: set[str] = set()
    composite_keys: set[tuple[str, str]] = set()
    has_datakey = False

    for path in src_dir.rglob("*.rs"):
        if path.name in SKIP_FILES:
            continue
        text = path.read_text(encoding="utf-8")
        if re.search(r"enum\s+DataKey\b", text):
            has_datakey = True
        for m in RAW_KEY_RE.finditer(text):
            raw_keys.add(m.group(2))
        for m in COMPOSITE_RE.finditer(text):
            composite_keys.add((m.group(1), m.group(2).strip()))

    return has_datakey, raw_keys, composite_keys


def propose_enum(name: str, raw_keys: set[str], composite_keys: set[tuple[str, str]]) -> str:
    lines = [
        "#[contracttype]",
        "#[derive(Clone)]",
        "pub enum DataKey {",
    ]
    # Unit variants from plain raw keys.
    for key in sorted(raw_keys):
        variant = to_pascal(key)
        doc = f"    /// Auto-generated from raw storage key `{key}`."
        lines.append(doc)
        lines.append(f"    {variant},")
    # Tuple variants from composite keys.
    for key, inner in sorted(composite_keys, key=lambda x: x[0]):
        variant = to_pascal(key)
        doc = f"    /// Auto-generated from composite storage key `{key}` (persistent)."
        lines.append(doc)
        # Best-effort inner type hint from the captured tail.
        hint = inner_type(key)
        if hint is None:
            # Derive a rough type from the expression (Address -> Address, Symbol -> Symbol).
            if "Address" in inner:
                hint = "Address"
            elif "Symbol" in inner:
                hint = "Symbol"
            elif "String" in inner:
                hint = "String"
            elif "u64" in inner or "event_id" in inner or "token_id" in inner:
                hint = "u64"
            elif "u32" in inner or "activity_type" in inner:
                hint = "u32"
            else:
                hint = "Val"
        lines.append(f"    {variant}({hint}),")
    lines.append("}")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="write the plan to scripts/datakey_migration_plan.md")
    args = parser.parse_args()

    if not CONTRACTS_DIR.exists():
        print(f"contracts/ directory not found at {CONTRACTS_DIR}", file=sys.stderr)
        return 1

    plan: list[str] = []
    plan.append("# DataKey migration plan (generated)")
    plan.append("")
    plan.append("Generated by `scripts/generate_datakey_migration.py` for Issue #41 / ADR-0011.")
    plan.append("")
    plan.append("Contracts using raw `symbol_short!` / `Symbol::new` storage keys are listed below"
                " with a proposed `DataKey` enum. Contracts that already declare a `DataKey` enum"
                " are marked compliant.")
    plan.append("")

    compliant = 0
    affected: list[tuple[str, set[str], set[tuple[str, str]]]] = []
    for crate in sorted(p for p in CONTRACTS_DIR.iterdir() if p.is_dir()):
        src = crate / "src"
        if not src.exists():
            continue
        has_datakey, raw_keys, composite_keys = scan_contract(src)
        # Only treat as "raw storage key usage" if raw keys were found OUTSIDE a
        # DataKey-declaring contract (otherwise they are event topics, etc.).
        if has_datakey:
            compliant += 1
            plan.append(f"## {crate.name} — compliant (declares `DataKey`)")
            plan.append("")
            continue
        if not raw_keys and not composite_keys:
            compliant += 1
            plan.append(f"## {crate.name} — compliant (no raw storage keys)")
            plan.append("")
            continue
        affected.append((crate.name, raw_keys, composite_keys))

    plan.append(f"**Summary:** {compliant} compliant, {len(affected)} affected.")
    plan.append("")
    for name, raw_keys, composite_keys in affected:
        plan.append(f"## {name} — AFFECTED")
        plan.append("")
        plan.append("Detected raw keys:")
        for k in sorted(raw_keys):
            plan.append(f"- `{k}`")
        for k, _ in sorted(composite_keys):
            plan.append(f"- composite `({k}, ...)`")
        plan.append("")
        plan.append("Proposed `DataKey` enum (apply under `contracttype`):")
        plan.append("")
        plan.append("```rust")
        plan.append(propose_enum(name, raw_keys, composite_keys))
        plan.append("```")
        plan.append("")

    output = "\n".join(plan) + "\n"
    if args.write:
        out_path = ROOT / "scripts" / "datakey_migration_plan.md"
        out_path.write_text(output, encoding="utf-8")
        print(f"Wrote migration plan to {out_path}")
    else:
        print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
