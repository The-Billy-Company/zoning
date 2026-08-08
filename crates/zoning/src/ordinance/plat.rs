//! The plat: which packages this tree holds, and which `.zone` files govern them.
//!
//! A plat is the map of parcels in a tract, and that is what this module produces —
//! the land, not the law. Everything here answers a question about the filesystem:
//! where the contracts are, which directory each one governs, which packages exist at
//! all, and which of those somebody else vendored.
//!
//! Depth is not the tool's business. A repository that *is* one package carries its
//! contract at its own root; a monorepo buries them under `libs/kernels/<pkg>/`; and
//! neither should have to say so on the command line.

use std::collections::HashMap;
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// A package the tool can see: where it is, what language, and whether it is governed.
///
/// "Parcel" because the answer worth having is about land, not about contracts — a
/// list of the packages that *have* a contract cannot say whether adoption is
/// finished, and the ungoverned ones are the whole reason to ask.
pub struct Parcel {
    /// Repo-relative directory holding the package manifest.
    pub dir: String,
    /// The language whose manifest was found there.
    pub language: &'static str,
    /// Its contracts, if it declares any.
    pub contracts: Vec<PathBuf>,
    /// The enclosing package that vendored this one, when a manifest above says so.
    pub vendored_by: Option<String>,
}

/// The directory a `.zone` file governs.
///
/// A contract sits at the root of what it organizes — `acme/acme.zone` governs
/// `acme/` — because that is where every other configuration a package carries
/// already lives, and a boundary tool that demanded its own drawer would be asking for
/// a directory before it had earned one.
///
/// The one exception is a file inside a `contract/` drawer, which governs the drawer's
/// parent. That was the only layout zoning accepted before, contracts are checked in,
/// and a tool does not get to invalidate a repository's on-disk shape to tidy its own
/// rules. Both spellings resolve to the same anchor, so nothing downstream can tell
/// them apart.
#[must_use]
pub fn anchor(zone: &Path) -> PathBuf {
    let dir = held(zone.parent());
    if dir.file_name().is_some_and(|name| name == "contract") {
        return held(dir.parent()).to_path_buf();
    }
    dir.to_path_buf()
}

