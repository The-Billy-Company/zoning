# zoning: An Import-Topology Gate

- [Overview](#overview)
- [Why Not a Code Review?](#why-not-a-code-review)
- [Should You Be Using This?](#should-you-be-using-this)
- [Support](#support)
- [Install](#install)
- [Editor Language](#editor-language)
- [The Language](#the-language)
  - [The Package Block](#the-package-block)
  - [Zones](#zones)
  - [Seals](#seals)
  - [Keeps](#keeps)
  - [Grants](#grants)
  - [Structural Laws](#structural-laws)
  - [Variances](#variances)
- [The Seven Laws](#the-seven-laws)
- [Reading a Map](#reading-a-map)
- [Adopting It](#adopting-it)
- [The Verbs](#the-verbs)
- [Wiring It Into CI](#wiring-it-into-ci)
- [Languages](#languages)
- [Build and Test](#build-and-test)
- [Where This Came From](#where-this-came-from)

## Overview

Every language hands you some boundary. Go has `internal/`, Rust has
`pub(crate)`, Python has a package graph, TypeScript has an exports map.

Inside a single Zig package there is nothing at all. Every import is a
filesystem path, any file may name any other, and because analysis is lazy a
genuine import cycle compiles clean. Architecture there is a convention with
nothing standing behind it.

zoning is what stands behind it: a package declares the shape it means to have
in a `.zone` file, and the tool judges that declaration against the real
`@import` graph.

```
✗ zone [irregex]: 310 files, 1436 imports, 2 violation(s), 3 allowed
src/kernel/regex/glean/differential_test.zig:32:1: [zone] zone `regex` imports up into `query` (`kernel/query/query.zig`) — imports may only point down the stack
src/exec/cold/emit/render.zig:31:1: [seal] reaches past the seal on `kernel/scan/` into `kernel/scan/simd.zig` — enter through `kernel/scan/scan.zig`

zone: Move the dependency down the stack, or ratify the edge with `variance zone … because "…"`.

seal: Re-export what the caller needs from the seal's entry file, or widen that seal's `open to` list.
```

Every failure closes with the remedy for its law. A gate whose output does not
say what to do next is a gate somebody eventually silences.

It reads the tree, not a build system; there is no project model to configure,
no graph to rebuild, nothing to keep in step. The judge stays std-only. The
installed executable also carries a narrow, MSRV-checked protocol stack for its
in-process language server.

## Why Not a Code Review?

A reviewer catches the import that points the wrong way. A reviewer does not
catch the fourth one, in the file nobody opened, eleven weeks later, once the
person who drew the line has moved on.

Architecture decays by increments that each look reasonable in isolation. That
is the whole failure mode, and it is exactly the kind a machine is good at.

The other half is that a declaration is a *document*. `zones { … }` read top to
bottom is the architecture, in the order it actually stacks, on one screen.

A README that says the same thing drifts. The `.zone` file cannot: it fails the
build the day it stops being true.

## Should You Be Using This?

- **Java** – [ArchUnit](https://www.archunit.org/) already does this, over the
  boundary Java gives you.
- **Go, wanting the boundary Go draws** – `go vet` and `internal/`, and nothing
  else to install.
- **A language that draws no boundary, or a boundary coarser than the one you
  want** – here. Zig has no module system inside a package at all; Python's
  package graph exists but says nothing about which of *your* packages may
  import which — [import-linter](https://github.com/seddonym/import-linter)
  covers that ground too, if you would rather stay in one ecosystem's tooling.

The dividing line is whether your language already separates the parts you mean
to keep apart, at the grain you actually want. Zig draws none; Python's is
real but coarser than a zone stack, which is why both dialects exist here.

## Support

- Bugs and feature requests go in the
  [issue tracker](https://github.com/The-Billy-Company/zoning/issues), with the
  contract and the output. A verdict without its `.zone` file is a verdict
  nobody can reproduce.
- Security vulnerabilities never go in a public issue. Mail
  <griffin@billylives.com> instead.
- A governed package usually lives in a repository of its own. An argument
  about where a boundary *should* fall belongs there; a wrong reading of the
  graph belongs here.

## Install

The binary ships from crates.io and PyPI, prebuilt, under two names — `zone`
is the command to type, `zoning` is the same executable installed alongside
it for anyone who typed the package name out of habit:

```bash
cargo install zoning          # the static binary — installs `zone` and `zoning`
uv tool install zoning        # the same thing, through PyPI
pipx install zoning
```

Or build it here:

```bash
cargo build --release         # target/release/{zone,zoning}
```

The first interactive run detects Cursor, VS Code, Zed, Neovim, and Vim and
installs the matching adapter. It never mutates an editor home under `CI`, from
a non-terminal process, when `ZONING_NO_SETUP=1`, or while serving LSP. The
explicit lifecycle is always available:

```bash
zone setup status
zone setup run
zone setup repair
zone setup uninstall
```

## Editor Language

`.zone` is a real editor language, not a filename with borrowed highlighting.
The same executable serves diagnostics and language intelligence:

```bash
zone lsp --stdio
```

Cursor and VS Code receive the exact SVG file identity plus completion, hover,
navigation, symbols, folding, semantic tokens, formatting, zone rename, and
safe code actions. The embedded VSIX launches the separately installed
`zoning` binary; the extension never downloads another executable.

Zed receives a native extension, Tree-sitter grammar, and the same LSP once its
registry submission lands. Zed does not expose a local-extension install CLI,
so setup adds `zoning` to `auto_install_extensions` without reformatting the
rest of the user's JSONC. Its default icon theme submission is separate because
Zed forbids language extensions from carrying icon themes.

Vim and Neovim receive file detection, syntax, indentation, folding, and LSP
client registration. Neovim 0.11 starts the server natively; Vim connects
through an installed supported client. Terminal editors cannot render SVG file
icons, so the runtime uses a Nerd Font glyph with Unicode and ASCII fallbacks.

Setup state is versioned and owned by zoning. Repair is idempotent; uninstall
removes only files recorded by zoning and leaves unrelated editor settings
alone.

## The Language

A package opts in by writing `<name>.zone` at its own root, beside the manifest
and every other file that configures it, and here is a whole one:

```
package acme {
    root     src
    language zig
    facade   root.zig
}

// Low to high. An import may not point up the page.
zones {
    portal   portal.zig
    assay    assay/**
    math     kernel/math/**
    regex    kernel/regex/**
    session  exec/session/**
    ffi      surface/ffi/**
}

seal kernel/regex through regex.zig     // enter a deep module by its door
keep surface/api.zig to root.zig        // and this region has a guest list

use build_options                       // and these imports may leave
use irregex by ffi

limit  reach to 5 hops
forbid cycles across directories

variance zone a.zig -> b.zig
    because "…and here is exactly what would retire this"
```

Comments are `//`. Globs mean what they mean to a Python reader; the matcher is
CPython's `glob.translate(..., recursive=True, include_hidden=True)`,
reimplemented and pinned by tests against that reference.

### Where The File Goes

A contract sits at the root of what it organizes: `acme/acme.zone`
governs `acme/`. That is where the manifest, the formatter config, and the
CI config already live, and a boundary tool that demanded its own directory
would be asking for a folder to hold one page.

A file inside a `contract/` drawer still governs the drawer's parent, because
that was the only layout zoning accepted before and contracts are checked in. A
tool does not get to invalidate a repository's on-disk shape to tidy its own
rules. Both spellings resolve to the same anchor and nothing downstream can tell
them apart; `zone draft --write` only mints the first.

`.zone` is also BIND's extension for DNS data, and now that a contract sits
where a nameserver file might, the extension cannot be what identifies one. So a
contract leads with `package` or `workspace` — what it governs, before anything
it says about it — and a sweep skips any `*.zone` that opens with something
else. That is a claim of authorship rather than a guess about content, and it
only governs sweeping: a file named on the command line is parsed, and faults,
like anything else.

### The Package Block

`root` names the source directory, relative to the directory the contract
governs. `language` names the dialect this package is read in, so a monorepo
holding several is still one run. `facade` names the files that may reach
anywhere: the module's public face, which by construction re-exports everything
and therefore imports everything. `exclude` drops paths from judgment entirely.

The whole block is optional. A member of a workspace can inherit every setting
in it, and then `package hearth` on one line is the entire header — a mandatory
empty `{ }` is exactly the boilerplate the workspace was written to delete.

The file that *declares* the package — `build.zig`, `pyproject.toml` — is never
judged as part of it, in any dialect. A build script legitimately imports things
no module file may, and a package whose contract made declaring it illegal would
be a joke.

### Workspaces

Ten packages in one tree are ten copies of the same four lines, and boilerplate
is not a cosmetic problem: four lines nobody reads are four lines nobody
notices are wrong, and a per-package fact that is really a whole-repository fact
will eventually disagree with itself in one package and pass. So a file above
them may claim them and say the shared part once:

```
workspace {
    member   libs/kernels/*
    root     src
    language zig
    facade   root.zig
    use      hyper by core/**
    limit    reach to 1 hop
}
```

The link points down, the way `[workspace] members` and `[tool.uv.workspace]`
do: the greater document claims its members, and a package is a member because
something above it said so, never because it declared a parent. One less thing
to keep in sync, and a package cannot quietly attach itself to a policy nobody
granted it. Inheritance is one hop — a member finds the nearest workspace above
it that claims it, and that is the whole search, because a chain of overriding
defaults is a thing you debug rather than a thing you read.

A member's own word always wins, so inheritance can delete a line somebody had
to repeat but can never change the meaning of one they wrote. What a workspace
*cannot* share is anything naming a file: zones, seals, keeps, and variances are
claims about one graph, and a blanket exception written once for a whole monorepo
is the accretion this language exists to prevent. Nor can a shared grant be
scoped by zone name — zones belong to a package and each member's are its own —
so `use hyper by core/**` is a path glob or nothing.

A file may hold both blocks: a root package with members below it. A file that
holds only a `workspace` governs no graph of its own, so a sweep finds it and
never offers it up to be judged.

### Zones

Declared bottom-up. Each zone is a name and the globs that belong to it, and
their order on the page *is* the stack: a file may import anything at or below
its own height, and nothing above.

Declare zones at the granularity the architecture actually has, not the
granularity of your top-level folders. Collapsing six kernel tiers into one
`kernel` zone is how `math` grows a dependency on `slate` and calls it legal.

### Seals

`seal <dir> through <entry>` says that directory is a deep module: outsiders
enter through the entry file and may not reach past it. The entry may live
inside the directory (`regex/regex.zig`) or beside it (`sqrt.zig` next to
`sqrt/`), because both spellings of "front door" are in use and a contract
should not have to care. `open to <globs>` widens the guest list for one seal.

A seal is the strongest statement in the language, and the one that pays most.
It is how "there is exactly one parser for this grammar" stops being a claim
and becomes a fact a fork cannot route around.

### Keeps

Zones order the stack and say nothing about peers, because peers at one height
are unordered by construction. `keep <subject> to <importers>` is where that
silence ends: only the named importers may reach the subject at all.

Two importers are always implicit and need no naming, the region's own insiders
and a file's directory sibling, because `render.zig` importing `render_test.zig`
is an aggregation idiom rather than an architectural crossing.

`keep <subject> to nobody` is the limit case, for the region whose whole point is
that nothing reaches it.

### Grants

Everything above governs where an import may point *inside* the package. `use`
governs the ones that leave:

```
use build_options            // anywhere in the package
use tokio by session cold    // only these zones
```

That scope is the reason the law exists. "The CLI face may talk to the network"
and "any file in this package may talk to the network" are different
architectures, and without a scope they are spelled the same way. A zone stack
tells you `kernel/` sits under `surface/`; it will never tell you that a leaf
three directories down started dialing an HTTP client, and that is the dependency
that ends up hardest to remove.

The standard library is exempt by construction, per dialect — `std`, `builtin`,
`root` in Zig; the whole standard library, version-independent, in Python.
Every zone has it, no zone chose it, and a contract that spent its lines
declaring it would bury the handful of grants that are decisions.

A grant nobody exercises is stale, and stale is a failure. It is a permission
somebody forgot to withdraw.

A grant a member *inherited* is judged once, against the whole workspace. Asking
each member to exercise every shared line would make sharing a line strictly
worse than repeating it, since one grant of nine members would fail eight times.
So it is stale only when no member exercises it, and the report names the
workspace that wrote it rather than a package that merely lives under it — and
a run that judged only part of the membership (`--under`, or a contract that
would not parse) says nothing at all, because absence of evidence across an
unknown remainder is not evidence.

### Structural Laws

`limit reach to N hops` caps how many `../` an import may climb. It is a
ceiling you lower deliberately, never one you raise to go green.

`forbid cycles across directories` bans import cycles that cross a directory
boundary, which is the class of cycle a lazy compiler will happily accept.

### Variances

Every exception is a `variance`, and every variance must carry a reason:

```
variance cycle {
    exec/cold/engine/serial.zig
    exec/cold/engine/swarm/swarm.zig
} because
        \\Work distribution recursion: `serial` hands a multi-file query to
        \\`swarm`, and each swarm worker runs the identical per-file path back
        \\in `serial`, which is what keeps the parallel and sequential answers
        \\byte-identical. Retire by extracting that per-file path into a leaf
        \\both import.
```

The `\\` block is a folded paragraph, delimited per line so a missing
terminator cannot swallow the rest of the file.

A variance that stops matching is a hard failure. Pay the debt and the build
tells you to delete the entry; exception lists in most tools accrete into
folklore nobody dares touch, and this one can only shrink.

`zone verify --suggest` drafts the stanzas for today's violations and writes
nothing. A machine can find the edge; it cannot supply the reason, and the
reason is the entire value.

## The Seven Laws

Seven, and closed on purpose. A boundary language whose vocabulary grows per
project stops being a language and becomes a config file.

- **`zone`** – an import points up the stack.
- **`seal`** – an import reaches past a sealed directory's entry file.
- **`keep`** – an importer not on the guest list reaches into a kept region.
- **`cycle`** – an import cycle crosses a directory boundary.
- **`reach`** – an import climbs more `../` than the ceiling allows.
- **`escape`** – an import climbs out of the module root entirely.
- **`use`** – a zone imports an outside module no grant covers.

Each exists because a compiler structurally cannot enforce it.

Six of them are claims about how a package's files sit relative to each other, so
a single-file module has almost nothing for a contract to say; `use` and `escape`
are the two that still bind. `zone list` says so rather than letting you write
the file and wonder.

## Reading a Map

`zone map` draws the contract the way gravity works, so "imports point down
the page" stops being a rule you memorise and becomes a thing you can see:

```
zone map · irregex · 26 zones, high to low
──────────────────────────────────────────────────────────────────────────
 25 │ ffi      ██············  7 ↓5      surface/ffi/**
 24 │ api      █·············  2 ↓3    ⊘ surface/api.zig surface/api_test.…
 23 │ session  ██████········ 35 ↓13  ⊙  exec/session/**
 22 │ cold     ███████······· 44 ↓16  ⊙  exec/cold/**
 21 │ cli      █·············  6 ↓4      surface/cli/**
  …
  8 │ regex    ██████████████ 92 ↓4   ⊙  kernel/regex/**
  7 │ scan     ███··········· 16 ↓4      kernel/scan/**
  5 │ math     ███··········· 18 ↓3      kernel/math/**
  3 │ fault    █·············  1 ↓1      fault.zig
  2 │ assay    █·············  5      ⊙  assay/**
  1 │ portal   █·············  1         portal.zig
──────────────────────────────────────────────────────────────────────────
 310 files · 1436 imports · 5 seals · 3 keeps · 2 grants · reach ≤ 5 hops
 3 ratified variance(s) — each one names what would retire it
```

The bar is how many files the zone holds. `↓N` is how many *distinct* zones
beneath it that zone actually reaches into, which is the number worth staring
at: a zone that reaches into every zone below it is not a layer, it is a pile.
`⊙` marks a sealed directory, `⊘` an anchored guest list.

## Adopting It

Every boundary tool is easy to love on a greenfield package and miserable to
adopt on a real one. A tree with nine hundred files has an architecture already —
it is simply undeclared — and the first contract somebody writes for it arrives
red, which teaches the reader exactly one lesson: the gate is noise.

So don't write the first one. Take it:

```bash
zone list                     # every package here, governed or not
zone draft . --write          # the contract this graph already obeys
zone verify                   # green, on the first run
```

`list` prints the exact `draft` invocation for each ungoverned package, so the
middle line is a paste rather than a guess — and it names the package what the
package's own manifest names it, not what its directory happens to be called.

`draft` derives the stack from a topological sort over directories, the grants
from the modules the code already imports, and the reach ceiling from the reach
the tree actually needs. Everything it emits is *true today*. Then the cleanup
begins, and every step of it — merge two zones, seal a directory, drop a grant,
lower the ceiling — is a decision somebody made on purpose rather than a fight
with a wall.

It refuses to guess at two things. Seals and keeps are claims — "this directory
is a deep module", "these peers are independent" — and a machine inferring them
from today's call sites would guess wrong the first time somebody adds a second
legitimate caller. And a real import cycle comes out as a `variance` stanza with
an empty reason, which does not parse: a draft over a genuinely tangled package
cannot be adopted until a person has written why each tangle stays.

Directories that import each other cannot be ordered, so they land in one zone,
and that zone is called `tangle`. A zone nobody enjoys reading is a zone somebody
eventually splits.

The question you actually have day to day is narrower, and `explain` answers it
without a contract edit or a full run:

```bash
zone explain src/exec/cold/emit/render.zig            # where does this file stand?
zone explain src/kernel/math/sqrt.zig src/portal.zig  # may I write this import?
```

The second form works whether or not the import exists yet, which is the point:
every tool in this class makes you write the line and run the whole gate to find
out. Paths are taken as typed, resolved against your shell's own directory — the
way an editor tab has it. And when a path is real but unjudged it says which of
the four reasons applies, because "untracked by git" and "excluded by the
contract" call for opposite actions.

The verdict reaches the exit code, so the question is also a shell question:

```bash
zone explain from.zig to.zig && $EDITOR from.zig
```

## The Verbs

- **`verify`** – does the code obey the contract. The default, and what CI runs.
- **`status`** – verify, plus the census: files per zone, hop histogram, what is
  sealable for free, and the entry-file bypass count for everything still open.
- **`list`** – every package in the tree, governed or not, with the next command
  for each.
- **`show`** – the resolved contract, as zoning understood it rather than as you
  typed it.
- **`map`** – the stack, drawn.
- **`explain FILE`** – one file's zone, reach, grants, and importers.
- **`explain FROM TO`** – whether that one import is legal, and the clause that
  decides.
- **`draft DIR`** – the contract `DIR`'s graph already obeys. `--write` files it
  at `DIR`'s root, and refuses if either layout already governs `DIR`.

Options: `--package NAME`, `--under DIR` (monorepos), `--root PATH`,
`--language NAME`, `--complete`, `--write`, `--untracked`, `--suggest`, `--json`,
`--no-color`.

Exit codes: `0` clean, `1` a violation or a stale declaration, `2` the contract
or the invocation is malformed. `explain` uses the same three, so `1` there means
the file is in violation or the import is not allowed.

## Wiring It Into CI

One step, and no Rust toolchain in the job:

```yaml
- name: Import topology
  run: uv run --no-project --with zoning==1.1.0 zone verify --complete
```

No build, no compile database, no network beyond fetching itself. Pin the
version: a gate whose verdict can change without a commit is not a gate.

`--complete` adds the one claim no law can make: every package in scope has a
contract. Without it a clean run says nothing whatsoever about the package
somebody added last week, and adoption that cannot notice a new ungoverned
package rots back toward zero, one package at a time.

It forgives a vendored dependency, and on the right authority. A vendored package
is a package by every test this tool can run — manifest, source, an import graph —
and it is nonetheless not yours: its architecture is decided in the repository it
came from, which is where its contract lives. The obvious fix is an allowlist,
which drifts the moment somebody vendors a second thing, so the dialect reads the
manifest instead. `build.zig.zon` spells one `.brigade = .{ .path = "brigade" }`,
and a build that had not said so would not link. The fact is already written down
and the compiler maintains it.

With a toolchain already in the job, the same binary comes from crates.io:

```yaml
- run: cargo install zoning --locked && zone verify --complete
```

## Languages

Zig first, because Zig is where the absence of any boundary is total. Nothing
above is about Zig, though: zones, seals, keeps, cycles, and reach are
statements about a graph of files, and a graph of files has the same shape in
every language.

So the language-specific surface is deliberately tiny, and a `Dialect` carries
only what genuinely varies:

- which extensions are source,
- how an import is spelled,
- whether a given spec names a path inside the module or a dependency outside
  it,
- the comment and string conventions, so imports are read from code alone rather
  than from a line that mentions one,
- which filenames declare a package, so `list` can find one nobody has governed,
- what name a manifest gives the package it declares, so a contract is called
  what the build already calls it,
- which modules the language always provides, so no contract spends a `use` line
  on the standard library,
- and which directories a manifest calls an in-tree dependency, so coverage knows
  whose package is whose.

Resolution, the graph, all seven laws, and every rendering are shared. That is on
purpose: a dialect that could resolve paths its own way is a dialect that could
disagree with the others about what a cycle is.

A contract names its own `language`, so a polyglot monorepo is one run rather than
one run per dialect. `--language` sets the default for a package that has not
said.

Python is the second dialect, and it earns its keep on the same absence: a
package's `import`s reach any module on `sys.path`, and nothing about the
language says which of *your* packages may reach which. `from a.b import c`
and `import a.b.c` both resolve dotted names against real files — a module or
a package's `__init__.py`, whichever exists — and a relative import's leading
dots climb exactly as many directories as they count, independent of how deep
`from` nests. The standard library grant is a fixed list read out of `sys`
across supported versions, not a per-repo guess, so `zone verify` never asks a
contract to `use` `json` or `pathlib`: those names are not the repository's to
grant, in any Python version the interpreter has shipped for years and will
keep shipping.

## Build and Test

Everything is cargo, except the parity gate:

```bash
cargo build --release
cargo test
cargo clippy --all-targets
```

`tools/differential.py` is the parity gate against the Python implementation
this was rewritten from. It mutates each real contract the way a person breaks
one; drop every seal, drop every guest list, squeeze the reach ceiling, revoke
every variance, invert the entire stack; and requires both implementations to
produce the same set of findings, law by law and file by file. It covers the six
laws both implementations have; `use` postdates the rewrite and has no Python
twin to check it against. `crates/zoning/tests/properties.rs` closes that gap
with an independent oracle instead: `the_use_law_flags_exactly_the_outside_imports_no_grant_covers`
grows randomized packages with real outside imports, drafts a randomized grant
table over them, and hand-computes which imports should be refused —
the same role `the_cycle_law_finds_what_a_slower_algorithm_finds` plays for
`cycle` (Tarjan checked against an O(n³) fixed-point relaxation). `use` is
pinned by fixtures *and* generative coverage now, not fixtures alone.

Agreeing on a clean tree proves nothing, because every gate agrees that nothing
is wrong.

## Where This Came From

This began as a Python tool called `ward`, inside a monorepo, guarding one Zig
package. It was correct and it was invisible: a gate that lives in your repo is
a gate only your repo runs, and the four packages that most needed it were the
four that had been split out into repositories of their own.

The `.zone` files were sitting there in each of them, declaring boundaries,
judged by nobody. The rewrite is a static binary with no dependencies for
exactly that reason: a gate has to be cheaper to install than to ignore.

Apache-2.0. Built at [The Billy Company](https://billylives.com).
