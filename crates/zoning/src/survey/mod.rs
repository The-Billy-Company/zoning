//! The survey: what the code actually does, before anyone judges it.
//!
//! A dialect says how an import is spelled; this module turns that into the one
//! structure every law reads — a set of judged files and the resolved edges between
//! them. Resolution is lexical, not filesystem: `../` in an import is a spelling,
//! and collapsing it needs no syscall, because the walk already proved the tree.
//!
//! Two kinds of import never become edges. One whose target is outside the judged
//! set — a coworker's untracked new file — is counted and dropped: it is not yet
//! part of the committed architecture. One that resolves to nothing at all is also
//! dropped, because the compiler owns compile-time resolution and a gate that races
//! it on someone's half-finished edit is just noise.

mod dialect;
mod prose;
mod walk;
mod zig;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub use dialect::{Dialect, Import, all as dialects, by_name as dialect};
pub use prose::Prose;
pub use walk::tracked;

use crate::pattern::Pattern;

/// One resolved intra-module import.
pub struct Edge {
    /// Module-relative path of the importing file.
    pub src: String,
    /// Module-relative path of the imported file.
    pub dst: String,
    /// 1-based line of the import statement in `src`.
    pub line: usize,
    /// How many directories the written spec climbs.
    pub hops: u32,
    /// The literal as written.
    pub spec: String,
}

impl Edge {
    /// The stable name a `variance` uses to ratify this edge.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{} -> {}", self.src, self.dst)
    }
}

/// An import whose path climbs out of the module root.
///
/// The build cannot follow it — a dependency that leaves the module has to be a
/// named module — so this is an architectural statement, not a typo, and it is the
/// one resolution failure worth a law.
pub struct Escape {
    /// Module-relative path of the importing file.
    pub src: String,
    /// The literal as written.
    pub spec: String,
    /// 1-based line of the import statement.
    pub line: usize,
}

impl Escape {
    /// The stable name a `variance` uses to ratify this escape.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{} -> {}", self.src, self.spec)
    }
}

/// What to survey.
pub struct Ask<'a> {
    /// The enclosing worktree, for reporting paths a human can click.
    pub repo_root: &'a Path,
    /// The module root the relative paths hang off.
    pub module_root: &'a Path,
    /// Globs whose files are held out of the judged set entirely.
    pub exclude: &'a [Pattern],
    /// The language.
    pub dialect: &'static dyn Dialect,
    /// The version-controlled file set, or `None` to judge the whole walk.
    pub tracked: Option<&'a HashSet<PathBuf>>,
}

/// The resolved import graph of one module.
pub struct Survey {
    /// The enclosing worktree.
    pub repo_root: PathBuf,
    /// Absolute path the module-relative paths hang off.
    pub module_root: PathBuf,
    /// The judged set: tracked, non-excluded module files, sorted.
    pub files: Vec<String>,
    /// Every edge with both endpoints in `files`.
    pub edges: Vec<Edge>,
    /// Every import that climbed out of the module.
    pub escapes: Vec<Escape>,
    /// External module names imported by name rather than by path.
    pub named: Vec<String>,
    /// Imports whose target is outside the judged set.
    pub skipped: usize,
    /// Exclude globs that actually held a file back.
    pub spent: HashSet<String>,
    /// The language this survey was read in.
    pub dialect: &'static dyn Dialect,
}

