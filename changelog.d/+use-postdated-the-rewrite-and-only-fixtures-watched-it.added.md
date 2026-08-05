`use` is the one law newer than `tools/differential.py`: it has no Python twin
in the implementation this was rewritten from, so the parity gate that catches
the other six laws disagreeing with their old selves had nothing to check it
against. It was pinned by hand-written fixtures alone — real coverage, but a
fixed handful of shapes next to a gate built to survive a person breaking a
contract in every way at once.

`tests/properties.rs` now grows randomized packages with real outside
imports, drafts a randomized grant table over them — some modules ungranted,
some unscoped, some scoped to a random subset of the package's zones — and
hand-computes which imports the law should refuse, independently of the code
under test. It plays the same role `the_cycle_law_finds_what_a_slower_algorithm_finds`
already played for `cycle`: an oracle that shares no logic with the judge,
run at a scale a fixture file cannot reach. `use` is load-bearing in CI on the
same footing as the other six now.
