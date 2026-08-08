# Changelog

All notable changes to `zoning` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions track the
workspace's `Cargo.toml`.

<!-- towncrier release notes start -->

## [1.3.1] - 2026-08-08

### Added

- A run that has to shell out to an editor's own CLI — `cursor --install-extension`,
  `--list-extensions` — genuinely costs seconds, and the first-use setup did that with
  nothing on screen, so a terminal that had gone quiet for a few seconds looked identical
  to one that had hung. The main verbs had the same gap on a slow disk or a large `--under`
  sweep: silence while `verify`/`status`/`show`/`map` read every contract, then the whole
  report at once.

  Both now start a small braille spinner on standard error the moment there is real work
  to do, and stop it — clearing the line — the instant an answer is ready to print. It
  costs nothing on the fast path: nothing renders until the call has run for 150ms, so a
  `map` over a handful of small packages never draws a frame it would have to immediately
  erase. Off a terminal, under `CI`, or with `ZONING_NO_SETUP` set, it never spawns a
  thread at all — the report and its exit code are unchanged either way, and stdout never
  carries a spinner byte.
- `use` is the one law newer than `tools/differential.py`: it has no Python twin
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
- zoning could only read Zig, and Zig's total absence of a module system was the
  whole reason the tool exists. Python is a different animal: `import a.b.c` and
  `from a.b import c` already resolve dotted names against real files, and
  `sys.path` already draws a boundary. What that graph has never said is which
  of *your* packages may reach which — the same question a zone stack answers,
  at a grain the language's own boundary is too coarse to draw.

  The new `python` dialect reads that graph on its own terms rather than
  borrowing Zig's: dotted names resolve to a leaf module or a package's
  `__init__.py`, a relative import's leading dots climb exactly as many
  directories as they count regardless of how deep the `from` nests, and
  `pyproject.toml` supplies both the package's declared name and its
  `path = "…"` vendored dependencies, the same authority `build.zig.zon` carries
  for a vendored Zig one. The standard-library grant is drawn from
  `sys.stdlib_module_names` unioned across every CPython release still in
  support rather than one interpreter's local answer, so a contract written
  against 3.12 keeps meaning the same thing under 3.13 or 3.14 without an
  edit.

### Changed

- A contract had to live in a `contract/` drawer, so a boundary tool asked every
  package for a directory to hold one page — while the manifest, the formatter
  config, and the CI config all sat at the root. And it had to say everything
  itself: ten Zig kernels in one tree were ten copies of the same `root`,
  `language`, and `facade`, which is not a cosmetic cost. Four lines nobody reads
  are four lines nobody notices are wrong, and a repository-wide fact spelled
  per-package will eventually disagree with itself in exactly one package and
  pass.

  A `.zone` file now governs the directory it sits in, so `acme/acme.zone`
  is the whole layout. The drawer still resolves to the same anchor, because
  contracts are checked in and a tool does not get to invalidate a repository's
  on-disk shape to tidy its own rules; nothing downstream can tell the two apart,
  and only `draft --write` picks a side. Since `.zone` is also BIND's extension
  for DNS data and a contract now sits where a nameserver file might, identity
  moved off the extension and onto the first declaration: a contract opens with
  `package` or `workspace`, and a sweep walks past any `*.zone` that opens with
  something else — a claim of authorship rather than a guess about content, and
  only for sweeping, since a file named on the command line is still parsed and
  still faults.

  The shared part is now sayable once. A `workspace { member … }` block claims
  packages below it and hands down `root`, `language`, `facade`, `use` grants, and
  the reach ceiling; a member's own word always wins, and a member that agrees
  with all of it writes `package hearth` with no block at all. The link points
  down, the way `[workspace] members` and `[tool.uv.workspace]` do — a package is
  a member because something above it said so, never because it declared a parent,
  so nothing can quietly attach itself to a policy nobody granted it. What a
  workspace deliberately cannot share is anything naming a file: zones, seals,
  keeps, and variances are claims about one graph, and a blanket exception written
  once for a whole monorepo is the accretion this language exists to prevent.

  An inherited grant is judged against the whole membership rather than each
  member, or sharing a line would be strictly worse than repeating it — one grant
  of nine members would fail eight times. It is stale only when no member
  exercises it, the report names the workspace that wrote it, and a run that saw
  only part of the membership (`--under`, or a sibling that would not parse) says
  nothing at all: absence of evidence across an unknown remainder is not evidence.
- Release versioning is documented where you pick a commit prefix, and the two
  settings that made it look like something other than semver are gone.

  `bump-minor-pre-major` and `bump-patch-for-minor-pre-major` sat in
  `release-please-config.json` since before 1.0.0. release-please reads them only
  while the version is below 1.0.0, so they have meant nothing since 1.0.0 while
  still reading like a bump policy - and this repo had never cut a patch release,
  which made them look like the reason. They were not: every release window so
  far happened to carry exactly one `feat`.

  What actually decides the number is now written down: `!` or a BREAKING CHANGE
  footer takes the major, `feat` takes the minor, everything else takes the patch,
  said in `CONTRIBUTING.md` where you pick a prefix and in full in the org
  standard, alongside the `Release-As: X.Y.Z` footer that pins an exact version
  when the rules would not pick it and which was previously documented nowhere.