/// A directory that is somewhere, given a parent that may be nowhere.
///
/// `Path::parent` answers `Some("")` for a bare filename, and an empty path is not a
/// directory anything can be joined onto — so the shell's own directory stands in,
/// which is what a bare filename meant.
fn held(dir: Option<&Path>) -> &Path {
    match dir {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

/// Every `.zone` file that would anchor to `dir`, nearest spelling first.
///
/// Both layouts, in one list, because [`anchor`] cannot tell them apart and neither may
/// anything that asks what governs a directory.
pub(super) fn charters(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = [dir.to_path_buf(), dir.join("contract")]
        .iter()
        .filter_map(|home| fs::read_dir(home).ok())
        .flat_map(|entries| entries.flatten().map(|entry| entry.path()))
        .filter(|path| {
            path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zone")) && path.is_file()
        })
        .collect();
    found.sort();
    found
}

/// The contract already governing `dir`, whichever spelling it uses.
///
/// The question `draft --write` has to ask before it writes. "Is there a file with the
/// name I was about to use" is the wrong one: the two layouts anchor to the same
/// directory, so it would cheerfully file a second contract beside an existing one and
/// leave the package governed by two documents that can disagree.
#[must_use]
pub fn governing(dir: &Path) -> Option<PathBuf> {
    charters(dir).into_iter().find(|path| governs(path))
}

/// Every zoning contract in the tree, at any depth.
///
/// `under` narrows the sweep to named subtrees when a caller wants only part of a large
/// repository; it is a filter, never a requirement.
#[must_use]
pub fn discover(repo_root: &Path, under: &[String]) -> Vec<PathBuf> {
    sweep(repo_root, under).0
}

/// Every package in the tree, governed or not, sorted by directory.
#[must_use]
pub fn parcels(repo_root: &Path, under: &[String]) -> Vec<Parcel> {
    let (contracts, mut found) = sweep(repo_root, under);
    let vendored = borrowed(repo_root, &found);
    for parcel in &mut found {
        let home = repo_root.join(&parcel.dir);
        parcel.contracts = contracts.iter().filter(|c| anchor(c) == home).cloned().collect();
        parcel.vendored_by = vendored.get(&parcel.dir).cloned();
    }
    found
}

/// Which of these packages another one vendors, by that other one's own manifest.
///
/// Read from the manifests already found rather than from every manifest on disk, so
/// this costs one small file read per package and answers nothing about trees the
/// sweep was told to leave alone.
fn borrowed(repo_root: &Path, found: &[Parcel]) -> HashMap<String, String> {
    let mut owned = HashMap::new();
    for parcel in found {
        let Some(dialect) = crate::survey::dialect(parcel.language) else { continue };
        let base = repo_root.join(&parcel.dir);
        for manifest in dialect.manifests() {
            let Ok(text) = fs::read_to_string(base.join(manifest)) else { continue };
            for dir in dialect.vendored(&text) {
                let rel = posix(&Path::new(&parcel.dir).join(&dir));
                owned.insert(rel.trim_start_matches("./").to_owned(), parcel.dir.clone());
            }
        }
    }
    owned
}

/// One sweep, both answers: the contracts, and the package roots.
///
/// Both questions are about the same directory entries, so asking them separately would
/// cross a large monorepo twice to learn less than one pass already saw.
///
/// Git answers first when it can. Finding contracts by reading every directory of a
/// large monorepo costs seconds — measured at 7 on one of them — and a gate that slow
/// stops being run, which is a correctness problem wearing a performance costume. One
/// `git ls-files` covers tracked *and* untracked-but-not-ignored files in ~40 ms, so a
/// contract written a moment ago is still found, and one sitting in an ignored
/// directory is correctly invisible. Outside a worktree the walk still answers.
fn sweep(repo_root: &Path, under: &[String]) -> (Vec<PathBuf>, Vec<Parcel>) {
    let (mut contracts, mut parcels) = match indexed(repo_root) {
        Some(paths) => sift(repo_root, paths.iter().map(String::as_str)),
        None => walked(repo_root),
    };
    if !under.is_empty() {
        let scoped: Vec<String> = under.iter().filter_map(|u| narrowed(repo_root, u)).collect();
        let within = |rel: &str| {
            scoped.iter().any(|u| u == "." || rel == u || rel.starts_with(&format!("{u}/")))
        };
        contracts.retain(|c| c.strip_prefix(repo_root).is_ok_and(|r| within(&posix(r))));
        parcels.retain(|p| within(&p.dir));
    }
    contracts.sort();
    parcels.sort_by(|a, b| a.dir.cmp(&b.dir));
    parcels.dedup_by(|a, b| a.dir == b.dir);
    (contracts, parcels)
}

/// One `--under` argument, as the repo-relative prefix the sweep compares against.
///
/// The sweep's rows are repo-relative posix paths, so a raw argument is only ever
/// compared correctly when it was typed in exactly that spelling. `./libs/kernels` and an
/// absolute path — both of which a shell's tab-completion produces, and both of which a
/// CI script is entitled to write — matched no row at all, which is the worst possible
/// answer to give a gate: a narrowing to nothing reads as a clean tree.
///
/// So the argument is resolved as a place rather than compared as a string. A path
/// outside the tree being swept narrows to nothing, which is what it means, and `scope`'s
/// climb depends on that: it re-sweeps each ancestor with the same arguments, where a
/// subtree of the original directory genuinely is out of scope.
fn narrowed(repo_root: &Path, under: &str) -> Option<String> {
    let named = Path::new(under);
    let full = if named.is_absolute() { named.to_path_buf() } else { repo_root.join(named) };
    let rel = posix(settled(&full).strip_prefix(settled(repo_root)).ok()?);
    Some(if rel.is_empty() { ".".to_owned() } else { rel })
}

/// A path resolved as far as the filesystem can answer, and lexically for the rest.
///
/// Both operands above have to be resolved by the *same* rule, and `canonicalize`
/// alone is not one: it refuses a path that does not exist, and either operand is
/// entitled to be one - a gate may be pointed at a directory somebody is about to
/// add, and `..` may climb to somewhere nobody ever created. Falling back to the
/// unresolved path is what made that dangerous, because `join` does not fold `..`
/// away: `<root>/../elsewhere` still carried `<root>` as a literal prefix, so a
/// sibling of the tree stripped as though it sat inside it and a gate aimed outside
/// the tree answered "clean" instead of "not here". That went unseen because the one
/// platform where it is caught by accident is macOS, whose temp dir canonicalizes
/// `/var` to `/private/var` - the prefixes then disagree for a reason that has
/// nothing to do with the `..`, and the same test passes for the wrong cause.
///
/// So: resolve the deepest ancestor that does exist, which is what makes a symlinked
/// `/tmp` compare equal to its target, and fold the tail lexically, which is what
/// keeps a path that points outside the tree outside it.
fn settled(path: &Path) -> PathBuf {
    let mut lexical = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !lexical.pop() {
                    lexical.push(part);
                }
            }
            named => lexical.push(named),
        }
    }
    let mut tail = Vec::new();
    let mut walk = lexical.as_path();
    loop {
        if let Ok(real) = walk.canonicalize() {
            return tail.iter().rev().fold(real, |deep, name| deep.join(name));
        }
        match (walk.parent(), walk.file_name()) {
            (Some(up), Some(name)) => {
                tail.push(name);
                walk = up;
            }
            _ => return lexical,
        }
    }
}

