use std::path::{Path, PathBuf};

use zoning::ordinance;
use zoning::survey::{Ask, Dialect, Survey};

use super::Tracked;

/// Which subtree this run governs, and every contract in it.
///
/// The scope is where you are standing. A gate that answers about the whole repository
/// no matter which directory you invoke it from cannot be used *inside* one package —
/// and in a monorepo it is also the slow answer, since it reads every other package to
/// tell you about yours. At a repository root, which is where CI stands, the two models
/// are the same run.
///
/// Two completions keep that from ever being a surprise. A directory with nothing
/// governed beneath it looks *up* for the package that encloses it, so a question asked
/// from `src/kernel/regex` is still answered by the contract that governs it. And an
/// explicit `--root` never climbs: a caller who names the subtree means it, including
/// when the answer is "nothing here".
///
/// The climb stops at the worktree, and that boundary is load-bearing rather than
/// tidy: above it lies a directory git cannot enumerate, where discovery falls back to
/// reading the filesystem — and a tool that walked out of the repository and up toward
/// `/` looking for a contract would not be slow, it would be hung.
pub(super) fn scope(here: &Path, pinned: bool, under: &[String]) -> (PathBuf, Vec<PathBuf>) {
    let here = here.canonicalize().unwrap_or_else(|_| here.to_path_buf());
    let found = ordinance::discover(&here, under);
    if pinned || !found.is_empty() {
        return (here, found);
    }
    let ceiling = zoning::repo_root(&here);
    for parent in here.ancestors().skip(1).take_while(|p| p.starts_with(&ceiling)) {
        let contracts = ordinance::discover(parent, under);
        if !contracts.is_empty() {
            return (parent.to_path_buf(), contracts);
        }
    }
    (here, Vec::new())
}

/// What to call a package whose directory is `rel`, for a column of names.
///
/// A package at the root of its own repository has `.` for a directory, and a column of
/// dots names nothing — so the enclosing directory answers for it, which is also the
/// name it will have when somebody drafts its contract.
pub(super) fn basename(root: &Path, rel: &str) -> String {
    let dir = if rel == "." { root.to_path_buf() } else { root.join(rel) };
    dir.file_name().map_or_else(|| rel.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// A path as short as it can be while still saying where it is: relative to the shell's
/// own directory when it lies under it, its last two components otherwise.
pub(super) fn tail(path: &Path) -> String {
    if let Ok(here) = std::env::current_dir() {
        if path == here {
            return ".".to_owned();
        }
        if let Ok(rel) = path.strip_prefix(&here) {
            return rel.display().to_string();
        }
    }
    let mut last: Vec<_> = path.components().rev().take(2).collect();
    last.reverse();
    last.iter().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// The name this package's own manifest gives it, if it gives it one.
pub(super) fn declared(dir: &Path, dialect: &'static dyn Dialect) -> Option<String> {
    dialect
        .manifests()
        .iter()
        .filter_map(|m| std::fs::read_to_string(dir.join(m)).ok())
        .find_map(|text| dialect.declared(&text))
        .filter(|name| !name.is_empty())
}

/// `to`, spelled the way you would have to type it from `from`.
///
/// Only ever one level up here, so this is the difference between a hint you can paste
/// and a hint you have to translate.
pub(super) fn pathdiff(from: &Path, to: &Path) -> String {
    if to == from {
        return ".".to_owned();
    }
    match to.strip_prefix(from) {
        Ok(rel) => rel.display().to_string(),
        Err(_) if from.parent() == Some(to) => "..".to_owned(),
        Err(_) => to.display().to_string(),
    }
}

/// Why a draft found no module, in the terms that decide what to do about it.
///
/// Three different situations arrive here and they call for opposite next moves, so a
/// single message covering all of them would be wrong twice. A directory holding a
/// manifest and nothing else is complete as it stands. A directory whose source is all
/// nested packages wants each of those drafted. And a directory with source this build
/// cannot read is a dialect problem, where naming a manifest nobody wrote — `build.zig`,
/// in a Rust tree — sends the reader looking for a file that was never the issue.
pub(super) fn barren(
    dir: &Path,
    source: &str,
    dialect: &'static dyn Dialect,
    nested: &[String],
) -> String {
    // `tail` renders the shell's own directory as `.`, which is right in a path column and
    // reads badly mid-sentence, so the place carries its own preposition.
    let shown = tail(&dir.join(source));
    let where_ = if shown == "." { "here".to_owned() } else { format!("under {shown}") };
    let lang = dialect.name();
    if !nested.is_empty() {
        return format!(
            "no {lang} source {where_} of its own — every file belongs to a nested package \
             ({}). Draft each of those instead",
            nested.join(", ")
        );
    }
    if dialect.manifests().iter().any(|m| dir.join(m).exists()) {
        return format!(
            "no {lang} source {where_}. A contract governs a module's imports, and a manifest \
             is not a module: `{}` declares this package rather than belonging to it, so there \
             is nothing left here to govern",
            dialect.manifests().join("` / `")
        );
    }
    let known: Vec<&str> = zoning::survey::dialects().iter().map(|d| d.name()).collect();
    format!(
        "no {lang} source {where_}. If the code is in another language, `--language NAME` \
         reads it — this build knows {}",
        known.join(", ")
    )
}

/// Survey a package directory that may have no contract yet.
pub(super) fn probe(
    dir: &Path,
    source: &str,
    dialect: &'static dyn Dialect,
    tracked: &mut Tracked<'_>,
    nested: &[String],
) -> Survey {
    let exclude: Vec<zoning::pattern::Pattern> =
        nested.iter().map(|glob| zoning::pattern::Pattern::new(glob)).collect();
    Survey::of(&Ask {
        repo_root: dir,
        module_root: &dir.join(source),
        exclude: &exclude,
        dialect,
        tracked: tracked.of(dialect),
    })
}

/// Where a package's module lives, and globs for the packages nested inside it.
///
/// `src/` when there is one, the directory itself otherwise — the convention the `root`
/// setting defaults to, so a draft and the contract it becomes read the same tree.
///
/// A vendored dependency with its own `build.zig` is a different package that happens to
/// sit in this directory tree. Judging its files as this package's would blame it for an
/// architecture it never agreed to, and would put a second package's directories in this
/// one's zone stack — so they are excluded, and the enclosing package keeps only the
/// dependency it genuinely has: the module, declared with `use`.
pub(super) fn module(dir: &Path) -> (&'static str, Vec<String>) {
    let source = if dir.join("src").is_dir() { "src" } else { "." };
    let inside = if source == "." { String::new() } else { format!("{source}/") };
    let nested = ordinance::parcels(dir, &[])
        .into_iter()
        .filter(|p| p.dir != ".")
        .filter_map(|p| p.dir.strip_prefix(&inside).map(|within| format!("{within}/**")))
        .collect();
    (source, nested)
}
