# Rust import topology

Rust uses the same `.zone` language and seven laws as Zig and Python. The Rust
adapter translates source references into the shared graph; it does not introduce
a second policy engine or Rust-specific contract clauses.

Cargo editions 2018, 2021 and 2024 are supported. A governed target in edition
2015 or an unknown edition fails explicitly: its name-resolution rules cannot
be inferred from the newer editions. Standalone sources without a Cargo
manifest use the documented 2018-and-later path spelling. Adding a Cargo
manifest makes the edition and target ownership authoritative.

## Write a contract

Put a contract beside the crate's `Cargo.toml`. Root it at `.` to cover production
code, integration tests and examples together, or at `src` to govern only source
under that directory, including its inline tests. Paths in the contract are
relative to that root.

```
package engine {
    root .
    language rust
    facade src/lib.rs
}

zones {
    values  src/value/**
    parse   src/parser/**
    run     src/runtime/**
    checks  tests/** examples/**
}

seal src/parser through mod.rs
keep src/runtime/** to src/lib.rs tests/** examples/**
use hashbrown by values
limit reach to 2 hops
forbid cycles across directories
```

Run `zone verify --complete --untracked` during adoption when new source files
have not been added to Git. Normal verification judges tracked files; exclusions
and tracking policy remain visible limits on what was checked.

The example declares an architecture, not a template to paste unchanged. A zone
must own real files, a grant must be exercised, and a variance must identify its
reason and removal condition. `zone draft` derives today's topology; seals and
keeps remain decisions for the package owner.

## What the graph means

A Rust package can contain several crates. Each library, binary, integration
test, example, benchmark and build script has its own root. `crate::` begins at
that target's root, not whichever `lib.rs` is nearest on disk. The adapter asks
native `cargo metadata --format-version 1 --no-deps --locked --offline` for
targets and direct dependencies once per invocation-owned session, then parses
source without building or executing build scripts or procedural macros. The
metadata command does not fetch dependencies; its `resolve` graph is absent with
`--no-deps`. This is a source graph, not a compiler's resolved HIR graph. See
[Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html).
Target ownership follows [Cargo targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html).

