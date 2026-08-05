# Changelog

All notable changes to `zoning` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions track the
workspace's `Cargo.toml`.

<!-- towncrier release notes start -->

## [1.1.0] - 2026-08-05

### Added

- A property and fuzz suite, with no test framework behind it. The fixtures prove each law
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
- `.zone` is now a first-class editor language: one installed `zoning` executable
  ships the LSP, a crisp SVG identity, safe first-run setup, and adapters for
  Cursor, VS Code, Zed, Neovim, and Vim.

### Fixed

- The `setup` fake-HOME test named its scratch directory after the current
  test-thread name to keep parallel runs from colliding, and Rust spells that
  name `setup::tests::fake_home_install_repair_and_uninstall_are_owned` - a
  path segment with three colons in it. Unix shrugs; Windows refuses to open a
  directory whose name isn't a legal filename, so every `cargo test --release`
  on `windows-latest` failed before the real assertions even ran. The thread
  name is now sanitized (`:` -> `_`) before it becomes a path component, which
  still keeps concurrent test runs apart without asking Windows to accept a
  volume-label character in the middle of a directory name.
- The cycle law was blind to most real cycles, and a property test found it in about a
  second. `forbid cycles across directories` withheld same-directory imports from the graph
  *before* searching it, which severs any tangle whose trip home goes through a neighbour:
  `a/one.zig -> a/two.zig -> b/three.zig -> a/one.zig` binds `a` and `b` into one
  indivisible unit exactly as tightly as a two-file cycle does, and zoning reported nothing.
  Crossing a boundary is a property of the cycle, not of the individual imports in it, so
  the filter moved off the edges and onto the component: search the whole graph, then keep
  the components that bind more than one module. A cycle wholly inside a directory - or
  inside a directory and the door file named for it - is still that module's own business.

  `draft` had been contradicting itself in one breath because of this. It would merge two
  directories into a single zone with the note "these 2 directories import each other, so no
  order separates them", then close with "Nothing else to declare: this graph is already a
  stack". The zone stack reads the directory graph and saw the tangle; the tangle detector
  dropped the edge that proved it. Now a draft over a graph with a real cycle emits the
  variance stanza with an empty reason, which does not parse, which is the whole point.

  Adopting this is not free, and it should not be: it turns previously-silent tangles into
  findings. In our own trees it surfaced five in a 310-file package - two nobody knew about,
  and three that existing variances had described with too few members - and one in another.
  Every one of them was real before this release; the tool simply could not see it. A
  variance whose member list is now short fails as stale rather than passing quietly, so the
  contract gets corrected rather than left subtly wrong.


## [1.0.0] - 2026-08-04

### Added

