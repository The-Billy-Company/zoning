# zoning: An Import-Topology Gate

- [Overview](#overview)
- [Why Not a Code Review?](#why-not-a-code-review)
- [Should You Be Using This?](#should-you-be-using-this)
- [Support](#support)
- [Install](#install)
- [The Language](#the-language)
  - [The Package Block](#the-package-block)
  - [Zones](#zones)
  - [Seals](#seals)
  - [Keeps](#keeps)
  - [Structural Laws](#structural-laws)
  - [Variances](#variances)
- [The Six Laws](#the-six-laws)
- [Reading a Map](#reading-a-map)
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
✗ zoning [irregex]: 311 files, 1436 imports, 2 violation(s), 3 allowed
src/kernel/regex/glean/differential_test.zig:32:1: [zone] zone `regex` imports up into `query` (`kernel/query/query.zig`) — imports may only point down the stack
src/exec/cold/emit/render.zig:31:1: [seal] reaches past the seal on `kernel/scan/` into `kernel/scan/simd.zig` — enter through `kernel/scan/scan.zig`

zone: Move the dependency down the stack, or ratify the edge with `variance zone … because "…"`.

seal: Re-export what the caller needs from the seal's entry file, or widen that seal's `open to` list.
```

Every failure closes with the remedy for its law. A gate whose output does not
say what to do next is a gate somebody eventually silences.

It reads the tree, not a build system; there is no project model to configure,
no graph to rebuild, nothing to keep in step. Judging 311 files and 1436
imports takes 30 milliseconds, from a static binary with zero dependencies.

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

- **Python** – [import-linter](https://github.com/seddonym/import-linter)
  already does this, over the boundary Python gives you.
- **Java** – [ArchUnit](https://www.archunit.org/), likewise.
- **Go, wanting the boundary Go draws** – `go vet` and `internal/`, and nothing
  else to install.
- **A language that draws no boundary, or a boundary coarser than the one you
  want** – here.

The dividing line is whether your language already separates the parts you mean
to keep apart. Zig does not, which is why this exists.

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

The binary ships from crates.io and PyPI, prebuilt:

```bash
cargo install zoning          # the static binary
uv tool install zoning        # the same thing, through PyPI
pipx install zoning
```

Or build it here:

```bash
cargo build --release         # target/release/zoning
```

## The Language

A package opts in by writing `contract/<name>.zone` beside its code, and here
is a whole one:

```
package irregex {
    root   src
    facade root.zig
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

limit  reach to 5 hops
forbid cycles across directories

variance zone a.zig -> b.zig
    because "…and here is exactly what would retire this"
```

Comments are `//`. Globs mean what they mean to a Python reader; the matcher is
CPython's `glob.translate(..., recursive=True, include_hidden=True)`,
reimplemented and pinned by tests against that reference.

### The Package Block

`root` names the source directory, relative to the contract's grandparent
(`contract/x.zone` → `../../src`). `facade` names the files that may reach
anywhere: the module's public face, which by construction re-exports everything
and therefore imports everything. `exclude` drops paths from judgment entirely.

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

`zoning verify --suggest` drafts the stanzas for today's violations and writes
nothing. A machine can find the edge; it cannot supply the reason, and the
reason is the entire value.

## The Six Laws

Six, and closed on purpose. A boundary language whose vocabulary grows per
project stops being a language and becomes a config file.

- **`zone`** – an import points up the stack.
- **`seal`** – an import reaches past a sealed directory's entry file.
- **`keep`** – an importer not on the guest list reaches into a kept region.
- **`cycle`** – an import cycle crosses a directory boundary.
- **`reach`** – an import climbs more `../` than the ceiling allows.
- **`escape`** – an import climbs out of the module root entirely.

Each exists because a compiler structurally cannot enforce it.

## Reading a Map

`zoning map` draws the contract the way gravity works, so "imports point down
the page" stops being a rule you memorise and becomes a thing you can see:

```
zoning map · irregex · 26 zones, high to low
──────────────────────────────────────────────────────────────────────────
 25 │ ffi      ██············  7 ↓4      surface/ffi/**
 24 │ api      █·············  2 ↓3    ⊘ surface/api.zig surface/api_test.…
 23 │ session  ██████········ 35 ↓13  ⊙  exec/session/**
 22 │ cold     ███████······· 44 ↓16  ⊙  exec/cold/**
 21 │ cli      █·············  6 ↓4      surface/cli/**
  …
  8 │ regex    ██████████████ 92 ↓4   ⊙  kernel/regex/**
  7 │ scan     ███··········· 16 ↓4      kernel/scan/**
  5 │ math     ███··········· 18 ↓3      kernel/math/**
  2 │ fault    █·············  1 ↓1      fault.zig
  1 │ assay    █·············  5      ⊙  assay/**
  0 │ portal   █·············  1         portal.zig
──────────────────────────────────────────────────────────────────────────
 310 files · 1432 imports · 5 seals · 3 keeps · reach ≤ 5 hops
 3 ratified variance(s) — each one names what would retire it
```

The bar is how many files the zone holds. `↓N` is how many *distinct* zones
beneath it that zone actually reaches into, which is the number worth staring
at: a zone that reaches into every zone below it is not a layer, it is a pile.
`⊙` marks a sealed directory, `⊘` an anchored guest list.

## The Verbs

- **`verify`** – does the code obey the contract. The default, and what CI runs.
- **`status`** – verify, plus the census: files per zone, hop histogram, what is
  sealable for free, and the entry-file bypass count for everything still open.
- **`list`** – which packages are governed, and which are not.
- **`show`** – the resolved contract, as zoning understood it rather than as you
  typed it.
- **`map`** – the stack, drawn.

Options: `--package NAME`, `--under DIR` (monorepos), `--root PATH`,
`--dialect NAME`, `--untracked`, `--suggest`, `--json`, `--no-color`.

Exit codes: `0` clean, `1` a violation or a stale declaration, `2` the contract
or the invocation is malformed.

## Wiring It Into CI

One step, and no Rust toolchain in the job:

```yaml
- name: Import topology
  run: uv run --no-project --with zoning==0.1.0 zoning verify
```

No build, no compile database, no network beyond fetching itself. Pin the
version: a gate whose verdict can change without a commit is not a gate.

With a toolchain already in the job, the same binary comes from crates.io:

```yaml
- run: cargo install zoning --locked && zoning verify
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
- and the comment and string conventions, so imports are read from code alone
  rather than from a line that mentions one.

Resolution, the graph, all six laws, and every rendering are shared. That is on
purpose: a dialect that could resolve paths its own way is a dialect that could
disagree with the others about what a cycle is.

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
produce the same set of findings, law by law and file by file.

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