The metadata subprocess owns a Unix process group or Windows job, a 20-second
leader deadline and a 16 MiB limit for each captured stream. When the leader
exits, remaining members are terminated before capture readers are joined, so
an inherited output pipe cannot keep a completed query waiting on a descendant.
The process owner also cleans up on ordinary errors and unwinding. The narrow
[process-wrap API](https://github.com/watchexec/process-wrap/tree/v11.0.0)
provides the platform operations; no package build is needed for metadata.

A test, example or binary importing its package's own library crosses a Rust
crate boundary. For example, `engine::Tree` is an outside dependency named
`engine` and needs a `use engine` grant, even when both targets are covered by
one package contract. It is not an internal edge back into a configured facade.
By contrast, `crate::helpers` in that test names the test's own helper module.

Module declarations establish source ownership. They are not dependency edges:
`mod child;` followed by a child's ordinary `super::Thing` must not manufacture a
cycle merely because the parent declares the child. Real imports and qualified
references create edges. Logical module scopes matter for inline modules and
`#[path]`; file depth alone does not determine `super`. Rust permits both
`name.rs` and `name/mod.rs`, but not both for the same declaration. The
[module rules](https://doc.rust-lang.org/reference/items/modules.html) specify
these cases and the different path bases inside inline modules.

Grouped imports, aliases, reexports and qualified paths must obey the same
boundary. A reference through a module's public entry is different from one that
names an internal module through an alias. For example,
`use crate::parser::Tree` enters the parser's entry even when `Tree` is
reexported from an internal file; `use crate::parser as p; use p::internal::Tree`
names that internal module. Rust's
[use declarations](https://doc.rust-lang.org/reference/items/use-declarations.html)
explain how these bindings are introduced.

Facades remain explicit. A `lib.rs` or `mod.rs` filename does not automatically
exempt its contents from policy. A configured facade may reach every zone, but
ordinary source may not import the facade from inside the package. Shared
definitions belong in a lower module that internal callers can name directly.
Do not mark every module entry as a facade to suppress findings.

Reach measures the reference as written: a `crate::` anchor does not spend parent
hops. `super` walks logical modules, including inline modules. Resolved file
paths and original spelling are separate so normalizing a path cannot change
the policy being checked. See [Rust paths](https://doc.rust-lang.org/reference/paths.html).

## Targets, dependencies and conditional source

External grants name the crate identifier visible in source, including a Cargo
dependency alias. Package names, library names and source identifiers need not
be identical. Workspace inheritance belongs to manifest interpretation; a path
dependency does not become exempt from governance merely because it is local.
Development dependencies serve tests, examples and benchmarks. Build-script
dependencies are separate from the package's runtime dependencies. See
[Cargo dependency kinds and renaming](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html).

The source graph takes a conservative union of conditional branches. This is
not the same as enabling every Cargo feature: `not(feature = "x")`, target
predicates and `cfg(test)` describe other branches. Alternative module files
must remain alternatives rather than silently selecting whichever was read
first. A cycle in the source union is a source-topology finding, not a claim
that all its edges coexist in one executable. See
[conditional compilation](https://doc.rust-lang.org/reference/conditional-compilation.html).

## Limits and failure behavior

Syntax analysis cannot reproduce all Rust name resolution. Macro expansion can
introduce modules, imports and shadowing; procedural macros and build scripts
can generate source that is not present in the governed tree. The compiler
resolves imports and macros together before later resolution stages. See
[name resolution](https://doc.rust-lang.org/reference/names/name-resolution.html).

This is not a call graph or type-inference pass. Method calls and type-relative
associated items may need compiler information that source scopes do not supply.
The import introducing a type is still governed, but resolving every later use
of that type is a separate obligation. A clean source graph does not establish
that the compiled program has no other semantic dependencies.

Paths plainly written inside macro arguments are a separate case from generated
tokens. A parser visitor does not automatically parse `assert_eq!` arguments as
expressions: `syn::Macro` stores a token stream and exposes explicit body parsing.
A clean syntax graph must not be presented as proof about expanded macro output.
See [syn macro bodies](https://docs.rs/syn/latest/syn/struct.Macro.html).

Literal `include!` targets must be governed `.rs` source files. They are parsed
in the including module's scope. A missing file, a nonliteral generated path or
an included file with another extension fails explicitly; the adapter does not
silently omit that code. `include_str!` and `include_bytes!` are data, not Rust
source dependencies.

Unreadable governed source, malformed Rust and an unsupported operation needed
to construct the declared graph are technical failures, not clean verdicts over
an unnoticed partial graph. The CLI and language server must use the same survey
result. The ordinary compiler remains responsible for type checking and actual
build configurations; zoning checks written topology within its reported scope.

## Prior art and the design choice

[ArchUnitRust](https://github.com/dragonsec-ai/ArchUnitRust#known-limitations)
offers architecture rules as Rust tests. Its extraction uses `syn` and Cargo
metadata, with explicit syntax-only, conditional-union and generated-source
limits. That is useful precedent for stating exactly what a source graph proves.

[cargo-modules](https://github.com/regexident/cargo-modules) analyzes internal
structure with rust-analyzer infrastructure. It exposes feature, target and
test-configuration selection and can reject dependency cycles. Its richer
semantic model carries a larger toolchain and analysis surface than zoning's
source gate.

[ArchTest](https://github.com/tdymel/arch_test) checks layers, parent access and
cycles. [cargo-deny](https://embarkstudios.github.io/cargo-deny/checks/bans/index.html)
governs dependency crates, including bans and duplicate versions. Those are
different scopes from zoning's file-level seals and kept regions.

Zoning keeps `zone`, `seal`, `keep`, `cycle`, `reach`, `escape` and `use` as its
policy vocabulary. Rust changes how the evidence is read, not what those laws
mean. A stronger future semantic backend should produce the same typed graph
and use the same contracts rather than growing a parallel Rust rule language.

The adapter depends on `syn`, `proc-macro2` and `toml` for structured parsing,
`process-wrap` for process-tree ownership, and `libc` for Unix error constants.
The graph laws retain their existing implementation.

## Library calls

Library callers create one `survey::Session` per invocation and pass it through
`Ask::session` for every package in that run. `Survey::of` returns a `Result`;
handle a source or metadata error before passing the graph to the judge. Start
a fresh session after changing manifests or Cargo target files. The CLI and
language server manage this lifetime automatically.
