// Copyright (c) 2026 The Billy Company
//! The survey: what the code actually does, before anyone judges it.
//!
//! Each language turns its import spellings into located file references. Shared
//! code admits sources, preserves spans and tracking, then builds the one graph all
//! seven laws read. Walks, reads and parsing are fallible: an unreadable source
//! cannot make a partial graph pass.

mod dialect;
mod prose;
mod python;
mod rust;
mod walk;
mod zig;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub use dialect::{
    Context, Destination, Dialect, Import, Reference, Source, all as dialects, by_name as dialect,
};
pub use prose::Prose;
pub use walk::{SKIP, tracked};

use crate::pattern::Pattern;

/// Invocation-owned inputs shared by package surveys in one run.
///
/// Start a new session after changing manifests or target files. CLI runs and LSP
/// publications own separate sessions, so no process-global state can go stale.
#[derive(Default)]
pub struct Session {
    pub(crate) cargo: std::cell::RefCell<rust::Cache>,
}

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
    /// The invocation that owns cached build-system inputs.
    pub session: &'a Session,
    /// The enclosing worktree, for reporting paths a human can click.
    pub repo_root: &'a Path,
    /// The module root the relative paths hang off.
    pub module_root: &'a Path,
    /// Globs whose files are held out of the judged set entirely.
    pub exclude: &'a [Pattern],
    /// The language.
    pub dialect: &'static dyn Dialect,
    /// This package's own declared name — the contract's `package NAME`, or a
    /// draft's best guess when there is no contract yet. Empty when nobody has
    /// named it. Only a dialect whose import spelling can name a whole package by
    /// the same identifier it is installed under reads this.
    pub package: &'a str,
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
    /// Resolve every import, or report why the graph is incomplete.
    ///
    /// # Errors
    /// Returns traversal, source-read or language-resolution errors instead of a
    /// successful graph that omitted unreadable or unsupported source.
    pub fn of(ask: &Ask<'_>) -> crate::Result<Self> {
        let (sources, spent) = read_sources(ask)?;
        let context = Context {
            session: ask.session,
            module_root: ask.module_root,
            package: ask.package,
            sources: &sources,
        };
        let references = ask.dialect.references(&context)?;
        let by_path: HashMap<&str, (&Source, Lines)> = sources
            .iter()
            .map(|source| (source.path.as_str(), (source, Lines::of(&source.text))))
            .collect();
        let (mut edges, mut escapes, mut outside, mut skipped) =
            (Vec::new(), Vec::new(), Vec::new(), 0);
        for reference in references {
            let (source, lines) = by_path.get(reference.source.as_str()).ok_or_else(|| {
                format!(
                    "the {} reader named an unjudged source `{}`",
                    ask.dialect.name(),
                    reference.source
                )
            })?;
            if !source.text.is_char_boundary(reference.offset) {
                return Err(format!(
                    "{}: invalid reference offset {} from the {} reader",
                    reference.source,
                    reference.offset,
                    ask.dialect.name()
                )
                .into());
            }
            let line = lines.at(reference.offset);
            let col = column(&source.text, reference.offset);
            let width = source.text[reference.offset..]
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '(')
                .count()
                .max(1);
            match reference.destination {
                Destination::Local { path, hops } => {
                    if path == reference.source {
                        continue;
                    }
                    if !by_path.contains_key(path.as_str()) {
                        skipped += 1;
                        continue;
                    }
                    edges.push(Edge {
                        src: reference.source,
                        dst: path,
                        line,
                        col,
                        width,
                        hops,
                        spec: reference.spec,
                    });
                }
                Destination::Outside(module) => outside.push(Departure {
                    src: reference.source,
                    spec: module,
                    line,
                    col,
                    width,
                }),
                Destination::Escape => escapes.push(Departure {
                    src: reference.source,
                    spec: reference.spec,
                    line,
                    col,
                    width,
                }),
            }
        }
        edges.sort_by(|a, b| {
            (&a.src, a.line, a.col, &a.dst, &a.spec).cmp(&(&b.src, b.line, b.col, &b.dst, &b.spec))
        });
        edges.dedup_by(|a, b| {
            a.src == b.src
                && a.dst == b.dst
                && a.line == b.line
                && a.col == b.col
                && a.spec == b.spec
        });
        Ok(Self {
            repo_root: ask.repo_root.to_path_buf(),
            module_root: ask.module_root.to_path_buf(),
            files: sources.into_iter().map(|source| source.path).collect(),
            edges,
            escapes,
            outside,
            skipped,
            spent,
            dialect: ask.dialect,
        })
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

fn read_sources(ask: &Ask<'_>) -> crate::Result<(Vec<Source>, HashSet<String>)> {
    let mut spent = HashSet::new();
    let mut sources = Vec::new();
    for (absolute, path) in walk::source_files(ask.module_root, ask.dialect.extensions())? {
        if ask.dialect.manifests().contains(&path.as_str()) {
            continue;
        }
        let held: Vec<&str> =
            ask.exclude.iter().filter(|p| p.matches(&path)).map(Pattern::as_str).collect();
        if !held.is_empty() {
            spent.extend(held.into_iter().map(str::to_owned));
            continue;
        }
        if ask.tracked.is_some_and(|tracked| !tracked.contains(&absolute)) {
            continue;
        }
        let text = fs::read_to_string(&absolute).map_err(|error| {
            format!(
                "{}: cannot read source: {error}; restore readable UTF-8 source before judging",
                absolute.display()
            )
        })?;
        sources.push(Source { path, text });
    }
    Ok((sources, spent))
}

/// Adapt a path-spelling language to the shared reference contract.
fn lexical(
    context: &Context<'_>,
    prose: &Prose,
    imports: impl Fn(&str, &[&str], &str, &str, &[u8]) -> Vec<Import>,
    is_local: impl Fn(&str) -> bool,
    extensions: &[&str],
) -> Vec<Reference> {
    let mut roots: Vec<&str> = context
        .sources
        .iter()
        .map(|source| {
            source
                .path
                .split_once('/')
                .map_or_else(|| stem(&source.path, extensions), |(first, _)| first)
        })
        .collect();
    roots.sort_unstable();
    roots.dedup();
    let mut found = Vec::new();
    for source in context.sources {
        let code = prose.code_only(&source.text);
        for import in imports(&source.path, &roots, context.package, &source.text, &code) {
            let destination = if is_local(&import.spec) {
                resolve(&source.path, &import.spec).map_or(Destination::Escape, |path| {
                    Destination::Local { path, hops: import.spec.matches("../").count() as u32 }
                })
            } else {
                Destination::Outside(import.spec.clone())
            };
            found.push(Reference {
                source: source.path.clone(),
                offset: import.offset,
                spec: import.spec,
                destination,
            });
        }
    }
    found
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