/// Every contract and manifest git can see, tracked or merely present.
fn indexed(repo_root: &Path) -> Option<Vec<String>> {
    let mut command = Command::new("git");
    command
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--"])
        .arg("*.zone")
        .current_dir(repo_root);
    for dialect in crate::survey::dialects() {
        for manifest in dialect.manifests() {
            command.arg(manifest).arg(format!("*/{manifest}"));
        }
    }
    let out = command.output().ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect()
    })
}

/// Sort repo-relative paths into the contracts and the package roots they imply.
fn sift<'a>(repo_root: &Path, paths: impl Iterator<Item = &'a str>) -> (Vec<PathBuf>, Vec<Parcel>) {
    let (mut contracts, mut parcels) = (Vec::new(), Vec::new());
    for rel in paths {
        let (dir, name) = rel.rsplit_once('/').unwrap_or((".", rel));
        if Path::new(name).extension().is_some_and(|e| e.eq_ignore_ascii_case("zone")) {
            let path = repo_root.join(rel);
            if governs(&path) {
                contracts.push(path);
            }
        } else if let Some(dialect) =
            crate::survey::dialects().iter().find(|d| d.manifests().contains(&name))
        {
            parcels.push(Parcel {
                dir: dir.to_owned(),
                language: dialect.name(),
                contracts: Vec::new(),
                vendored_by: None,
            });
        }
    }
    (contracts, parcels)
}

/// The same answer by reading directories, for a tree git does not know about.
fn walked(repo_root: &Path) -> (Vec<PathBuf>, Vec<Parcel>) {
    let mut stack = vec![repo_root.to_path_buf()];
    let mut found: Vec<String> = Vec::new();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                if !crate::survey::SKIP.contains(&name.as_ref()) {
                    stack.push(entry.path());
                }
            } else if let Ok(rel) = entry.path().strip_prefix(repo_root) {
                found.push(posix(rel));
            }
        }
    }
    sift(repo_root, found.iter().map(String::as_str))
}

/// Does this file govern a package a run should judge?
///
/// Two different questions, and both are settled by the first declaration.
///
/// The first is whether the file is ours at all. The extension never was: BIND has
/// written DNS zones into `*.zone` since long before this tool existed. While contracts
/// lived in a `contract/` drawer the two could not collide; now that one sits wherever a
/// package keeps its configuration, a sweep that parsed every match would fail a
/// repository's build over its nameserver data. So a contract leads with `package` or
/// `workspace`, and a file opening with anything else was not addressed to us. That is a
/// claim of authorship rather than a guess about content, and it only governs sweeping —
/// a file named on the command line is parsed, and faults, like any other.
///
/// The second is whether it governs a graph. A file whose whole job is holding a
/// workspace together has no package of its own; it is read where the members are
/// resolved, and offering it up to be judged would fail a clean tree for the crime of
/// being organised. A file that leads with `workspace` and *also* declares a package is
/// a root package with members below it, and is judged like any other.
///
/// A file that cannot be parsed is always included, whichever way it leads. Silently
/// dropping a malformed contract would stop it being judged at all, which is the one
/// failure this gate must not have.
fn governs(path: &Path) -> bool {
    // A contract's header comment can be long, and a DNS zone declares itself in its
    // first line or two, so a bounded read settles every foreign file. Only a file that
    // opens as a workspace is read in full, to see whether it also holds a package.
    match lead(&head(path)) {
        Some("package") | None => true,
        Some("workspace") => match fs::read_to_string(path) {
            Ok(text) => match super::parse::parse(&text, path) {
                Ok(tree) => tree.package.is_some(),
                Err(_) => true,
            },
            Err(_) => false,
        },
        Some(_) => false,
    }
}

