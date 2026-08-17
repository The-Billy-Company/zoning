# Changelog

All notable changes to `zoning` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions track the
workspace's `Cargo.toml`.

<!-- towncrier release notes start -->

## [1.4.0] - 2026-08-17

### Added

- A `note` fragment type, for the paragraph that frames a release rather than an
  entry in it. Towncrier renders types in declaration order and `note` is
  declared first, so it lands above `### Added` with no template fork and
  retires itself on fold like any other fragment.
- `bench/` races this gate against the tools people already use, and checks that it is
  right. Three rungs: `correctness` asks every tool the same question on committed cases
  whose expected verdict comes from the language reference rather than from our own
  output; `speed` builds one clean layered package, writes the same rule as a `.zone`
  contract, an `.importlinter` layers contract, and `tach.toml` modules, and times all
  three; `compiler` asks whether `zig` itself says anything about a real cross-directory
  cycle, which is the only incumbent Zig has.

  Only a case where *we* miss something the language says we should catch can fail the
  run. A rival disagreeing is reported and never fatal - a benchmark that breaks when
  somebody else ships a release is a benchmark that gets deleted. The oracle rung runs in
  CI with `--no-uv`, so it is hermetic and downloads nothing.

  Two rules keep the numbers honest. A tool that failed to run is never scored as a tool
  that found a violation, even though both exit non-zero: a moved flag or a config the
  current release parses differently is reported as `error`, quoting what the tool said,
  instead of becoming somebody's win. Reading the output for that is the last net rather
  than the first, because the likeliest way to race a tool that is not installed is a
  launcher standing where it should be, whose complaint looks nothing like a crash - so
  every tool says its own version first, in the tree it is about to judge, since a shim
  resolves per directory. A version has a digit in it and a complaint does not, which is
  the whole test; no launcher is named anywhere, because a list of launchers is wrong the
  day somebody uses one nobody here has heard of. And where a rival cannot express a rule at all it
  is reported `n/a` with the reason in its own terms - `import-linter` and `tach` judge
  modules, so a file-granular guest list has no spelling in either config language, and
  calling that a failure to answer would be a lie in our favour.

  The corpus is deliberately clean, because a dirty one measures how fast a tool can give
  up. A clean graph has no early exit in it: every tool has to read every file to say
  yes, which is the honest worst case and also what CI pays on every green run.
- `zone map --json` emits the whole import graph, and `zone explain --json` answers both
  of its questions in the machine form: one file's standing, or whether one import is
  allowed. Previously only `verify` and `status` had a machine form, and `explain
  --json` accepted the flag and printed prose anyway - which is worse than refusing it,
  since a script asking for JSON and getting a report has no way to tell.

  Every edge and every departure in the graph carries the zone each end sits in,
  resolved during the pass that already did the glob matching, so a consumer never
  re-derives where a file lives. A verb with no machine form now refuses `--json` and
  says which verbs have one.

### Changed

- `zone explain FILE` used to print three importers and `+6`. Three of nine is the shape
  of an answer, not an answer, and the missing six were the ones you were about to go
  looking for. It prints all of them now, wrapped under the field so a long list stays
  readable instead of running off the edge.

  Two other lines were the same defect from the other direction. The zone line dumped
  every glob the zone claims - hundreds, on a drafted contract - where the fact you
  asked for is which one reached *this* file; that is now its own `claimed` line naming
  the single clause. And `may use` listed the language's entire ambient set, which is a
  property of Python rather than a decision anybody made about the package, so it now
  reports the count and lists only the grants the contract actually wrote.

### Fixed