- `tools/differential.py` held a hardcoded table of seven contracts by name and
  repo-relative path — the packages that happened to sit beside this one on the
  machine it was written on. That is two problems in one line each. A public
  repository shipped a list of somebody's private tree, and a contributor cloning
  it got seven `skip` lines and a gate that proved nothing, with no hint that the
  list was the thing to edit.

  It sweeps the surrounding workspace instead, taking any `<pkg>/<pkg>.zone` or
  `<pkg>/contract/<pkg>.zone` it finds and skipping this repository, whose own
  fixtures are contracts written to fail rather than packages to check. Point it
  somewhere specific with `--contract PATH`, repeatable. The list nobody could
  keep in step is gone, and the documentation examples that named real packages
  now name `acme`, like every other example here.
- `zone list` names the package a contract declares rather than the file it was
  written in.

  The stem was a fine stand-in while every contract was named after its package,
  because then the two were the same word. They are not the same word once a tree
  adopts a convention: a fleet that calls every root contract `charter.zone` got a
  column reading `charter  charter.zone`, which is the listing telling you what
  you just read. The `package` block was always the authoritative name - it is
  what a verdict header, a `--package` filter, and a workspace lookup all read -
  so the listing now reads it too, and falls back to the stem only for a contract
  too malformed to parse, where a name is the one thing left to salvage.

  The README also writes the naming convention down, under `What To Call It`: one
  name at a repository root, a role name for a package nested inside a bigger tree
  (`kernel.zone`, `service.zone`), and nothing enforced either way. A tool that
  dictated filenames would be back to demanding a directory.

### Fixed

- Four editors read `.zone` through four different runtimes - a TextMate grammar, a
  Tree-sitter grammar with four queries, a Vim syntax file, and one language server behind
  all of them - and the only thing CI asked of any of them was that it built. So the parts
  that decide what you actually see on screen were the least checked code in the repo, and
  they were wrong in five places nobody would have noticed by reading them.

  `variance seal` painted `seal` as a statement keyword in Vim, in Zed, and in the server's
  semantic tokens, because the word naming a law is spelled the same as the word opening a
  statement. A `//` comment in Vim highlighted every keyword inside it, since the comment
  rule was defined after the words it was supposed to swallow. Zone names like `floor.zig`
  had no scope at all in VS Code, where Zed has painted them since the grammar shipped. The
  server's semantic tokens matched keywords as substrings, so `package` lit up inside
  `packages/**`, and the legend advertised five token types when the server only ever emits
  two. VS Code's `increaseIndentPattern` only recognized one of the block forms the language
  has, and nothing continued the line after `because`.

  All five are fixed, and each of the four editors now has a suite that would have caught
  its own: TextMate scopes tokenized through `vscode-textmate` exactly as the editor does,
  Tree-sitter highlight annotations plus a pass requiring every query to still match what
  Zed reads from it, `syntax.vim` assertions on the group at a line and column and what it
  links to, and a protocol suite that answers every capability the server advertises - with
  a gate that fails if `capabilities()` ever grows a sixth promise nothing keeps. The Vim
  suites run under both `vim` and `nvim`, which disagree about enough to be worth it.
- `--under` compared its argument to the sweep's rows as a string, and the sweep's
  rows are repo-relative posix paths. So `--under libs/kernels` worked and `--under
  ./libs/kernels` matched nothing — as did any absolute path, which is what a
  shell's tab-completion hands you and what a CI script is entitled to write. The
  failure mode is the worst one available to a gate: a narrowing to nothing reads
  exactly like a clean tree, so `zone verify --under ./libs/kernels` judged no
  package at all and exited 0.

  The argument is now resolved as a place rather than compared as a spelling —
  absolute, `./`-prefixed, and `.` itself all name the subtree they obviously mean,
  and a path outside the tree being swept still narrows to nothing, because that is
  what it means.
- `draft` wrote `use module by` — a `by` clause with nothing after it — whenever
  an outside module was imported only from the facade and never from any file a
  zone actually covers. The facade has no zone to scope a grant to, so the
  scope list came out empty, and an empty scope is not a legal grant: `zoning
  verify` on the file `draft --write` had just produced refused to parse it.

  The grant is now unscoped whenever any of its imports come from the facade —
  `use module`, no `by` — the same shape a person would write by hand for a
  dependency that isn't any one zone's business.
- `zone explain FILE` decided which findings belonged to that file by checking
  whether the finding's human-readable subject string started with the file's
  path. That only holds by construction for a law whose subject already is
  `"{file} -> {target}"`; `use`'s subject is `"{zone or facade} -> {module}"`,
  which never starts with a file path, so a file with an ungranted import
  always reported a clean standing while `zone verify` on the same package
  listed it as broken. An unclaimed-zone violation had the identical gap
  (`subject` there is `"unclaimed:{file}"`).

  `explain` now matches a finding to a file by the finding's actual recorded
  path instead of pattern-matching its prose, converted into the same
  repo-relative coordinates `Finding.path` is already reported in. The new
  `use`-law property test caught this by generating packages where it was
  exercised for the first time at scale.


