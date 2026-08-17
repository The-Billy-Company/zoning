"""Cases where the right answer is known before any tool is asked.

A speed table only compares tools that agree. These cases are where they don't,
and each one's expected verdict is derived from the language reference — what
Python *does* when the import runs — rather than from whatever this repository's
gate happens to print today. That direction matters: a case whose expectation was
captured from our own output could never catch us being wrong, which is the only
thing a correctness rung is for.

Each case is a directory holding `case.json` and a `tree/`. The tree is a whole
tiny package with its own `.zone` contract, plus a rival config wherever the rule
is expressible in that rival's language. Where it isn't, the case says so and the
rival is reported `n/a` — a gate we can state and they cannot is a real
difference, and pretending they declined to answer would be a lie in our favour.

Two fields shape the gate. `gate: false` marks a case whose *correct* answer is
contested or is a blind spot every static tool shares (a runtime
`importlib.import_module`, say): it is measured and reported and never fails the
run, because failing on it would just pressure someone into deleting the case.
`tools` lists who can be asked at all.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from rivals import Launcher, Run, judge

HERE = Path(__file__).resolve().parent
CASES = HERE / "cases"


@dataclass(frozen=True)
class Case:
    """One committed tree and the verdict the language says it deserves."""

    name: str
    title: str
    rule: str
    expect: str  # "clean" or "violation"
    why: str
    tools: list[str]
    gate: bool
    tree: Path


def load() -> list[Case]:
    """Every case on disk, in name order, found rather than listed."""
    found = []
    for spec in sorted(CASES.glob("*/case.json")):
        data = json.loads(spec.read_text(encoding="utf-8"))
        found.append(
            Case(
                name=spec.parent.name,
                title=data["title"],
                rule=data["rule"],
                expect=data["expect"],
                why=data["why"],
                tools=data.get("tools", ["zoning"]),
                gate=data.get("gate", True),
                tree=spec.parent / "tree",
            )
        )
    return found


def ask(case: Case, tools: dict[str, Launcher]) -> dict[str, Run]:
    """Every tool's verdict on one case, with `absent` for the ones not asked."""
    answers: dict[str, Run] = {}
    for key, tool in tools.items():
        if key not in case.tools:
            answers[key] = Run("absent", 0.0, "rule not expressible")
        else:
            answers[key] = judge(tool, case.tree)
    return answers


def agrees(case: Case, run: Run) -> str:
    """How one verdict stands against the case's expectation.

    `error` never becomes a disagreement about semantics: a tool that failed to
    start has not disagreed with anything, and scoring it as wrong would hide a
    broken adapter behind a plausible-looking correctness result.
    """
    if run.verdict == "absent":
        return "n/a"
    if run.verdict == "error":
        return "error"
    return "agrees" if run.verdict == case.expect else "MISSES"
