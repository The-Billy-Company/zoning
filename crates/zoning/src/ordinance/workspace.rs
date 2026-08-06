//! The greater document: one file a package's contract may hang off.
//!
//! Ten packages in one tree are ten copies of the same four lines — the language they
//! are written in, where each keeps its source, what fronts it, how far an import may
//! climb — and boilerplate is not a cosmetic problem. Four lines nobody reads are four
//! lines nobody notices are wrong, and a per-package fact that is actually a
//! whole-repository fact will eventually disagree with itself in one package and pass.
//!
//! So a file may hold a set of packages together:
//!
//! ```text
//! workspace {
//!     member   libs/kernels/*
//!     root     src
//!     language zig
//!     facade   root.zig
//!     limit    reach to 1 hop
//! }
//! ```
//!
//! and each member's own contract says only what makes it different. The link points
//! down, the way `[workspace] members` and `[tool.uv.workspace]` do: the greater
//! document claims its members, and a member is a member because something above it
//! said so, never because it declared a parent. One less thing to keep in sync, and a
//! package cannot quietly attach itself to a policy nobody granted it.
//!
//! Inheritance is one hop. A member finds the nearest workspace above it that claims
//! it, and that is the whole search — a chain of overriding defaults is a thing you
//! debug, not a thing you read.

use std::fs;
use std::path::{Path, PathBuf};

use super::fault::{Fault, Span};
use super::{Use, parse, plat};
use crate::survey::Dialect;

/// What a member inherits, resolved, and the file it came from.
pub(super) struct Shared {
    /// The workspace file, for reporting where a declaration came from.
    pub path: PathBuf,
    pub root: Option<String>,
    pub language: Option<&'static dyn Dialect>,
    pub facade: Vec<String>,
    /// Grants, each already marked inherited so the bench can price them differently.
    pub uses: Vec<Use>,
    pub max_hops: Option<u32>,
}

/// The nearest workspace above `zone` that claims the package it governs.
///
/// # Errors
/// Returns the workspace file's own fault, rendered against its own source. A member
/// inheriting from a contract nobody can read must fail loudly: the alternative is a
/// package that silently loses its defaults and is judged against a contract half of
/// which was never applied.
pub(super) fn enclosing(
    zone: &Path,
    fallback: &'static dyn Dialect,
) -> Result<Option<Shared>, Fault> {
    let home = plat::anchor(zone);
    let home = home.canonicalize().unwrap_or(home);
    let ceiling = crate::repo_root(&home);
    // Strictly above the package: a workspace file sitting in the very directory it
    // would claim is a file arguing with itself, and the climb stops at the worktree
    // for the same reason discovery does — above it lies `/`.
    for dir in home.ancestors().skip(1).take_while(|d| d.starts_with(&ceiling)) {
        for candidate in plat::charters(dir) {
            let Some(shared) = claimed(&candidate, dir, &home, fallback)? else { continue };
            return Ok(Some(shared));
        }
    }
    Ok(None)
}

/// The inheritance this candidate offers `home`, if it claims it at all.
fn claimed(
    candidate: &Path,
    dir: &Path,
    home: &Path,
    fallback: &'static dyn Dialect,
) -> Result<Option<Shared>, Fault> {
    let Ok(source) = fs::read_to_string(candidate) else { return Ok(None) };
    // A neighbour's contract is not this run's to validate: a file that cannot parse
    // fails when it is judged in its own right, and refusing to resolve *this* package
    // because some unrelated `.zone` beside it is mid-edit would make one broken file
    // stop the whole tree. A workspace that does claim us is a different matter, and
    // faults below.
    let Ok(tree) = parse::parse(&source, candidate) else { return Ok(None) };
    let Some(workspace) = tree.workspace else { return Ok(None) };
    let Ok(rel) = home.strip_prefix(dir) else { return Ok(None) };
    let members = crate::pattern::Globs::new(workspace.members.iter().map(|m| &m.text));
    if !members.matches(&plat::posix(rel)) {
        return Ok(None);
    }

    let language = match &workspace.language {
        Some(named) => Some(crate::survey::dialect(&named.text).ok_or_else(|| {
            let known: Vec<&str> = crate::survey::dialects().iter().map(|d| d.name()).collect();
            Fault::at(
                format!(
                    "no language named `{}` — this build reads {}",
                    named.text,
                    known.join(", ")
                ),
                named.span.clone(),
                &source,
            )
        })?),
        None => None,
    };
    let dialect = language.unwrap_or(fallback);
    let shared = scoped(&workspace.uses, &source)?;
    let mut uses = super::resolve_uses(shared, &[], dialect, candidate, &source)?;
    for grant in &mut uses {
        grant.inherited = true;
    }

    Ok(Some(Shared {
        path: candidate.to_path_buf(),
        root: workspace.root.as_ref().map(|t| t.text.clone()),
        language,
        facade: workspace.facade.iter().map(|t| t.text.clone()).collect(),
        uses,
        max_hops: workspace.reach.map(|(hops, _)| hops),
    }))
}

/// The workspace's grants, refusing the one scope it cannot mean.
///
/// `use httpx by session` is the ordinary spelling of a grant, and it is exactly the
/// spelling a workspace cannot honour: `session` is a zone, zones belong to a package,
/// and each member's are its own. Resolved silently against an empty zone list it would
/// cover nothing at all — a grant that reads like a permission and grants none — so it
/// is a fault, with the two scopes that do work named in it.
fn scoped<'a>(uses: &'a [parse::Use], source: &str) -> Result<&'a [parse::Use], Fault> {
    for grant in uses {
        for word in &grant.scope {
            if !word.text.contains(['/', '*', '?', '[', '.']) {
                return Err(Fault::at(
                    format!(
                        "`{}` is a zone name, and a workspace has no zones — every member \
                         names its own. Scope a shared grant with a path glob, or leave the \
                         `by` off to grant every member",
                        word.text
                    ),
                    word.span.clone(),
                    source,
                ));
            }
        }
    }
    Ok(uses)
}

/// Where a declaration came from, for a fault that must not point at the wrong file.
///
/// An inherited value has a span in a file the reader is not looking at, so it is
/// reported against the member's own header with the workspace named in the message.
pub(super) fn blame(shared: &Shared, message: &str, span: Span, source: &str) -> Fault {
    let file =
        shared.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    Fault::at(format!("{message} — inherited from `{file}`"), span, source)
}
