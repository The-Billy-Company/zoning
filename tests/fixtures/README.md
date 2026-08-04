# Fixtures

One tiny package per law, so the suite proves each law both fires and stays
quiet without depending on any real repository being checked out beside this
one.

- `pass/` — packages whose code obeys their contract. `zoning verify` exits 0.
- `fail/` — packages that break exactly one thing. `zoning verify` exits 1, and
  the integration test in `tests/laws.rs` asserts *which* law fired, because a
  gate that fails for the wrong reason is a gate that will pass for the wrong
  reason too.

Each box is a whole package: `contract/<name>.zone` beside a `src/` tree. They
are deliberately a handful of files each. A fixture large enough to be
interesting is a fixture nobody re-reads when it breaks.

`--untracked` is required when running these by hand: they are checked into
this repository, but their `src/` trees are not what `git ls-files` would call
the module under judgment when the box is copied to a scratch directory.
