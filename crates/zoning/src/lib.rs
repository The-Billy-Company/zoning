//! **zoning** — declare where a package's imports may go, then judge the real graph.
//!
//! Most languages give you *some* boundary: Go has `internal/`, Python has a package
//! graph, TypeScript has an exports map, Rust has `pub(crate)`. Inside a single Zig
//! package there is nothing at all — every import is a filesystem path, any file may
//! name any other, and because analysis is lazy a genuine import cycle compiles
//! clean. Architecture there is convention with nothing behind it.
//!
//! A package opts in by writing `contract/<name>.zone` next to its code:
//!
//! ```text
//! package irregex {
//!     root   src
//!     facade root.zig
//! }
//!
//! zones {                        // low to high; an import may not point up
//!     portal   portal.zig
//!     math     kernel/math/**
//!     regex    kernel/regex/**
//! }
//!
//! seal kernel/regex through regex.zig     // enter a deep module by its door
//! keep surface/api.zig to root.zig        // and this region has a guest list
//!
//! limit  reach to 5 hops
//! forbid cycles across directories
//!
//! variance zone a.zig -> b.zig because "…and here is what retires it"
//! ```
//!
//! Seven laws, one exception mechanism, and every exception must say how it gets
//! retired — a variance that stops matching is a hard failure, so paying the debt
//! forces deleting the entry. Exception lists shrink instead of accreting.
//!
//! # The shape of the tool
//!
//! Five modules, each deep enough to be worth its boundary:
//!
//! | Module | Question it owns |
//! |---|---|
//! | [`ordinance`] | What does this contract say, and is it believable? |
//! | [`survey`] | What does the code actually import? |
//! | [`judge`] | Where do those two disagree? |
//! | [`draft`] | What contract would this graph already obey? |
//! | [`report`] | How does a person or a machine read the answer? |
//!
//! [`survey`] is where languages beyond Zig arrive: a [`survey::Dialect`] carries
//! only what genuinely varies — extensions, how an import is spelled, whether a spec
//! names something local, which modules the language hands out for free — while
//! resolution, the graph, and all seven laws stay shared. Two dialects cannot
//! disagree about what a cycle is. A contract names its own language, so one run
//! judges a polyglot repository.
//!
//! # As a library
//!
//! ```no_run
//! use std::path::Path;
//! use zoning::{judge, ordinance::Ordinance, survey::{Ask, Survey}};
//!
//! let zig = zoning::survey::dialect("zig").expect("the zig dialect ships in-tree");
//! let contract = Ordinance::read(Path::new("contract/irregex.zone"), zig)?;
//! let repo = Path::new(".");
//! let found = Survey::of(&Ask {
//!     repo_root: repo,
//!     module_root: &contract.module_root,
//!     exclude: &contract.exclude,
//!     dialect: contract.dialect,
//!     tracked: None,
//! });
//! let verdict = judge::judge(&found, &contract);
//! assert!(verdict.ok());
//! # Ok::<(), zoning::ordinance::Fault>(())
//! ```

pub mod draft;
pub mod judge;
pub mod ordinance;
pub mod pattern;
pub mod report;
pub mod survey;

use std::path::{Path, PathBuf};

/// The enclosing worktree, so zoning works from source or from an installed binary.
///
/// Falls back to `start` when there is no repository above it: a directory with a
/// `contract/` in it is still worth judging, and refusing to run outside version
/// control would make the tool useless in exactly the sandboxes people test it in.
#[must_use]
pub fn repo_root(start: &Path) -> PathBuf {
    let here = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    here.ancestors()
        .find(|c| c.join(".git").exists())
        .map_or_else(|| here.clone(), Path::to_path_buf)
}