impl Survey {
    /// Resolve every import under the module root.
    #[must_use]
    pub fn of(ask: &Ask<'_>) -> Self {
        let mut spent = HashSet::new();
        let admitted: Vec<(PathBuf, String)> =
            walk::source_files(ask.module_root, ask.dialect.extensions())
                .into_iter()
                .filter(|(abs, rel)| {
                    let held: Vec<&str> = ask
                        .exclude
                        .iter()
                        .filter(|p| p.matches(rel))
                        .map(Pattern::as_str)
                        .collect();
                    if held.is_empty() {
                        ask.tracked.is_none_or(|t| t.contains(abs))
                    } else {
                        spent.extend(held.into_iter().map(str::to_owned));
                        false
                    }
                })
                .collect();
        let judged: HashSet<&str> = admitted.iter().map(|(_, rel)| rel.as_str()).collect();

        let (mut edges, mut escapes, mut skipped) = (Vec::new(), Vec::new(), 0);
        let mut named: HashSet<String> = HashSet::new();
        for (abs, src) in &admitted {
            let Ok(raw) = fs::read_to_string(abs) else { continue };
            let code = ask.dialect.prose().code_only(&raw);
            let lines = Lines::of(&raw);
            for import in ask.dialect.imports(&raw, &code) {
                let line = lines.at(import.offset);
                if !ask.dialect.is_local(&import.spec) {
                    named.insert(import.spec);
                    continue;
                }
                let Some(dst) = resolve(src, &import.spec) else {
                    escapes.push(Escape { src: src.clone(), spec: import.spec, line });
                    continue;
                };
                if !judged.contains(dst.as_str()) {
                    skipped += 1;
                    continue;
                }
                edges.push(Edge {
                    hops: import.spec.matches("../").count() as u32,
                    src: src.clone(),
                    dst,
                    line,
                    spec: import.spec,
                });
            }
        }

        let mut named: Vec<String> = named.into_iter().collect();
        named.sort();
        Self {
            repo_root: ask.repo_root.to_path_buf(),
            module_root: ask.module_root.to_path_buf(),
            files: admitted.into_iter().map(|(_, rel)| rel).collect(),
            edges,
            escapes,
            named,
            skipped,
            spent,
            dialect: ask.dialect,
        }
    }

    /// Module-relative path → repo-relative, for a report line an editor can open.
    #[must_use]
    pub fn rel(&self, path: &str) -> String {
        let absolute = self.module_root.join(path);
        absolute.strip_prefix(&self.repo_root).map_or_else(
            |_| absolute.to_string_lossy().into_owned(),
            |p| p.to_string_lossy().replace('\\', "/"),
        )
    }

    /// Every file's importers, keyed by the imported file.
    #[must_use]
    pub fn importers(&self) -> HashMap<&str, Vec<&str>> {
        let mut out: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            out.entry(&edge.dst).or_default().push(&edge.src);
        }
        out
    }
}

/// Collapse `dir(src) / spec` lexically. `None` when it climbs out of the module.
fn resolve(src: &str, spec: &str) -> Option<String> {
    let mut parts: Vec<&str> =
        src.rsplit_once('/').map_or_else(Vec::new, |(dir, _)| dir.split('/').collect());
    for part in spec.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// Byte offset → 1-based line, over one file.
struct Lines(Vec<usize>);

impl Lines {
    fn of(text: &str) -> Self {
        Self(text.bytes().enumerate().filter(|&(_, b)| b == b'\n').map(|(i, _)| i).collect())
    }

    fn at(&self, offset: usize) -> usize {
        self.0.partition_point(|&start| start < offset) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_resolve_lexically_and_escapes_are_detected() {
        assert_eq!(resolve("a/b/c.zig", "d.zig").as_deref(), Some("a/b/d.zig"));
        assert_eq!(resolve("a/b/c.zig", "../d.zig").as_deref(), Some("a/d.zig"));
        assert_eq!(resolve("a/b/c.zig", "../../d.zig").as_deref(), Some("d.zig"));
        assert_eq!(resolve("a/b/c.zig", "../../../d.zig"), None, "climbs out");
        assert_eq!(resolve("root.zig", "a/b.zig").as_deref(), Some("a/b.zig"));
        assert_eq!(resolve("root.zig", "../x.zig"), None);
        assert_eq!(resolve("a/b.zig", "./c.zig").as_deref(), Some("a/c.zig"));
    }

    #[test]
    fn line_numbers_are_one_based_and_land_on_the_statement() {
        let text = "one\ntwo\nthree\n";
        let lines = Lines::of(text);
        assert_eq!(lines.at(0), 1);
        assert_eq!(lines.at(4), 2);
        assert_eq!(lines.at(8), 3);
    }
}
