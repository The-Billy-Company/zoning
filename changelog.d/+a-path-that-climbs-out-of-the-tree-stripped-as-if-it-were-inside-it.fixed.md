Resolving `--under` as a place rather than a spelling left one spelling still
unresolved: a path that climbs back out. `join` does not fold `..` away and
`canonicalize` refuses a path nobody created, so `--under ../elsewhere` kept the
tree's own root as a literal prefix and stripped clean through it — landing back
in the same failure the last fix was about, a gate aimed outside the tree
answering "clean" rather than "not here".

Both sides of that comparison now resolve by one rule: the deepest ancestor that
exists is resolved, which is what makes a symlinked `/tmp` compare equal to its
target, and whatever is left is folded lexically, which is what keeps a sibling
of the tree outside it.

Worth saying where it hid, because the test for it was already written and
passing: macOS resolves its temp directory's `/var` to `/private/var`, so the
fixture's prefixes disagreed for a reason that had nothing to do with the climb,
and the assertion held on the one platform anybody runs by hand while the bug
shipped on Linux. The test now also asks of a tree that is not reached through a
symlink, which is the only place the question is a real one.
