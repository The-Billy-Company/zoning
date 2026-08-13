#!/usr/bin/env python3
"""The Zig rung, where the incumbent is the compiler and it has no opinion.

Zig has no import-topology tool to race, so the useful comparison is against the
toolchain everybody already runs: `zig` itself. The fixture is a genuine
cross-directory cycle — `src/a/a.zig` imports `../b/b.zig`, which imports back —
and the question is whether compiling it says anything about that.

This rung reports what the compiler did rather than asserting it. Zig's semantic
analysis is lazy, its treatment of a mutual `@import` is its own business, and a
future release is entitled to change its mind; a bench that hard-coded "the
compiler stays quiet" would start failing on a day when Zig got *better*, which
is the wrong direction for a gate to fire in. Only our own verdict is gated: the
cycle is really there, so `zone verify` has to find it.

The fixture is `tests/fixtures/fail/knot`, reused rather than copied. It is
already committed, already a real cycle, and already what the Rust integration
suite judges — a second copy here would be a second thing to keep in step.
"""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import time
from pathlib import Path

from rivals import Launcher, judge

HERE = Path(__file__).resolve().parent
KNOT = HERE.parent / "tests" / "fixtures" / "fail" / "knot"
TIMEOUT = 180


def run(ours: Launcher) -> dict[str, object]:
    """Both answers about one cycle: ours, and the compiler's."""
    mine = judge(ours, KNOT)
    return {
        "fixture": str(KNOT.relative_to(HERE.parent)),
        "zoning": {"verdict": mine.verdict, "seconds": round(mine.seconds, 4)},
        "zig": compile_it(),
        # The gate: the cycle exists, so the only acceptable verdict is that it
        # was found. What zig said is recorded beside it, never gated on.
        "gated": mine.verdict == "violation",
    }


def compile_it() -> dict[str, object]:
    """Ask `zig` to build the same tree, and report what it made of it.

    `build-obj` rather than `build-exe`: the fixture is a library-shaped tree with
    no `main`, so asking for an executable would fail for a reason that has
    nothing to do with the cycle, and the number would be an artefact of the
    fixture's shape instead of a fact about Zig.
    """
    zig = shutil.which("zig")
    if not zig:
        return {"verdict": "absent", "seconds": 0.0, "detail": "zig is not installed"}
    with tempfile.TemporaryDirectory(prefix="zoning-bench-zig-") as tmp:
        argv = [zig, "build-obj", "root.zig", f"-femit-bin={Path(tmp) / 'knot.o'}"]
        start = time.perf_counter()
        try:
            done = subprocess.run(  # noqa: S603
                argv,
                cwd=KNOT / "src",
                capture_output=True,
                text=True,
                timeout=TIMEOUT,
                check=False,
            )
        except (OSError, subprocess.SubprocessError) as broke:
            return {"verdict": "error", "seconds": 0.0, "detail": str(broke)}
        spent = time.perf_counter() - start
    quiet = done.returncode == 0
    return {
        # "quiet" is not "clean": the compiler was not asked about architecture and
        # did not claim the tree was well shaped. It compiled, and that is all.
        "verdict": "quiet" if quiet else "complained",
        "seconds": round(spent, 4),
        "detail": "" if quiet else first(done.stderr),
    }


def first(text: str) -> str:
    for line in text.splitlines():
        if line.strip():
            return line.strip()[:88]
    return ""
