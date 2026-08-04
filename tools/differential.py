#!/usr/bin/env python3
"""Prove the Rust judge and the Python one reach the same verdict.

A rewrite that only agrees on a clean tree has proven nothing — every gate agrees
that nothing is wrong. So this mutates each real contract in the ways a person
actually breaks one (drop a seal, drop a guest list, lower the reach ceiling,
un-ratify a cycle) and requires the two implementations to produce the *same set
of findings*, law by law, file by file.

Usage:
    tools/differential.py --python /path/to/python3.14 --ward /path/to/ward-pkg
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ZONING = HERE.parent / "target" / "release" / "zoning"
WORKSPACE = HERE.parent.parent

# Every governed package, as (repo-relative contract, module tree to hang beside it).
CONTRACTS = [
    ("irregex", "irregex/contract/irregex.zone"),
    ("gist", "gist/contract/gist.zone"),
    ("relate", "relate/contract/relate.zone"),
    ("blast", "blast/contract/blast.zone"),
    ("billog", "billy/libs/kernels/billog/contract/billog.zone"),
    ("lamina", "billy/libs/kernels/lamina/contract/lamina.zone"),
    ("principia", "billy/libs/kernels/principia/contract/principia.zone"),
]


def mutations(text: str) -> dict[str, str]:
    """The ways a contract gets broken, applied one at a time."""
    out: dict[str, str] = {"pristine": text}

    # Drop every seal: everything that entered a deep module by its door now bypasses it.
    stripped = re.sub(r"(?m)^seal .*(\n\s+open to .*)*$", "", text)
    if stripped != text:
        out["no-seals"] = stripped

    # Drop every guest list: kept regions become open, so nothing is flagged — the
    # inverse check, that removing a rule removes exactly its findings.
    stripped = re.sub(r"(?m)^keep .*$", "", text)
    if stripped != text:
        out["no-keeps"] = stripped

    # Squeeze the reach ceiling until real imports exceed it.
    for hops in (1, 2):
        squeezed = re.sub(r"(?m)^limit\s+reach to \d+ hops$", f"limit  reach to {hops} hops", text)
        if squeezed != text:
            out[f"reach-{hops}"] = squeezed

    # Un-ratify every exception: the cycles and edges the contract excuses come back.
    unratified = re.sub(r"(?ms)^variance .*?because.*?(?=\n\n|\Z)", "", text)
    if unratified != text:
        out["no-variances"] = unratified

    # Invert the zone stack: every edge that pointed down now points up.
    zones = re.search(r"(?ms)^zones \{\n(.*?)^\}", text)
    if zones:
        body = zones.group(1)
        rows = [ln for ln in body.splitlines() if ln.strip() and not ln.strip().startswith("//")]
        if len(rows) > 1:
            flipped = body.replace("\n".join(rows), "\n".join(reversed(rows)))
            out["inverted"] = text[: zones.start(1)] + flipped + text[zones.end(1) :]
    return out


def stage(root: Path, contract_text: str, name: str, module_root: Path) -> Path:
    """A scratch package: the mutated contract, beside a symlink to the real tree."""
    box = root / name
    (box / "contract").mkdir(parents=True)
    (box / "contract" / f"{name}.zone").write_text(contract_text)
    (box / "contract" / f"{name}.ward").write_text(unrename(contract_text))
    os.symlink(module_root, box / module_root.name)
    return box


def unrename(text: str) -> str:
    """The same contract in the older spelling, for the Python implementation."""
    text = re.sub(r"(?m)^zones(\s*\{)", r"tiers\1", text)
    return re.sub(r"(?m)^variance ", "allow ", text)


def findings(argv: list[str], cwd: Path, env: dict[str, str] | None = None) -> set[tuple]:
    done = subprocess.run(
        argv, cwd=cwd, capture_output=True, text=True, env={**os.environ, **(env or {})}, check=False
    )
    if done.returncode == 2:
        return {("INVOCATION-FAILED", done.stderr.strip()[:200])}
    try:
        rows = json.loads(done.stdout or "[]")
    except json.JSONDecodeError:
        return {("UNPARSEABLE", done.stdout[:200])}
    # `tier` was renamed `zone` in the rewrite; the law is the same law.
    renamed = {"tier": "zone", "allow": "variance"}
    return {
        (renamed.get(r.get("law"), r.get("law")), r.get("file", ""), r.get("subject", ""))
        for r in rows
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--ward", required=True, help="directory containing the `ward` package")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    if not ZONING.exists():
        print(f"build first: cargo build --release ({ZONING} missing)", file=sys.stderr)
        return 2

    checked = agreed = 0
    disagreements: list[str] = []

    for name, rel in CONTRACTS:
        source = WORKSPACE / rel
        if not source.exists():
            print(f"skip {name}: no {rel}")
            continue
        text = source.read_text()
        module_root = source.parent.parent / declared_root(text)

        for label, mutated in mutations(text).items():
            with tempfile.TemporaryDirectory() as tmp:
                box = stage(Path(tmp), mutated, name, module_root)
                rust = findings([str(ZONING), "verify", "--json", "--untracked"], box)
                py = findings(
                    [args.python, "-m", "ward.cli", "verify", "--json", "--untracked"],
                    box,
                    {"PYTHONPATH": args.ward},
                )
                checked += 1
                if rust == py:
                    agreed += 1
                    if args.verbose:
                        print(f"  = {name}/{label}: {len(rust)} finding(s)")
                    continue
                only_rust = sorted(rust - py)[:5]
                only_py = sorted(py - rust)[:5]
                disagreements.append(
                    f"{name}/{label}: rust {len(rust)} vs python {len(py)}\n"
                    f"    only rust:   {only_rust}\n"
                    f"    only python: {only_py}"
                )

    print(f"\n{agreed}/{checked} scenarios agree")
    for row in disagreements:
        print(f"\n✗ {row}")
    return 0 if agreed == checked else 1


def declared_root(text: str) -> str:
    found = re.search(r"(?m)^\s+root\s+(\S+)", text)
    return found.group(1) if found else "src"


if __name__ == "__main__":
    sys.exit(main())
