<!--
Thanks for sending this. Delete any section that does not apply rather than
writing "n/a" in all of them - a short, honest PR body beats a filled-in form.
CONTRIBUTING.md has the long version of everything below.
-->

## What changed

<!-- One or two sentences in the voice of the change. What is different now? -->

## Why

<!-- The problem, not the patch. If there is an issue, link it. -->

## What proves it

<!--
The question review asks first. Name the fixture, the property/fuzz seed, or
the differential run - and what it would have done before this change.
"Existing tests pass" is not proof that a new verdict is right.

Changed what a verb reports? Show it on a tree: which files moved zones, which
violation appeared or disappeared, and why.
-->

## Does it still read the tree, not a cache

<!--
zoning has no persisted graph and no project model to keep in step - every run
reads the tree fresh. If this change makes a run faster by trusting something
persisted between invocations, say so explicitly: that is a change to what the
tool promises, not an optimization.
-->

## What it costs

<!--
Allocation, another pass over the tree, a slower cold path, a wider LSP
surface. If the answer is genuinely nothing, say so - that is an answer.
-->

## What it replaces

<!--
If a newer path supersedes an older one, the older one should be gone in this
same PR. Two spellings of the same thing is how a codebase grows two spellings
of the same bug.
-->

---

- [ ] `cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` all
      pass, and the release binary still judges `tests/fixtures/{pass,fail}`
      correctly
- [ ] `python3 tools/differential.py` agrees with `ward` on every fixture, if
      the change touches a law both implementations share
- [ ] A news fragment is in `changelog.d/` (`+<slug>.<type>.md`), unless this
      is comment-only, format-only, or genuinely invisible
- [ ] Every failure this PR introduces names its remedy in the diagnostic text
- [ ] A new law was not added; a new `Dialect` did not resolve a path its own
      way
