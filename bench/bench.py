#!/usr/bin/env python3
"""Race this gate against the ones people already use, and check it is right.

Two rungs, both quick enough to run while you read the output:

    correctness   committed cases whose answer the language reference decides,
                  asked of every tool that can express the rule at all
    speed         one clean layered package, judged by every tool, best of N

Only the correctness rung can fail the run, and only when *we* miss a case the
language says we should catch. A rival disagreeing is reported, never fatal: this
is our gate's ratchet, not a lawsuit against theirs. Speed is reported and never
gates, because a machine under load is not evidence.

Usage:
    bench/bench.py                          both rungs, default corpus
    bench/bench.py --only correctness        just the oracle (no rivals needed)
    bench/bench.py --zones 20 --modules 25   a 500-module corpus
    bench/bench.py --json                    one record, for a script
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import compiler  # noqa: E402
import corpus  # noqa: E402
import oracle  # noqa: E402
import rivals  # noqa: E402

RIVALS = ["import-linter", "tach"]
RUNGS = ("correctness", "speed", "compiler")


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--only", choices=RUNGS, help="run one rung")
    ap.add_argument("--zones", type=int, default=10, help="layers in the speed corpus")
    ap.add_argument("--modules", type=int, default=10, help="files per layer")
    ap.add_argument("--reps", type=int, default=3, help="timed runs per tool, best wins")
    ap.add_argument("--no-uv", action="store_true", help="only race rivals already on PATH")
    ap.add_argument("--json", action="store_true", help="one JSON record instead of tables")
    args = ap.parse_args()

    ours = rivals.zone()
    if ours is None:
        print(
            "no zoning binary — build one with `cargo build --release`, "
            "or install the wheel so `zone` is on PATH",
            file=sys.stderr,
        )
        return 2

    tools = {"zoning": ours}
    for key in RIVALS:
        found = rivals.rival(key, allow_uv=not args.no_uv)
        if found:
            tools[key] = found

    began = time.perf_counter()
    report: dict[str, object] = {"tools": {k: t.label for k, t in tools.items()}}
    missed = 0

    if args.only in (None, "correctness"):
        rows, missed = correctness(tools)
        report["correctness"] = rows
    if args.only in (None, "speed"):
        report["speed"] = speed(tools, args)
    if args.only in (None, "compiler"):
        zig = compiler.run(ours)
        report["compiler"] = zig
        missed += 0 if zig["gated"] else 1
    report["seconds"] = round(time.perf_counter() - began, 2)

    if args.json:
        print(json.dumps(report, indent=2))
    else:
        render(report, tools)
    return 1 if missed else 0


def correctness(tools: dict[str, rivals.Launcher]) -> tuple[list[dict[str, object]], int]:
    """Ask every tool every case. Returns the rows, and how many *we* missed."""
    rows: list[dict[str, object]] = []
    missed = 0
    for case in oracle.load():
        answers = oracle.ask(case, tools)
        stood = {key: oracle.agrees(case, run) for key, run in answers.items()}
        if case.gate and stood.get("zoning") == "MISSES":
            missed += 1
        rows.append(
            {
                "case": case.name,
                "title": case.title,
                "expect": case.expect,
                "gate": case.gate,
                "why": case.why,
                "verdicts": {k: run.verdict for k, run in answers.items()},
                "stood": stood,
                "detail": {k: run.detail for k, run in answers.items() if run.detail},
            }
        )
    return rows, missed


def speed(tools: dict[str, rivals.Launcher], args: argparse.Namespace) -> dict[str, object]:
    """Build one corpus, judge it with everything, best of `--reps` per tool.

    Every tool gets one untimed warm-up first. All three cache something — a
    resolved dependency set, a module graph, a `__pycache__` — and timing a cold
    first run against a warm second one would just be measuring whose cache got
    filled by whom.
    """
    with tempfile.TemporaryDirectory(prefix="zoning-bench-") as tmp:
        root = Path(tmp)
        files = corpus.build(root, zones=args.zones, modules=args.modules)
        rows: dict[str, object] = {}
        for key, tool in tools.items():
            rivals.judge(tool, root)  # warm-up, deliberately unmeasured
            runs = [rivals.judge(tool, root) for _ in range(max(1, args.reps))]
            said = {run.verdict for run in runs}
            rows[key] = {
                "verdict": runs[0].verdict if len(said) == 1 else "unstable",
                "best": round(min(run.seconds for run in runs), 4),
                "median": round(statistics.median(run.seconds for run in runs), 4),
                "detail": next((run.detail for run in runs if run.detail), ""),
            }
        return {
            "files": files,
            "zones": args.zones,
            "modules": args.modules,
            "reps": max(1, args.reps),
            "tools": rows,
        }


def render(report: dict[str, object], tools: dict[str, rivals.Launcher]) -> None:
    """The tables, plus the caveats a reader needs in order to trust them."""
    print("tools")
    for key, label in report["tools"].items():  # type: ignore[union-attr]
        print(f"  {key:<14} {label}")

    rows = report.get("correctness")
    if rows:
        print("\ncorrectness — expectation comes from the language, not from any tool")
        header = f"  {'case':<16} {'expect':<10} " + " ".join(f"{k:<14}" for k in tools)
        print(header)
        for row in rows:  # type: ignore[union-attr]
            stood = row["stood"]
            mark = "" if row["gate"] else "  (informational)"
            cells = " ".join(f"{stood.get(k, '?'):<14}" for k in tools)
            print(f"  {row['case']:<16} {row['expect']:<10} {cells}{mark}")
        for row in rows:  # type: ignore[union-attr]
            if row["stood"].get("zoning") == "MISSES" and row["gate"]:
                print(f"\n  MISSED {row['case']}: {row['why']}")

    fast = report.get("speed")
    if fast:
        best = min(
            (r["best"] for r in fast["tools"].values() if r["verdict"] != "error"),  # type: ignore[index]
            default=0.0,
        )
        shape = f"{fast['files']} files, {fast['zones']} layers"  # type: ignore[index]
        print(f"\nspeed — {shape}, clean graph, best of {fast['reps']} runs")  # type: ignore[index]
        print(f"  {'tool':<14} {'verdict':<12} {'best':>9} {'median':>9} {'ratio':>8}")
        for key, row in fast["tools"].items():  # type: ignore[index]
            ratio = f"{row['best'] / best:.1f}x" if best and row["verdict"] != "error" else "-"
            print(
                f"  {key:<14} {row['verdict']:<12} {row['best']:>8.3f}s "
                f"{row['median']:>8.3f}s {ratio:>8}"
            )
            if row["detail"]:
                print(f"  {'':<14} {row['detail']}")
        if any(t.how == "uv" for t in tools.values()):
            print("\n  note: a rival reached via uv carries uv's wrapper in its number.")
            print("  install it on PATH for a clean comparison.")

    zig = report.get("compiler")
    if zig:
        print(f"\nzig — one real cross-directory cycle in {zig['fixture']}")  # type: ignore[index]
        ours, theirs = zig["zoning"], zig["zig"]  # type: ignore[index]
        print(f"  {'zoning':<14} {ours['verdict']:<12} {ours['seconds']:>8.3f}s")
        print(f"  {'zig build-obj':<14} {theirs['verdict']:<12} {theirs['seconds']:>8.3f}s")
        if theirs["detail"]:
            print(f"  {'':<14} {theirs['detail']}")
        if not zig["gated"]:  # type: ignore[index]
            print("\n  MISSED: the cycle is in the fixture and zone verify did not report it")

    print(f"\n{report['seconds']}s total")


if __name__ == "__main__":
    sys.exit(main())
