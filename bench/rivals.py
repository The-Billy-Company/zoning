"""The tools this benchmark races, and how each one is asked the same question.

Every gate here answers one thing — *does this package's import graph obey its
declared shape* — and answers it with an exit code. So the adapter for each tool
is small on purpose: find it, run it in the tree, read the code.

The one rule that makes the numbers trustworthy is that a tool which *failed to
run* is never scored as a tool that *found a violation*. Both exit non-zero. A
misspelled config, a version whose flags moved, a missing interpreter — each of
those is `error`, reported with the first line the tool said, and it is never
quietly folded into a win for anybody. That distinction is the whole reason this
file classifies verdicts instead of testing `returncode == 0`.

Reading the output for a failure is the last net rather than the first, because
the most likely way to race a tool that is not there is a launcher standing in
its place — a shim whose complaint looks nothing like a crash and exits the same
`1` a real violation does. So every tool has to say its own version first, and say
it *in the tree it is about to judge*, since a shim resolves per directory: the
launcher that answered in this repository may be standing in front of nothing in a
temporary corpus. A tool that will not identify itself is never raced, and never
scored.

Rivals are reached the way the rest of this repository reaches a Python tool it
does not depend on: whatever is already on `PATH`, else `uv run --no-project
--with`. Versions are floors rather than pins, and whatever actually resolved is
recorded in the report — a benchmark that hides which build it raced is a
benchmark you cannot repeat.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent

# Version floors, not pins: a rival that shipped a fix last week should be raced
# with the fix. `probe` records what resolved, so the report still says which
# build produced each number.
SPEC = {
    "import-linter": "import-linter>=2",
    "tach": "tach>=0.29",
}

# What each rival's own binary is called, and how to ask it its version.
CONSOLE = {"import-linter": "lint-imports", "tach": "tach"}

LABEL = {
    "zoning": "zoning",
    "import-linter": "import-linter",
    "tach": "tach",
}

TIMEOUT = 120


@dataclass(frozen=True)
class Launcher:
    """One tool, resolved to something runnable, with how it was found."""

    key: str
    argv: list[str]
    how: str  # "path", "uv", or "built"
    version: str

    @property
    def label(self) -> str:
        via = "" if self.how in {"path", "built"} else f" (via {self.how})"
        return f"{LABEL[self.key]} {self.version}{via}"


@dataclass
class Run:
    """One judgment: what the tool concluded, and how long it took to conclude it."""

    verdict: str  # "clean", "violation", "error", or "absent"
    seconds: float
    detail: str = ""


def zone() -> Launcher | None:
    """Our own binary — a release build if there is one, else whatever is installed.

    A debug build is refused rather than silently raced: it runs several times
    slower than the artifact anyone ships, so a number from one is not a number
    about zoning, and publishing it would understate the tool by a factor nobody
    reading the table could see.
    """
    built = REPO / "target" / "release" / "zoning"
    if built.exists() and (version := speak([str(built)])[0]):
        return Launcher("zoning", [str(built)], "built", version)
    found = shutil.which("zone") or shutil.which("zoning")
    if found and (version := speak([found])[0]):
        return Launcher("zoning", [found], "path", version)
    return None


def rival(key: str, *, allow_uv: bool = True) -> Launcher | None:
    """A rival, from `PATH` first so its number carries no wrapper we imposed.

    A `PATH` hit that will not say its version is not a rival, it is whatever was
    installed where the rival should have been — so the search continues past it to
    `uv` rather than racing it. Preferring a working wrapper over a broken direct
    hit is the only order that cannot silently drop a rival from the table.
    """
    found = shutil.which(CONSOLE[key])
    if found and (version := speak([found])[0]):
        return Launcher(key, [found], "path", version)
    if allow_uv and shutil.which("uv"):
        argv = ["uv", "run", "--no-project", "--with", SPEC[key], CONSOLE[key]]
        if version := speak(argv)[0]:
            return Launcher(key, argv, "uv", version)
    return None


def speak(argv: list[str], cwd: Path | None = None) -> tuple[str | None, str]:
    """What the tool calls itself, and the first line it said trying to answer.

    `None` is "it would not identify itself": a non-zero exit, or a last token with
    no digit in it. Every tool raced here answers `--version` with a version, and a
    launcher standing in for one that is not installed answers with its own
    complaint instead. That difference is the only way to tell a tool from a shim
    without naming a single launcher — and naming them is a list that is wrong the
    day somebody uses a launcher nobody here has heard of.
    """
    try:
        done = subprocess.run(
            [*argv, "--version"],
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
            cwd=cwd,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as broke:
        return None, str(broke)
    said = first(f"{done.stdout}\n{done.stderr}")
    token = said.split()[-1] if said.split() else ""
    if done.returncode != 0 or not any(ch.isdigit() for ch in token):
        return None, said
    return token, said


# One answer per tool per tree, since establishing it costs a process and the
# speed rung asks the same pair over and over.
_SPOKEN: dict[tuple[str, str], str] = {}


def stumbled(tool: Launcher, root: Path) -> str:
    """Empty if `tool` runs in `root`, else the line it said instead of a version.

    Asked per tree rather than once, because a shim resolves per directory and a
    tool that cannot start exits the same `1` a real violation does. Without this
    the whole table inverts quietly: a launcher with nothing behind it agrees with
    every case that expects a violation, and its time measures the launcher giving
    up rather than the tool doing the work.
    """
    key = (tool.key, str(root))
    if (seen := _SPOKEN.get(key)) is None:
        version, said = speak(tool.argv, cwd=root)
        seen = "" if version else (said or "said nothing when asked its version")
        _SPOKEN[key] = seen
    return seen


def judge(tool: Launcher, root: Path) -> Run:
    """Run one tool over one tree, timed, and classify what it concluded.

    Timed with `perf_counter` around the whole process, interpreter startup
    included, because that is the cost a pre-commit hook or a CI job actually
    pays. A comparison that subtracted it would be measuring an algorithm nobody
    can invoke.

    A tool that cannot start *here* is reported as one before anything is timed, so
    no launcher's complaint is ever priced or scored as an opinion about the tree.
    """
    if broke := stumbled(tool, root):
        return Run("error", 0.0, broke)
    argv = argv_for(tool, root)
    env = dict(os.environ)
    # import-linter imports the root package to find it, so the tree it is
    # judging has to be importable from where we start it.
    env["PYTHONPATH"] = str(root)
    env.pop("PYTHONDONTWRITEBYTECODE", None)
    start = time.perf_counter()
    try:
        done = subprocess.run(
            argv,
            cwd=root,
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
            env=env,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return Run("error", float(TIMEOUT), f"no verdict in {TIMEOUT}s")
    except OSError as broke:
        return Run("error", time.perf_counter() - start, str(broke))
    spent = time.perf_counter() - start
    said, detail = verdict(done.returncode, done.stdout, done.stderr)
    return Run(said, spent, detail)


# The flags that turn each tool into "judge the tree I am standing in". `zone`
# needs `--untracked`, because neither a generated corpus nor a fixture committed
# to *this* repository is what `git ls-files` would call the module under
# judgment.
ARGS = {
    "zoning": ["verify", "--untracked", "--no-color"],
    "import-linter": [],
    "tach": ["check"],
}


def argv_for(tool: Launcher, root: Path) -> list[str]:
    """The full command line, including the one flag that cannot be omitted.

    `zone` gets an explicit `--root`. Given none, it climbs to the enclosing git
    worktree to find the package it should judge — and every case tree here is
    nested inside this repository, which declares no contract of its own. So the
    sweep would find nothing, exit 0, and report every case as clean: the oracle
    would invert silently and read as a rival's win. The same trap is why this
    repository's own CI names `--root` for each fixture box.
    """
    if tool.key != "zoning":
        return [*tool.argv, *ARGS[tool.key]]
    return [*tool.argv, *ARGS["zoning"], "--root", str(root)]


# Text that means the tool never got as far as an opinion, whatever it exited
# with. Kept small and specific: a broad match here would launder a real
# violation into "error" and quietly award the run to whoever crashed.
BROKEN = (
    "Traceback (most recent call last)",
    "Usage:",
    "No such file or directory",
    "could not be found",
    "is not a valid",
    "Could not find",
    "error: unexpected argument",
    "unknown option",
)


def verdict(code: int, out: str, err: str) -> tuple[str, str]:
    """What the exit code meant, and the first line worth quoting.

    Exit 1 is "I looked and it is wrong" for all three tools; exit 0 is "I looked
    and it is fine". Anything else — or a recognizable failure-to-start in the
    output — is the tool not answering, which is a different fact and is reported
    as one.
    """
    text = f"{out}\n{err}"
    if code not in {0, 1} or any(mark in text for mark in BROKEN):
        return "error", first(text)
    return ("clean", "") if code == 0 else ("violation", first(out) or first(err))


def first(text: str) -> str:
    """The first line with something on it, trimmed to fit a table cell."""
    for line in text.splitlines():
        if line.strip():
            return line.strip()[:88]
    return ""