- A contract now names its own language - `language zig` in the package block - so a
  monorepo holding Zig beside Python is one run, not one run per dialect with a flag
  apiece. `--dialect` survives as `--language` and means what it always should have: the
  default for a package that has not said.

  Discovery went recursive and stopped needing to be told anything. `contract/*.zone` is
  found at any depth, so a repository that *is* one package and a monorepo burying them
  at `libs/kernels/<pkg>/contract/` are the same invocation. It asks git first: reading
  every directory of a large monorepo cost seconds - seven, measured on one of them - and
  a gate that slow stops being run, which is a correctness problem wearing a performance
  costume. One `git ls-files` covers tracked *and* untracked-but-not-ignored files in about
  forty milliseconds, so a contract written a moment ago is found and one sitting in an
  ignored directory is correctly invisible. Outside a worktree the walk still answers.

  Scope is where you are standing. A gate that answers about the whole repository no
  matter which directory you invoke it from cannot be used *inside* one package, and in a
  monorepo it is also the slow answer, since it reads every other package to tell you
  about yours. A directory with nothing governed beneath it looks up for the package that
  encloses it; an explicit `--root` never climbs, because a caller who names the subtree
  means it, including when the answer is "nothing here". At a repository root, which is
  where CI stands, the two models are the same run.

  `zoning list` stopped reporting only the packages that already have a contract - a list
  of the finished work cannot tell you whether you are finished. It reports every package
  in the tree with the next command for each, and it distinguishes three states that used
  to look identical: ungoverned (a gap, with the `draft` line to close it), vendored (a
  dependency another package's manifest declares - governed upstream, not yours), and no
  module at all (a manifest with no source of its own, so there is nothing for a contract
  to say). A one-file package says so too, because five of the seven laws are claims about
  how files sit relative to each other and a single file has no relatives.

  Also: `keep <glob> to nobody`, for the directory whose whole point is that nothing
  imports it.
- A property and fuzz suite, with no test framework behind it. The fixtures prove each law
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
- Every boundary tool is easy to love on a greenfield package and miserable to adopt on
  a real one. A tree with nine hundred files has an architecture already - it is simply
  undeclared - and the first contract somebody writes for it arrives red, which teaches
  the reader exactly one lesson: the gate is noise. Two new verbs are the answer.

  `zoning draft <dir>` writes the contract the graph already obeys. Zones come out of a
  topological sort over directories, so nothing points up the page; grants come out of
  the modules the code is already importing; the reach ceiling is the reach the tree
  actually needs today. The first `verify` is green, and then every step of the cleanup -
  merge two zones, seal a directory, drop a grant, lower the ceiling - is a decision
  somebody made on purpose instead of a fight with a wall.

  It refuses to guess at two things. Seals and keeps are *claims* ("this directory is a
  deep module", "these peers are independent"), and a machine inferring them from today's
  call sites would guess wrong the first time somebody adds a second legitimate caller.
  And a real import cycle comes out as a `variance` stanza with an empty reason, which
  does not parse - so a draft over a genuinely tangled package cannot be adopted until a
  person has written why each tangle stays.

  Directories that import each other cannot be ordered, so they land in one zone. That
  zone is called `tangle`. I tried naming it after its members first and got
  `folio_lex_quire_walk_press`, five names wearing one, growing with the knot. The
  comment above the row lists the members; the name's job is to say what the row is, and
  a zone nobody enjoys reading is a zone somebody eventually splits.

  `zoning explain FILE` answers where one file stands - its zone, its reach, its grants,
  who imports it, what a seal in front of it would mean. `zoning explain FROM TO` answers
  whether that one import would be legal, and names the clause that decides, whether or
  not the import exists yet. That second form is the one I reach for most: the question a
  person actually has is "may I write this line", asked before writing it, and every
  tool in this class makes you write the line and run the whole gate to find out.

  Paths are taken as typed, resolved against the shell's own directory - the way an
  editor tab has it, the way a stack trace has it. Making somebody translate into
  module-relative coordinates first is the friction that stops a diagnostic verb from
  being used at all. And when a path is real but unjudged, it says which of the four
  reasons applies: wrong extension, excluded by the contract, untracked by git, or
  genuinely outside. In a worktree ten agents are editing, "untracked" is the one that
  bites, and "no governed package owns this" would have read like a bug.

  Then I ran the flow I had just written into the README and it was wrong twice, so both
  are fixed. `zoning draft src` is the natural thing to type, because `src` is where the
  code is - and it used to succeed quietly, producing a package named `src`, filing its
  contract one directory too deep, and leaving every later `--package` spelled wrong.
  The package is the directory somebody *declared*, so pointing at the module root of an
  enclosing package now says exactly that, and hands you the invocation for the package.

  And a drafted contract now takes the name the package's own manifest gives it - `.name
  = .demo` in `build.zig.zon` - rather than the name of the directory it sits in. A
  package usually already has a name, everything downstream depends on it by that name,
  and a directory can be renamed without the package being. `list` reads the same field,
  so the listing, the drafted filename, the `package` block, and `--package` all agree
  before anybody has run anything.

  Three ways a draft can find no module also needed telling apart, because they call for
  opposite next moves: a directory holding only a manifest is complete as it stands, a
  directory whose files all belong to nested packages wants each of those drafted, and a
  directory of source this build cannot read is a dialect problem. That last one used to
  answer a Rust tree by naming `build.zig` - sending the reader to look for a file that
  was never the issue - and now points at `--language`.
- Six laws governed where an import may point *inside* a package, and said nothing at
  all about the ones that leave it. That was the bigger hole. A zone stack tells you
  `kernel/` sits under `surface/`; it does not tell you that a leaf three directories
  down started dialing an HTTP client, and in the languages this tool exists for, that
  is the dependency that ends up hardest to remove.

  So: `use <module> [by <zone>…]`. A named import the build resolves - `irregex`,
  `build_options`, later `requests` - now needs a grant, and the grant carries a scope,
  because "the CLI face may talk to the network" and "any file in this package may talk
  to the network" are different architectures that used to be spelled the same way.
  Grants go stale like everything else here: a `use` nobody exercises is a permission
  somebody forgot to withdraw, and it fails the run rather than sitting there.

  The standard library is exempt by construction, per dialect - `std`, `builtin`, `root`
  in Zig. Every zone has it, no zone chose it, and a contract that spent its lines
  declaring it would bury the handful of grants that are actually decisions.

  One ergonomic thing I got wrong the first time and want written down. The first
  implementation reported the law per import site, so a package importing `irregex` from
  a hundred and thirty files got a hundred and thirty findings for one missing line -
  technically accurate, and a report nobody would read. It also priced a single
  undeclared decision as a hundred and thirty violations in a burndown, which is the
  kind of number that makes people fix the wrong thing. It now groups by module and
  scope: one finding, the count riding along, the first site as the location so an editor
  still lands somewhere real.
- `zoning verify --complete` adds the one claim no law can make: every package in scope
  has a contract. Without it, a clean run says nothing whatsoever about the package
  somebody added last week, and adoption that cannot notice a new ungoverned package rots
  back toward zero one package at a time. It is behind a flag because it is a different
  question from the seven laws, and it belongs to a repository that has finished adopting
  rather than one still starting.

  The interesting part is what it forgives, and on whose authority. A vendored dependency
  is a package by every test this tool can run - manifest, source, an import graph - and
  it is nonetheless not yours: its architecture is decided in the repository it came from,
  which is where its contract lives. The obvious fix is an allowlist, and an allowlist is
  a hardcoded list of exceptions that drifts the moment somebody vendors a second thing.
  So the dialect reads the manifest instead. `build.zig.zon` spells a vendored dependency
  `.brigade = .{ .path = "brigade" }`, and a build that had not said so would not link -
  the fact is already written down, load-bearing, and maintained by the compiler. Coverage
  just reads it.

  The same reading fixed a subtler bug. A package whose module root *is* its package root
  had its own `build.zig` judged as module code, and a build script legitimately imports
  things no module file may - out of the package, and the vendored dependency by name.
  Judged, declaring a package was a violation of the package's own contract. The file that
  declares a module does not belong to it, so manifests are out of the judged set, in every
  dialect, by the same declaration that made them findable.

### Fixed

- The cycle law was blind to most real cycles, and a property test found it in about a
  second. `forbid cycles across directories` withheld same-directory imports from the graph
  *before* searching it, which severs any tangle whose trip home goes through a neighbour:
  `a/one.zig -> a/two.zig -> b/three.zig -> a/one.zig` binds `a` and `b` into one
  indivisible unit exactly as tightly as a two-file cycle does, and zoning reported nothing.
  Crossing a boundary is a property of the cycle, not of the individual imports in it, so
  the filter moved off the edges and onto the component: search the whole graph, then keep
  the components that bind more than one module. A cycle wholly inside a directory - or
  inside a directory and the door file named for it - is still that module's own business.

  `draft` had been contradicting itself in one breath because of this. It would merge two
  directories into a single zone with the note "these 2 directories import each other, so no
  order separates them", then close with "Nothing else to declare: this graph is already a
  stack". The zone stack reads the directory graph and saw the tangle; the tangle detector
  dropped the edge that proved it. Now a draft over a graph with a real cycle emits the
  variance stanza with an empty reason, which does not parse, which is the whole point.

  Adopting this is not free, and it should not be: it turns previously-silent tangles into
  findings. In our own trees it surfaced five in a 310-file package - two nobody knew about,
  and three that existing variances had described with too few members - and one in another.
  Every one of them was real before this release; the tool simply could not see it. A
  variance whose member list is now short fails as stale rather than passing quietly, so the
  contract gets corrected rather than left subtly wrong.