/// The start of a file, as much of it as identity can need.
fn head(path: &Path) -> String {
    let mut bytes = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        let _ = file.take(8192).read_to_end(&mut bytes);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The first word of the first declaration, comments and blank lines skipped.
fn lead(text: &str) -> Option<&str> {
    text.lines()
        .map(|line| line.split("//").next().unwrap_or_default().trim())
        .find(|line| !line.is_empty())
        .and_then(|line| line.split_whitespace().next())
}

/// A relative path with forward slashes, whatever the platform spells them as.
pub(super) fn posix(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "a test that cannot build its fixture has failed")]

    use super::*;

    #[test]
    fn a_contract_governs_the_directory_it_sits_in() {
        assert_eq!(anchor(Path::new("libs/acme/acme.zone")), Path::new("libs/acme"));
        assert_eq!(anchor(Path::new("acme.zone")), Path::new("."));
    }

    #[test]
    fn the_older_drawer_still_governs_its_parent() {
        assert_eq!(anchor(Path::new("libs/acme/contract/acme.zone")), Path::new("libs/acme"));
        assert_eq!(anchor(Path::new("contract/acme.zone")), Path::new("."));
    }

    #[test]
    fn identity_is_the_leading_declaration() {
        assert_eq!(lead("// a comment\n\npackage x {\n"), Some("package"));
        assert_eq!(lead("workspace {\n"), Some("workspace"));
        assert_eq!(lead("$TTL 3600\n@ IN SOA ns1.example.com.\n"), Some("$TTL"));
        assert_eq!(lead("; bind comment\n$ORIGIN example.com.\n"), Some(";"));
        assert_eq!(lead("\n\n// nothing but comments\n"), None);
    }

    #[test]
    fn a_narrowing_is_a_place_and_not_a_spelling() {
        let root = std::env::temp_dir().join("zoning-narrowed-test/libs/kernels");
        fs::create_dir_all(&root).expect("temp tree");
        let tree = root.parent().and_then(Path::parent).expect("two levels up");
        for spelling in ["libs/kernels", "./libs/kernels", "libs/./kernels"] {
            assert_eq!(narrowed(tree, spelling).as_deref(), Some("libs/kernels"), "{spelling}");
        }
        assert_eq!(narrowed(tree, &root.display().to_string()).as_deref(), Some("libs/kernels"));
        // The tree itself narrows to everything rather than to nothing, and a sibling of it
        // narrows to nothing rather than to everything — the two ways this can fail open.
        assert_eq!(narrowed(tree, ".").as_deref(), Some("."));
        assert_eq!(narrowed(tree, "../elsewhere"), None);
        // …and asked of a tree that is not reached through a symlink, which is the only
        // place the last line above is a real question. macOS resolves its temp dir's
        // `/var` to `/private/var`, so a climb out of the tree fails the prefix test there
        // for a reason that has nothing to do with the climb — and the same assertion held
        // on macOS while `<tree>/../elsewhere` stripped clean through on Linux. A fixture
        // whose canonical path is itself keeps both platforms answering the same question.
        let plain = fs::canonicalize(std::env::temp_dir())
            .expect("a temp dir to resolve")
            .join("zoning-narrowed-plain/libs/kernels");
        fs::create_dir_all(&plain).expect("temp tree");
        let bare = plain.parent().and_then(Path::parent).expect("two levels up");
        assert_eq!(narrowed(bare, "libs/kernels").as_deref(), Some("libs/kernels"));
        assert_eq!(narrowed(bare, ".").as_deref(), Some("."));
        assert_eq!(narrowed(bare, "../elsewhere"), None);
        assert_eq!(narrowed(bare, "libs/kernels/../../../elsewhere"), None);
    }
}
