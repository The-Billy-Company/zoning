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
mod python;
mod walk;
mod zig;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub use dialect::{Dialect, Import, all as dialects, by_name as dialect};
pub use prose::Prose;
pub use walk::{SKIP, tracked};

use crate::pattern::Pattern;

/// One resolved intra-module import.
#[derive(Clone)]
pub struct Edge {
    /// Module-relative path of the importing file.
    pub src: String,
    /// Module-relative path of the imported file.
    pub dst: String,
    /// 1-based line of the import statement in `src`.
    pub line: usize,
    /// 1-based character column of the import statement.
    pub col: usize,
    /// Character width of the import head.
    pub width: usize,
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

/// An import that leaves the package: by name, or by climbing out of the root.
///
/// The two are the same event with different spellings. A named module is a
/// dependency the build system resolves, which the `use` law governs; a path that
/// climbs past the root is a dependency the build system *cannot* resolve, which the
/// `escape` law governs. Both need the importing file, the literal, and the line, so
/// both are this.
pub struct Departure {
    /// Module-relative path of the importing file.
    pub src: String,
    /// The module name, or the literal path that climbed out.
    pub spec: String,
    /// 1-based line of the import statement.
    pub line: usize,
    /// 1-based character column of the import statement.
    pub col: usize,
    /// Character width of the import head.
    pub width: usize,
}

impl Departure {
    /// The stable name a `variance` uses to ratify this dependency.
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
    pub escapes: Vec<Departure>,
    /// Every import naming a module outside the package, located.
    pub outside: Vec<Departure>,
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
                // The file that declares a package is not part of the module it declares.
                // It only ever collides with the judged set when the module root *is* the
                // package root, and a build script's imports are the build graph's, not
                // this module's — governing them would put the build's own dependencies
                // in the stack it is describing.
                .filter(|(_, rel)| !ask.dialect.manifests().contains(&rel.as_str()))
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

        // Every top-level name this survey considers its own — the first path segment
        // of a nested file, or the bare stem of one loose at the module root. Only a
        // dialect whose import spelling is a module name rather than a path (Python)
        // reads this; it is how such a dialect tells its own package from an external
        // one that happens to share the same leading word.
        let roots: Vec<&str> = {
            let mut set: Vec<&str> = admitted
                .iter()
                .map(|(_, rel)| match rel.split_once('/') {
                    Some((first, _)) => first,
                    None => stem(rel, ask.dialect.extensions()),
                })
                .collect();
            set.sort_unstable();
            set.dedup();
            set
        };

        let (mut edges, mut escapes, mut skipped) = (Vec::new(), Vec::new(), 0);
        let mut outside = Vec::new();
        for (abs, src) in &admitted {
            let Ok(raw) = fs::read_to_string(abs) else { continue };
            let code = ask.dialect.prose().code_only(&raw);
            let lines = Lines::of(&raw);
            for import in ask.dialect.imports(src, &roots, &raw, &code) {
                let line = lines.at(import.offset);
                let col = column(&raw, import.offset);
                let width = raw[import.offset..]
                    .chars()
                    .take_while(|character| !character.is_whitespace() && *character != '(')
                    .count()
                    .max(1);
                if !ask.dialect.is_local(&import.spec) {
                    outside.push(Departure {
                        src: src.clone(),
                        spec: import.spec,
                        line,
                        col,
                        width,
                    });
                    continue;
                }
                let Some(dst) = resolve(src, &import.spec) else {
                    escapes.push(Departure {
                        src: src.clone(),
                        spec: import.spec,
                        line,
                        col,
                        width,
                    });
                    continue;
                };
                if dst == *src {
                    // A file cannot architecturally depend on itself; a dialect whose
                    // module name and file name can coincide (Python's package importing
                    // its own top-level name from within) would otherwise draw one.
                    continue;
                }
                if !judged.contains(dst.as_str()) {
                    skipped += 1;
                    continue;
                }
                edges.push(Edge {
                    hops: import.spec.matches("../").count() as u32,
                    src: src.clone(),
                    dst,
                    line,
                    col,
                    width,
                    spec: import.spec,
                });
            }
        }

        Self {
            repo_root: ask.repo_root.to_path_buf(),
            module_root: ask.module_root.to_path_buf(),
            files: admitted.into_iter().map(|(_, rel)| rel).collect(),
            edges,
            escapes,
            outside,
            skipped,
            spent,
            dialect: ask.dialect,
        }
    }

    /// This module's real file set, with one hypothetical import in place of its graph.
    ///
    /// For asking a law about an edge that does not exist yet. The file set stays real
    /// so zone membership, seals, and keeps all resolve exactly as they would after the
    /// edit; only the graph is replaced, which is why the answer comes from the laws
    /// themselves instead of from a second, drifting copy of them.
    #[must_use]
    pub fn hypothetically(&self, edge: Edge) -> Self {
        Self {
            repo_root: self.repo_root.clone(),
            module_root: self.module_root.clone(),
            files: self.files.clone(),
            edges: vec![edge],
            escapes: Vec::new(),
            outside: Vec::new(),
            skipped: 0,
            spent: self.spent.clone(),
            dialect: self.dialect,
        }
    }

    /// Every outside module this package imports, once each, sorted.
    #[must_use]
    pub fn modules(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.outside.iter().map(|o| o.spec.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names
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

    /// Best source span for a finding reported at `path:line`.
    #[must_use]
    pub fn span_at(&self, path: &str, line: usize) -> (usize, usize) {
        self.edges
            .iter()
            .map(|edge| (&edge.src, edge.line, edge.col, edge.width))
            .chain(
                self.escapes
                    .iter()
                    .chain(&self.outside)
                    .map(|edge| (&edge.src, edge.line, edge.col, edge.width)),
            )
            .find(|(source, edge_line, _, _)| source == &path && *edge_line == line)
            .map_or((1, 1), |(_, _, col, width)| (col, width))
    }
}

/// A root-level file's own name, minus whichever of `extensions` it carries.
fn stem<'a>(rel: &'a str, extensions: &[&str]) -> &'a str {
    extensions.iter().find_map(|ext| rel.strip_suffix(&format!(".{ext}"))).unwrap_or(rel)
}

fn column(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .rsplit_once('\n')
        .map_or(source[..offset.min(source.len())].chars().count(), |(_, tail)| {
            tail.chars().count()
        })
        + 1
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