- A Python import broken across lines with a backslash was read as two things, neither
  of them right: the names after the break went missing from the graph, and the
  backslash itself was reported as a module named `\`. So a file could import whatever
  it liked as long as it wrapped the line, and the one law that catches an undeclared
  dependency never fired.

  Continuations are now joined where every other lexical question is answered - the
  blanked copy of the source that already knows a comment from a string erases the
  backslash and its newline to spaces, so a logical line reaches the dialect as one
  line without the dialect knowing the rule exists. Line and column numbers still come
  from the original text, so nothing a diagnostic points at moves.
- A version bump moved `Cargo.toml` and left the lockfile behind, and `--locked`
  is the flag whose whole job is to refuse to fix that. A lockfile records the
  version of every package it locks, including the one it sits next to, so the
  release bumping the manifest through its `x-release-please-version` annotation
  put the two a version apart. `cargo publish --locked` then stopped with "cannot
  update the lock file because --locked was passed", which is correct behaviour and
  a wedge: nothing about it improves on a retry, so the crate never reaches the
  registry no matter how many times the release runs.

  `gist` hit it on v1.2.0 with the wheel and the Go module already published, so
  the tag existed and the crate did not. The committed lock was stale in the tree
  too, which means `cargo build --locked` in `bindings/rust` was already failing
  for anyone who tried it.

  The publish now re-pins the lock's own version from the manifest beside it
  first, hermetically - a `version = "..."` rewrite and nothing else, so no
  third-party pin can move and the graph being published is still the one that was
  tested, which is the reason `--locked` is there at all. `cargo update
  --workspace` was the first attempt and the wrong one: it resolves the whole
  graph, so it wants a sibling `irregex` checkout for the `irgx` path dependency
  that this job has no reason to make, and relate's v1.1.0 failed exactly there
  while `cargo publish --locked` had never needed it.
- CI cancelled its own evidence on `main`. The concurrency group keyed on the ref
  and cancelled unconditionally, which is right on a branch whose runs are drafts -
  a force-push should kill the run it obsoleted rather than race it - and wrong on
  `main`, where every commit is a candidate to be released and the run is the only
  record of whether it may be.

  `release.yml` will not publish a tag unless `release-ready` concluded success on
  that exact commit, which is the check that makes a green release meaningful. But
  `release-ready` gathers its dependencies under `if: always()`, so it reports on
  jobs that never finished as readily as on jobs that failed. So the next push to
  main revoked the previous commit's verdict: a still-running job ended
  `cancelled`, `release-ready` read that as a failure, and preflight declined a
  release with nothing wrong with it. On a tree several people push to, that is
  not a rare race; it is most releases, and it looks exactly like a real test
  failure until you notice the conclusion is `cancelled` rather than `failure`.

  Caught it on `gist`, whose v1.2.0 tag was green on the pull request and then lost
  the release commit's `python (3.14)` job to three docs commits landing behind the
  merge. Every repository in the family had the same line, so every one has the
  same fix: pushes to main no longer cancel each other and each commit keeps its
  own answer, while pull request branches still supersede as before.
- Every release here so far was tagged, released, and relabelled by a person,
  because `release-please-config.json` named the package and that one line kept the
  bot from ever cutting one.

  With `include-component-in-tag` off, release-please writes a standalone release
  PR's body with no component in it, and names the branch
  `release-please--branches--main` with no component either. Then, on merge, before
  it will tag anything, it compares that empty component against
  `component || package-name` - so a `package-name` here makes the two halves of
  its own bookkeeping disagree permanently. Every merge logged `PR component:
  undefined does not match configured component: zoning` and returned without
  creating the tag or the release. That is worse than a missed release, because it
  wedges: an untagged merged release PR makes the *next* run abort before it opens
  anything, so the queue stops until someone relabels the old PR by hand - which is
  where 1.3.1 has been sitting since August, tagged but never released.

  The three maintenance steps on the release PR - the changelog fold, the VS Code
  payload refresh, and the `Cargo.lock` re-pin - only ran on the push where
  release-please rewrote the PR. The action sets its `pr` output only when it wrote
  something, so a `ci`/`docs` commit carrying a new fragment, which changes no
  version and therefore no note, left that output empty and skipped all three with
  nothing saying so. A PR could sit open with fragments unfolded, a stale VSIX, and
  a lock a version behind, and the first complaint would come from preflight
  refusing to publish it. The branch is now resolved from the
  `autorelease: pending` label instead, which is release-please's own marker for
  the PR it is holding open rather than a name guessed from a convention.

  With `always-update` on, the branch is rebuilt on every push while the PR is
  open, so the fold recomputes from main rather than appending to whatever the
  branch already carries - towncrier treats a second write of the same version as a
  hard error, not a no-op.
- The GitHub Release page now carries the changelog section it names. Two
  changelogs were produced per release and only one of them was towncrier's:
  `skip-changelog` hands `CHANGELOG.md` to the fragments, but that key governs
  the *file*, and composing the release **body** is a separate path inside
  release-please that kept running off conventional-commit subjects. So the page
  people land on was assembled from commit subjects while the notes someone
  wrote sat in the changelog - irregex v2.1.1 published two lines against a
  folded section of a hundred and ten, because eleven of its thirteen commits
  were `ci:` or `docs:` and both are hidden. A `notes` job now posts the folded
  `## [X.Y.Z]` section over that body on tag, waiting for the release to exist
  rather than assuming it already does, and truncating at a whole bullet under
  GitHub's 125,000-character body ceiling rather than failing on a tag that is
  already immutable.
