A property and fuzz suite, with no test framework behind it. The fixtures prove each law
fires on the tree built to break it, which is the right test for a law and the wrong one
for a claim about *every* graph - and it was a claim about every graph that was false. So
there are now five properties over generated packages: a drafted contract is true of the
graph it came from, a tangle is never drafted into silence, the cycle law finds what a
slower O(n^3) reachability oracle finds, a verdict does not move when the walk order is
stirred, and no finding exists that no file's `explain` can show. Plus four fuzz targets
over the two places bytes arrive from outside - a hand-written contract and a source tree -
because a gate that panics is indistinguishable from a broken build and sends its reader
looking in the wrong repository.

There is no `proptest` here for the same reason there are no dependencies anywhere else: a
gate that runs in everyone's CI should be a static binary you can audit in an afternoon.
What a property test actually needs is a generator, a deterministic seed, and invariants
worth asserting, and none of that requires a framework. `ZONING_SEED` replays a failure
exactly and `ZONING_CASES` turns the same code into a soak, which a scheduled job now runs
at fifty times the per-commit case count with a fresh seed each time, so the search keeps
covering ground the last run did not.

The oracle is the part worth stealing. It shares no code with the law it checks: transitive
closure by relaxation to a fixed point, then mutual reachability as the equivalence
relation. Far too slow to ship, which is exactly what makes it a trustworthy second
opinion, and it disagreed with Tarjan on the thirteenth generated package.
