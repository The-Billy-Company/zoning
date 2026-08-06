#!/usr/bin/env python3
"""Prove the Rust judge and the Python one reach the same verdict.

A rewrite that only agrees on a clean tree has proven nothing — every gate agrees
that nothing is wrong. So this mutates each real contract in the ways a person
actually breaks one (drop a seal, drop a guest list, lower the reach ceiling,
un-ratify a cycle) and requires the two implementations to produce the *same set
of findings*, law by law, file by file.

The contracts are whichever ones the surrounding workspace holds, swept rather
than named, so this file carries no list of packages to keep in step.

Usage:
    tools/differential.py --python /path/to/python3.14 --ward /path/to/ward-pkg
    tools/differential.py --ward /path/to/ward-pkg --contract ../acme/acme.zone
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
REPO = HERE.parent
ZONING = REPO / "target" / "release" / "zoning"
WORKSPACE = REPO.parent

# Directories a sweep never enters, so discovery stays cheap in a large tree.
SKIP = {
    "build",
    "dist",
    "node_modules",
    "site-packages",
    "target",
    "vendor",
    "zig-cache",
    "zig-out",
    "zig-pkg",
}


def discover(root: Path, depth: int = 6) -> list[tuple[str, Path]]:
    """Every governed package under `root`, found rather than listed.

    A contract sits at the root it governs (`acme/acme.zone`) or in that root's
    drawer (`acme/contract/acme.zone`); both spellings resolve to the same anchor,
    so a sweep accepts either and names no package in this file. This repository
    is skipped: its own fixtures are contracts written to fail, not packages.
    """
    found: dict[str, Path] = {}
    stack = [(str(root), 0)]
    while stack:
        here, level = stack.pop()
        with os.scandir(here) as entries:
            for entry in entries:
                if entry.name.startswith(".") or entry.name in SKIP:
                    continue
                if entry.is_dir(follow_symlinks=False):
                    if level < depth and entry.path != str(REPO):
                        stack.append((entry.path, level + 1))
                elif entry.name.endswith(".zone"):
                    found.setdefault(entry.name[: -len(".zone")], Path(entry.path))
    return sorted(found.items())


def anchor(contract: Path) -> Path:
    """The directory a contract governs, under either layout."""
    return (
        contract.parents[1] if contract.parent.name == "contract" else contract.parent
    )


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
        squeezed = re.sub(
            r"(?m)^limit\s+reach to \d+ hops$", f"limit  reach to {hops} hops", text
        )
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
        rows = [
            ln
            for ln in body.splitlines()
            if ln.strip() and not ln.strip().startswith("//")
        ]
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


def findings(
    argv: list[str], cwd: Path, env: dict[str, str] | None = None
) -> set[tuple]:
    done = subprocess.run(
        argv,
        cwd=cwd,
        capture_output=True,
        text=True,
        env={**os.environ, **(env or {})},
        check=False,
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
        (
            renamed.get(r.get("law"), r.get("law")),
            r.get("file", ""),
            r.get("subject", ""),
        )
        for r in rows
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument(
        "--ward", required=True, help="directory containing the `ward` package"
    )
    ap.add_argument(
        "--contract",
        action="append",
        type=Path,
        metavar="PATH",
        help="a contract to mutate; repeatable. Default: sweep the workspace.",
    )
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    if not ZONING.exists():
        print(f"build first: cargo build --release ({ZONING} missing)", file=sys.stderr)
        return 2

    contracts = (
        [(path.resolve().stem, path.resolve()) for path in args.contract]
        if args.contract
        else discover(WORKSPACE)
    )
    if not contracts:
        print(f"no contract found under {WORKSPACE}", file=sys.stderr)
        return 2

    checked = agreed = 0
    disagreements: list[str] = []

    for name, source in contracts:
        if not source.exists():
            print(f"skip {name}: no {source}")
            continue
        text = source.read_text()
        module_root = anchor(source) / declared_root(text)
        if not module_root.is_dir():
            print(f"skip {name}: no module root at {module_root}")
            continue

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
