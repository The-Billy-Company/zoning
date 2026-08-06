# Fixtures

One tiny package per law, so the suite proves each law both fires and stays
quiet without depending on any real repository being checked out beside this
one.

- `pass/` — packages whose code obeys their contract. `zone verify` exits 0.
- `fail/` — packages that break exactly one thing. `zone verify` exits 1, and
  the integration test in `tests/laws.rs` asserts *which* law fired, because a
  gate that fails for the wrong reason is a gate that will pass for the wrong
  reason too.

Each box is a whole package: a `<name>.zone` contract beside a `src/` tree. They
are deliberately a handful of files each. A fixture large enough to be
interesting is a fixture nobody re-reads when it breaks.

Most of them keep their contract in a `contract/` drawer, which is no longer
where a new one goes and is exactly why they still do: the drawer is the layout
that shipped first, and leaving these there means every law in the suite is
proven over it on every run rather than by one dedicated test.

`pass/kin/` is the other layout and the workspace, in one box. `kin.zone` holds
a `workspace` block and no package, so it governs nothing itself; `hearth/`
inherits every setting and its `package` line carries no block at all, while
`lantern/` overrides the one setting it disagrees with. The grant `kin.zone`
shares is exercised by `hearth` alone, which is the point — a shared grant is
judged against the whole membership, so one member using it keeps it alive for
all of them. `bind.zone` is a real DNS zone file sitting in the same directory,
and a sweep must walk straight past it.

`--untracked` is required when running these by hand: they are checked into
this repository, but their `src/` trees are not what `git ls-files` would call
the module under judgment when the box is copied to a scratch directory.
