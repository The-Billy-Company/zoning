# Contributing

Thanks for looking. This page is the practical half — what to install, what to
run, and what a reviewable change looks like here. The design half is
[`README.md`](README.md): the language, the seven laws, and why they're closed.

Two other files bound this one. Report a vulnerability privately, never in an
issue: [`SECURITY.md`](SECURITY.md). How we treat each other:
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## What this repository is, and what it is not

zoning is a single, self-contained Rust workspace — one crate
([`crates/zoning`](crates/zoning)), no sibling checkouts to clone, no path
dependency on anything else in this GitHub organization. The binary ships from
crates.io and PyPI (the wheel wraps the same binary via
[`maturin`](https://www.maturin.rs/), `bindings = "bin"` — the PyPI artifact
and the crates.io artifact are the same program built from the same source,
never a reimplementation). `editors/` carries four small, independent
sub-projects (Vim/Neovim runtime, a VS Code extension, a Zed extension and
Tree-sitter grammar) that package the same binary for an editor rather than
reimplement it.

That shape decides where an issue goes. "A real violation verified clean",
"the glob matcher disagrees with CPython on this pattern", "the editor
extension didn't detect Zed" — all here. "Python doesn't have this problem" is
[import-linter](https://github.com/seddonym/import-linter)'s; this project
doesn't compete with the tools languages already give you, per the README's
[Should You Be Using This?](README.md#should-you-be-using-this) section.

## Setup

| For | Install | Pinned by |
| --- | --- | --- |
| the binary | Rust stable, MSRV **1.88** | `rust-version` in [`Cargo.toml`](Cargo.toml), the `msrv` job in CI |
| the wheel | [uv](https://docs.astral.sh/uv/) | `requires-python` floor in [`pyproject.toml`](pyproject.toml) |
| the VS Code extension | Node/npm | `editors/vscode/package.json` |
| the Zed grammar/extension | Node/npm, `wasm32-wasip1` target | `editors/zed/grammar/package.json`, `editors/zed/Cargo.toml` |
| the discipline gate | markdownlint-cli2, typos, yamllint, Taplo, editorconfig-checker | the `discipline` job in [`ci.yml`](.github/workflows/ci.yml), mirrored into `.mise.toml` |
| supply-chain audit | cargo-deny | [`deny.toml`](deny.toml), the `check` job in CI |

On bare rustup with no mise, [`rust-toolchain.toml`](rust-toolchain.toml) resolves
`rustfmt`, `clippy`, and the `wasm32-wasip1` target on first use — it names the
`stable` channel rather than a fixed release, so it can never disagree with
what `ci.yml`'s own `dtolnay/rust-toolchain@stable` steps install.

If you run [mise](https://mise.jdx.dev), that table is one command:

```bash
mise install
```

`.mise.toml` pins every row at the version CI uses and `mise.lock` carries the
checksums. The pins are mirrors of the files in the third column and never the
authority — bumping one means bumping the other in the same commit.

```bash
cargo build --release       # target/release/zoning
cargo test                  # unit + fixture + property + fuzz suites
cargo clippy --all-targets  # workspace lints: pedantic, unsafe_code = "forbid"
cargo fmt --check
```

## The test loop

```bash
cargo build --release
for box in tests/fixtures/pass/*/; do target/release/zone verify --untracked --root "$box"; done
for box in tests/fixtures/fail/*/; do target/release/zone verify --untracked --root "$box"; done  # each must fail
cargo test --test properties --test fuzz            # the generators, per-PR case count
ZONING_CASES=5000 ZONING_SEED=1 cargo test --release --test properties --test fuzz  # the nightly soak, replayed
python3 tools/differential.py                       # parity against the tool this was rewritten from
```

`tools/differential.py` is the oracle: it mutates every real fixture contract
the way a person breaks one — drops every seal, drops every guest list,
squeezes the reach ceiling, revokes every variance, inverts the whole stack —
and requires this implementation to agree, law by law and file by file, with
the Python tool (`ward`) this was rewritten from. Agreeing on a clean tree
proves nothing, because every gate agrees that nothing is wrong; the mutation
loop is what actually exercises the seven laws' failure modes. It covers the
six laws both implementations share; `use` postdates the rewrite and is pinned
by `tests/fixtures/` instead.

A property or fuzz failure prints `ZONING_SEED=…`; pasting that in front of
`cargo test` replays it exactly, which is the only part of shrinking that
matters at this size.

### The editor loop

Each of the four editors reads the same language through a different runtime, so
each has its own suite and none of them needs the editor installed to run:

```bash
cargo test --test lsp                    # the protocol: every advertised capability, both transports
cargo test --test editors                # parity: every adapter launches the server and claims .zone
./editors/vim/test/run.sh                # every suite in every Vim on this machine (TAP)
(cd editors/zed/grammar && npm ci) && ./editors/zed/test/run.sh   # grammar corpus, highlights, queries
(cd editors/vscode && npm ci && npm test)                         # TextMate scopes, indent rules, manifest
```

`tests/lsp.rs` holds a parity gate: an advertised capability with no test
answering it fails the suite, so `capabilities()` cannot grow a promise the
server does not keep. `tests/editors.rs` is the seam between the two halves —
it reads the adapter files themselves and requires each one to spell the same
`zoning lsp --stdio` the protocol suite proves, so an adapter and a passing
server can't drift apart silently.

The Vim runner reports TAP and runs both `vim` and `nvim` when both are
present; a machine with neither is a failure rather than a silent pass. The Zed
runner regenerates the parser, runs the corpus and highlight annotations, and
then runs each `.scm` against a fixture — an unmatched query compiles fine and
paints nothing, which is exactly what a grammar rename leaves behind.

## The constraints a change is held to

- **Every failure names its remedy.** [README.md](README.md#overview) puts it
  plainly: "a gate whose output does not say what to do next is a gate
  somebody eventually silences." A new diagnostic that doesn't end in a
  concrete next step is not done.
- **The seven laws are closed.** `zone` `seal` `keep` `cycle` `reach` `escape`
  `use`, and no eighth. A proposal for a new law has to argue the compiler
  structurally cannot enforce what it wants, the way each of the seven already
  does — not merely that it would be convenient to check.
- **A dialect adds surface, never behavior.** Resolution, the graph, all seven
  laws, and every rendering are shared across dialects; a `Dialect` may only
  answer "which extensions, how an import is spelled, what the standard
  library is called" — never resolve a path its own way. A dialect that could
  disagree with the others about what a cycle is would make the tool's verdict
  depend on which language you happened to be judging.
- **A draft never guesses at a claim.** `zone draft` derives the stack, the
  grants, and the reach ceiling from what the graph already does — all true
  today — but refuses to guess at seals, keeps, or a cycle's reason, because
  those are decisions a person makes, not facts a machine can infer from one
  snapshot of the call sites.
- **`unsafe_code = "forbid"` stays forbidden.** It's a workspace lint in
  [`Cargo.toml`](Cargo.toml), not a convention. A change that needs it needs a
  different design first.

## What CI will check

[`ci.yml`](.github/workflows/ci.yml) splits build/test jobs, which assemble
and run the binary, from discipline jobs, which read this checkout alone and
fail before a build would matter:

| Job | What it holds |
| --- | --- |
| `check` | `cargo fmt --check`, `cargo clippy --all-targets --all-features`, `cargo test --all-features` (the LSP conformance and adapter-parity suites among them), `cargo deny check` (`deny.toml`: advisories, licenses, bans) |
| `dogfood` | the release binary judges every `tests/fixtures/{pass,fail}` box |
| `portability` | `cargo test --release` on Linux, macOS, and Windows |
| `wheel` | the maturin wheel builds and its installed console script judges a fixture, on all three platforms |
| `editors` | VS Code's TextMate scopes and indent rules, the Zed grammar corpus and every `.scm` query, the Vim and Neovim runtime suites, and each extension still packages and builds |
| `soak` | nightly, 50× the per-PR case count of the property and fuzz suites, from a fresh seed. Deliberately outside `release-ready` |
| `actions` | zizmor and actionlint over every workflow |
| `discipline` | markdownlint, typos, yamllint, Taplo, editorconfig-checker, Ruff (`tools/differential.py`, `editors/vscode/scripts/deterministic.py`), ShellCheck (`.githooks/pre-push`) |
| `changelog` | towncrier fragments parse and the draft render is non-vacuous |
| `msrv` | `cargo check --all-features` at the floor Rust version (`1.88`, kept in step with `Cargo.toml`) |

`release-ready` needs every one of those - `soak` deliberately excepted -
green on the exact tagged commit; that's the check `release.yml`'s preflight
polls for (`release.toml`'s `[ci].required_check`).

## Every change carries its own news

Write a towncrier fragment in the **same PR**:

```bash
towncrier create '+<slug>.<type>.md'    # types: added changed deprecated removed fixed security
```

Fragment names read like the sentence they are:
`+the-lsp-server-folds-a-crashed-buffer-into-one-diagnostic.fixed.md`. The
leading `+` tells towncrier there is no issue number attached. The body is
prose for a person reading release notes — what changed and what it means for
them, not a restatement of the diff.

Skip it only for comment-only, format-only, or genuinely invisible internal
work. When unsure, write it.

## Commits and pull requests

Commit subjects here are a conventional prefix plus a lowercase sentence that
says what changed, in the voice of the change rather than the ticket:

```text
fix: the glob matcher stopped at a trailing slash CPython allows
feat: a language server, first-run editor setup, and adapters for .zone
ci: the changelog fold's quiet-check compared the wrong tree
```

Prefixes in use: `feat` `fix` `perf` `refactor` `docs` `test` `build` `ci`
`chore`. Keep the subject under about 72 characters and put the reasoning in
the body, where reviewers and `git log` both find it. The subject line becomes
the squash commit message, which is what release-please reads to decide the
next version — an unconventional title is not a style nit.

For the pull request: one concern per PR, describe what would have caught the
bug if it had existed, and fill in the template. Reviews here ask three
questions more than any others — *what proves this?*, *what does it cost?*,
and *what did it replace?* Answering them in the description saves a round
trip.

If you removed something that a newer path superseded, remove it completely.
Leaving the old implementation beside the new one to be safe is how a codebase
grows two spellings of the same bug.

## Licensing

This project is Apache-2.0. There is no CLA: contributions are accepted under
the same license the project already carries, per the inbound=outbound norm in
section 5 of the license itself.

[`NOTICE`](NOTICE) credits the two behaviors implemented from published
specification rather than from borrowed source — Tarjan's
strongly-connected-components algorithm and CPython's `glob.translate`. If you
bring in code, data, or an idea from another tool, credit it at the call site
and in `NOTICE`.