- The `discipline` job pins ruff and then runs it with no config, so the repo's Python was
  being judged by ruff's defaults - and it had never actually passed under them. Twelve
  findings, none of them in the Rust the tool is made of, so scheduled CI on `main` was red
  for a reason nobody was going to go looking for.

  Two things were wrong. Seven `# noqa` directives named rules the default set doesn't
  enable, so they suppressed nothing and ruff flagged each one as dead; the `E402` four were
  dead twice over, since ruff already exempts imports that follow a `sys.path` preamble. And
  all five `bench/*.py` carried a shebang while none of them was executable. Only `bench.py`
  has an entry block, so it is executable now and the other four - which are imported, never
  run - lost a shebang that was decorative.

  `ruff format` had never run at all: the check step aborts on the line above it. Two files
  are reformatted to the 88 columns the rest of the tree already used.
- The census asked each zone how many files it held, and answering that question is
  itself a walk over every zone - so the tally cost zones squared times files in glob
  matches. On a drafted contract, where a zone per directory is the entire point, a
  1400-zone package spent 22 seconds counting before it judged anything.

  It now tallies in one pass over the files: 22s to under a second on the same package,
  with the same numbers. A file two zones claim still counts for neither, because that
  is a violation rather than a tenancy.
- The published crate declared Apache-2.0 and carried none of it. `readme` reaches up
  to the repository's own `README.md` and Cargo relocates that one file into the
  tarball, which made the omission easy to miss: nothing else above `crates/zoning/`
  travels, so the license text and the NOTICE did not. Section 4 of that license asks
  a redistributor for exactly those two files.

  Both are now committed in `crates/zoning/`, byte-identical to the root pair, and the
  tarball carries them.
- `dice::scratch` named a directory after what was being tested, not after who asked, so
  every test in a binary that asked for the same fixture got the same tree - and started by
  emptying it. Cargo runs a binary's tests on parallel threads, so the four tests in
  `report.rs` each deleted a tree another was walking. The survey then came back a file
  short and the report dropped one of forty callers, which is the assertion that failed.

  It read like a wrapping bug in the report, which is the wrong place to look: the emitter
  is deterministic and the missing name was a different one each time - `_00` on Linux,
  `_39` on Windows - and never missing at all on macOS, which is what losing a race looks
  like from the outside. Scheduled CI had been red on this since the tests landed.

  A scratch directory is now private per call rather than per name, so no two tests can
  ever name the same tree. Nothing about the fixtures or the assertions moved.
- `import a.mid.deep.leaf` binds one name and initializes three packages: Python runs
  `a/mid/__init__.py`, then `a/mid/deep/__init__.py`, then the leaf. The Python dialect
  read only the leaf, so every one of those ancestors was a dependency the importer
  genuinely had and the contract could not see - and because zones are globs over paths
  rather than module names, a contract could put `mid/__init__.py` in a zone above the
  importer and pass clean. A stack no interpreter could honour, judged green.

  An absolute import now also names each ancestor package it wakes. Only the ones the
  importer does not already live under, which is the same reason a relative import
  contributes none: nothing inside `mid/` runs until `mid/__init__.py` already has, so
  reaching `mid.deep.leaf` from within `mid/` cannot be a new dependency on the root.
  Counting those would have made the ordinary absolute self-import - `from mypkg.sub
  import x`, written inside `mypkg`, with any non-empty `__init__.py` - a cycle through
  the package root in every Python package alive.
- `list` prints the exact `draft` invocation for each ungoverned package so the middle
  line of adoption is a paste rather than a guess. For a Python package it printed a
  command that then failed: `list` knew the dialect from the manifest it had just read,
  `draft` did not read one at all, and under the default dialect it found no `build.zig`,
  concluded the directory was not a package, and refused - naming the package it was
  refusing as an empty path, so the message read with a word missing.

  `draft` now takes the language from the manifest sitting in the directory it was
  pointed at, which is what `--language` always meant: the language for packages that do
  not name one. Nothing needs the flag to draft a package that declares itself. The
  drafted header also no longer tells a Python package it was read from an `@import`
  graph.
- `zone draft` gave every zone a recursive `dir/**`, so a directory that both holds
  files and has children handed the same file to two zones. On a flat tree nobody
  noticed; on a real one the first `zone verify` after a draft opened with thousands of
  `claimed by 2 zones` findings, which is the worst possible first impression for a
  contract whose whole promise is that it is true of the tree it came from.

  A drafted zone now claims the files in its own directory and nothing underneath
  (`dir/*.py`), so the zones partition the tree the way the draft says they do. The
  generator behind the property tests grows nested directories now too - the shape this
  bug needed to appear in was the one shape it could not produce.

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
- Resolving `--under` as a place rather than a spelling left one spelling still
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
