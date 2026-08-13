# bench

Race this gate against the ones people already use, and check that it is right.

```bash
cargo build --release                        # the bench refuses a debug binary
python3 bench/bench.py                       # all three rungs
python3 bench/bench.py --only correctness    # no rivals needed, nothing downloaded
python3 bench/bench.py --json                # one record, for a script
```

Three rungs, all quick enough to run while you read the output:

| Rung | Question | Gates? |
|---|---|---|
| `correctness` | on cases the language reference decides, who is right? | yes, on us |
| `speed` | same package, same rule, three tools - how long? | no |
| `compiler` | Zig has no rival; does `zig` itself say anything? | yes, on us |

## What can fail

Only a case where **we** miss something the language says we should catch. A
rival disagreeing is reported and never fatal - this is our ratchet, not a
lawsuit against theirs. Speed never gates, because a laptop under load is not
evidence.

That split is the whole design. A benchmark that fails when a rival ships a
release is a benchmark someone eventually deletes.

## The rule that keeps the numbers honest

A tool that **failed to run** is never scored as a tool that **found a
violation**. Both exit non-zero. A moved flag, a config the current release
parses differently, a missing interpreter: each of those is `error`, reported
with the first line the tool actually said, and never quietly folded into a win
for anybody. `rivals.py` classifies verdicts for exactly this reason instead of
testing `returncode == 0`.

The same care runs the other way. Where a rival cannot express a rule at all it
is reported `n/a` with a note in its own terms, not as a rival that declined to
answer. `bench/cases/README.md` has the two cases where that applies and why.

## Speed, and why the corpus is clean

`corpus.py` generates a layered package - N stacked subpackages, M files each,
every file importing one file from the layer beneath - and writes the same rule
three times: a `.zone` contract, an `.importlinter` layers contract, and
`tach.toml` modules with explicit dependencies. Nothing is committed; it is built
in a temporary directory and resized with `--zones` and `--modules`.

The corpus has **no violation in it**, on purpose. A dirty corpus measures how
fast a tool can give up, and the first violation might be one file in, so the
number would say more about directory ordering than about the tool. A clean graph
has no early exit: to answer "yes" every tool has to read every file and resolve
every edge. That is the honest worst case, and it is also the case CI pays on
every green run.

Every tool gets one untimed warm-up, then `--reps` timed runs, best of them.
Interpreter startup is inside the measurement, because that is the cost a
pre-commit hook actually pays; subtracting it would measure an algorithm nobody
can invoke. Rivals are taken from `PATH` first so their numbers carry no wrapper
we imposed - reached through `uv` the row says so, and the note under the table
repeats it.

## Zig

There is no rival to race, so the comparison is against the toolchain everybody
already runs. The fixture is `tests/fixtures/fail/knot`, reused rather than
copied: a real cross-directory cycle that the Rust suite already judges.

The rung reports what `zig build-obj` did instead of asserting it. Zig's analysis
is lazy, its handling of a mutual `@import` is its own business, and a future
release may change its mind - a bench that hard-coded "the compiler stays quiet"
would start failing on a day when Zig got *better*. So the compiler's answer is
recorded as `quiet` or `complained`, and `quiet` is deliberately not spelled
`clean`: it was never asked about architecture and never claimed the tree was
well shaped. It compiled. That is all it said.

## Adding a case

Drop a directory in `cases/` with a `case.json` and a `tree/`. The one rule is
that the expectation comes from the language, cited in `spec`, and not from our
own output - a case captured from what we print today could never catch us being
wrong tomorrow. `cases/README.md` has the fields.