## [1.2.0] - 2026-08-05

### Added

- The crate is `zoning` and always will be — that's the thing you `cargo install`
  or `pip install` — but nobody wants to type six syllables before every `verify`.
  `zone` is now installed alongside it, from the same source, as the second
  `[[bin]]` target of the identical binary: `cargo install zoning`, `pipx install
  zoning`, and `uv tool install zoning` all put both `zone` and `zoning` on
  `PATH`, and either name runs the same executable byte-for-byte apart from its
  own filename.

  `zone` is the one the docs teach now — `--help`, `--version`, every error
  message, and the `zone map`/`zone [package]:` report headers all say `zone`
  regardless of which name launched them, the way `rg --version` says `rg` and
  not `ripgrep`. `zoning` keeps working exactly as before for anyone whose
  fingers, scripts, or CI YAML already know it; nothing that names `zoning`
  today needs to change.
- The first parity pass copied what every sibling repo already had; this one catches what zoning
  needed that they didn't, because zoning is the only one of the five that is 100% Rust rather than
  Rust bindings over a Zig core. `deny.toml` plus a `cargo deny check` step in the `check` job close
  a real gap the siblings did not have either — this crate carries four real dependencies now
  (`lsp-server`, `lsp-types`, `serde`, `serde_json`) for the in-process LSP server, and nothing was
  watching that graph for a RustSec advisory, a license outside policy, or TLS/async-runtime crates
  that transport has no reason to link. `rust-toolchain.toml` resolves `rustfmt`, `clippy`, and the
  `wasm32-wasip1` target `editors/zed` needs for a bare-rustup contributor with no mise — pinned to
  `stable` rather than a fixed release like the siblings' copy of this file, because every job in
  `ci.yml` installs its toolchain with `dtolnay/rust-toolchain@stable`, and a fixed-version pin would
  have silently frozen every one of those rolling jobs to whatever release was current the day the
  file was written. `.vscode/{settings,extensions,tasks}.json` gives a contributor the same
  watcher/search excludes, formatter bindings, and one-click cargo/dogfood/deny/editor tasks the
  siblings ship, adapted off Rust rather than Zig as the primary language.
- zoning shipped without the governance and hygiene layer its sibling repos already carry:
  no `CODE_OF_CONDUCT.md`, `SECURITY.md`, or `CONTRIBUTING.md`, no issue or pull-request
  templates or `CODEOWNERS`, no `labels.json`/`triage.py` triage automation, and no
  `.typos.toml` / `.taplo.toml` / `.yamllint` / `.editorconfig-checker.json` / `.mise.toml`
  to hold the parts of the tree that aren't Rust to the same bar as the parts that are.
  Filing against this repo meant a different experience than filing against a sibling for
  no reason but that nobody had written the second one down yet.

  All of it is written now, specific to what zoning actually is rather than copied verbatim.
  `SECURITY.md`'s threat model covers a false verdict, glob semantics silently diverging from
  the CPython contract they're specified against, and the in-process LSP server this crate now
  ships (`zoning lsp --stdio`) - not a generic supply-chain section a zero-dependency crate has
  no surface to justify. `labels.json` carries zoning's own `area/*` taxonomy (`cli`, `lsp`,
  `contract`, `editors`, `ci`, `docs`, `build`) mapped onto its real module tree, alongside the
  `size/*`/`status/*`/`type/*` rows kept byte-identical with every sibling so `triage.py peers`
  still holds across all of them. A new `discipline` CI job runs markdownlint, typos,
  yamllint, taplo, editorconfig-checker, ruff, and shellcheck over everything `cargo test`
  never touches, and it is a `release-ready` dependency exactly like `check` or `dogfood` - a
  tag can no longer ship with a broken paper trail any more than it can ship with a failing
  test.


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

  So: `use <module> [by <zone>…]`. A named import the build resolves - `hyper`,
  `build_options`, later `requests` - now needs a grant, and the grant carries a scope,
  because "the CLI face may talk to the network" and "any file in this package may talk
  to the network" are different architectures that used to be spelled the same way.
  Grants go stale like everything else here: a `use` nobody exercises is a permission
  somebody forgot to withdraw, and it fails the run rather than sitting there.

  The standard library is exempt by construction, per dialect - `std`, `builtin`, `root`
  in Zig. Every zone has it, no zone chose it, and a contract that spent its lines
  declaring it would bury the handful of grants that are actually decisions.

  One ergonomic thing I got wrong the first time and want written down. The first
  implementation reported the law per import site, so a package importing `hyper` from
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
  `.vendor = .{ .path = "vendor" }`, and a build that had not said so would not link -
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
