# Security Policy

`zoning` reads a tree it did not write — someone else's source, someone else's
`.zone` contract — and answers a question people wire straight into CI and,
now that `zoning lsp --stdio` exists, straight into their editor: *does this
import graph obey the declared shape?* The threat model here is not "someone
attacks the binary" so much as **the tree is the attacker** — a crafted
contract, a hostile import graph, a symlink, a path that wasn't meant to be
followed. Every one of those is input, and a gate that can be talked into the
wrong verdict is worse than no gate at all, because the wrong verdict looks
like an all-clear.

## Reporting a vulnerability

**Do not open a public issue, pull request, or discussion.**

Use GitHub's private reporting — the **Security** tab on this repository,
"Report a vulnerability" — which opens a thread only the maintainers can read.
If that is unavailable to you, email **<griffin@billylives.com>**.

Please include:

- what you found and what it lets an attacker do;
- the smallest reproduction you can manage: the `.zone` contract, the source
  tree it judges (a script that builds the fixture beats a tarball), and the
  exact command line;
- `zoning --version`, and whether you ran the crates.io binary, the PyPI
  wheel, or a source build;
- your OS and architecture.

We will acknowledge within **72 hours** and give you a triage verdict with a
severity within **7 days**. If it is real we will agree a disclosure date with
you, credit you in the changelog fragment and the release notes unless you
would rather we did not, and ship the fix before the details go public. There
is no paid bounty.

We will not pursue anyone who reports in good faith, works against their own
machines and their own data, and gives us a reasonable window to fix the thing
before publishing.

## Supported versions

zoning is past 1.0 and follows semver. Fixes land on `main` and ship in the
next release; there are no maintained release branches and no backports to
earlier majors. Watch releases on this repository if you pin, and pin the
version in CI either way — [`README.md`](README.md#wiring-it-into-ci) says as
much: a gate whose verdict can change without a commit is not a gate.

## What we consider a vulnerability here

- **A real violation that verifies clean.** The entire value of a gate is that
  a violation cannot slip through. A crafted `.zone` file, a specific import
  shape, or an edge case in the glob matcher that makes `verify` exit `0` over
  a graph that actually breaks a `zone`, `seal`, `keep`, `cycle`, `reach`,
  `escape`, or `use` law is the highest-severity class of bug this project
  has, full stop — worse than a crash.
- **Glob semantics that silently diverge from the reference.** The matcher's
  entire contract is that a `.zone` glob means exactly what the same glob
  means to CPython's `glob.translate(..., recursive=True,
  include_hidden=True)` — that's the promise a contract author is relying on
  when they write one. A divergence that reclassifies a file into the wrong
  zone, or in or out of a seal's guest list, without erroring is in scope even
  if it never causes a crash.
- **Escaping the tree you were given.** A symlink, a `..` in an import
  spelling, or a contract's `root`/`exclude` glob that walks judgment outside
  the repository the tool was pointed at — whether that widens what gets
  judged or narrows it below what the contract claims to cover.
- **A variance that outlives what it names.** A `variance` stanza is
  supposed to go stale the moment the exact edge it names stops existing —
  that's the mechanism that keeps an exception list from becoming permanent
  folklore. Anything that lets a stale variance keep matching, or lets a
  variance's glob reach edges its author never saw, defeats that.
- **The editor payload writing somewhere it shouldn't.** The first interactive
  run (and `zoning setup run`) detects an installed editor and writes its
  language-server/extension payload into that editor's own config directory.
  Writing outside the paths `zoning setup status` reports, or being trickable
  into writing there, is in scope — as is `zoning setup uninstall` leaving
  behind, or removing, anything it did not itself record.
- **The language server trusting the wrong boundary.** `zoning lsp --stdio`
  is meant for exactly one editor process talking to exactly one instance over
  its own stdio pipe. Anything that lets a second process attach to that pipe,
  or lets a crafted `.zone` file or source buffer make the server do something
  other than answer a diagnostics/completion/hover/rename request about the
  buffer it was asked about, is in scope.
- **A contract or a source file that panics the judge instead of erroring.**
  Every fallible path here is supposed to return a diagnostic a human reads
  (`unwrap_used`/`expect_used` are `warn`-level clippy lints for exactly this
  reason); a malformed `.zone` file, a pathological import graph, or a
  filesystem edge case that panics instead is a bug, and one worth reporting
  if you can make it happen from untrusted input rather than from your own
  contract typo.

## What is not a vulnerability

- **Memory safety.** `unsafe_code = "forbid"` is a workspace-level lint in
  [`Cargo.toml`](Cargo.toml), enforced by the compiler on every build profile,
  not a convention someone could forget. There is no `unsafe` block anywhere
  in this crate to audit.
- **`--suggest` drafting a variance you disagree with.** It drafts the
  stanza for today's violation and writes nothing; the reason is yours to
  write, and a machine cannot supply one. A bad draft is a quality report,
  not a vulnerability.
- **Cost proportional to the tree.** A monorepo with `--complete` costs more
  than a single package. That is arithmetic.
- **A dialect that doesn't exist yet.** zoning judges the dialects it ships;
  a request for a language it does not yet read is a feature request.

## What already tries to catch this

None of it is a guarantee, and finding something these missed is exactly the
kind of report we want:

- `tools/differential.py` mutates every real fixture contract the way a
  person breaks one — drops every seal, drops every guest list, squeezes the
  reach ceiling, revokes every variance, inverts the whole stack — and
  requires this implementation to agree, law by law and file by file, with
  the tool it was rewritten from. Agreeing on a clean tree proves nothing,
  because every gate agrees that nothing is wrong; the mutation loop is the
  part that actually exercises the failure modes above.
- `cargo test` carries dedicated fixtures under `tests/fixtures/{pass,fail}/`
  for each law, plus property and fuzz suites the nightly `soak` job in
  [`ci.yml`](.github/workflows/ci.yml) runs at fifty times the per-PR case
  count from a fresh seed every night — a failure there prints
  `ZONING_SEED=…`, and pasting that in front of `cargo test` replays it
  exactly.
- `clippy::pedantic`, `unwrap_used`, and `expect_used` all run as warnings in
  CI, which is where "should have returned a diagnostic, panicked instead"
  gets caught before it ships.
- the `wheel` job in CI *runs* the installed PyPI console script against a
  fixture on every commit, on Linux, macOS, and Windows, so a wheel whose
  entry point resolves to a binary that behaves differently from the
  crates.io one would fail there.
- `cargo deny check` runs in the `check` job against [`deny.toml`](deny.toml)
  on every commit: a RustSec advisory anywhere in the four-dependency graph,
  a license outside the allowlist, or a dependency this crate has no reason
  to carry (`openssl-sys`, `native-tls`, `tokio` — the LSP transport is a
  synchronous stdio loop, not a reason to link TLS or an async runtime).

## Provenance

[`NOTICE`](NOTICE) credits the two behaviors implemented from published
specification rather than from borrowed source: cycle detection (Tarjan's
strongly-connected-components algorithm) and glob semantics (CPython's
`glob.translate`). The crate's own dependency graph is small and named in
[`crates/zoning/Cargo.toml`](crates/zoning/Cargo.toml) — `lsp-server`,
`lsp-types`, `serde`, and `serde_json`, scoped entirely to the language-server
protocol boundary; the judge itself (`survey`, `ordinance`, `judge`, `report`)
stays std-only.
