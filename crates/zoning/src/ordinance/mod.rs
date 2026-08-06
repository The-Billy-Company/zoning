//! The ordinance: a package's declared import topology, as the laws will read it.
//!
//! Text goes in, a validated model comes out, and everything the grammar cannot
//! state is checked on the way — a seal naming an entry file that is not on disk,
//! two zones with the same name, two variances ratifying the same edge. All of it
//! fails closed: a contract that cannot be believed is not downgraded to a warning,
//! it stops the run.
//!
//! The resolved model carries no tokens and no spans. A law reasons about
//! architecture and should not be handed a parser's bookkeeping.
//!
//! Seven laws, one exception mechanism:
//!
//! ```text
//! zones { … }      ordered low → high. An import may not point up.
//! seal … through   a directory is a deep module: outsiders use its entry file.
//! keep … to        a region only the named importers may reach at all.
//! use … by         an outside module, and the zones granted it.
//! forbid cycles    no import cycle crossing a directory boundary (always on).
//! limit reach      a ceiling on `../` hops, so nesting tracks logical depth.
//! (escape)         always on: an import may not climb out of the module root.
//! variance … because   one ratified exception, with a mandatory reason.
//! ```
//!
//! `seal` and `keep` are orthogonal and compose: a seal narrows **what** an outsider
//! may name (the entry file, not the internals); a keep narrows **who** may name it
//! at all. Sibling independence — the thing zones structurally cannot say, because
//! peers at one height are unordered — is a keep per peer.
//!
//! `use` is the only declaration pointing outward. Everything else here partitions
//! and orders files this package owns; a `use` names something it does not own and
//! says which of its zones may depend on it. The language's own always-available
//! modules are [`Dialect::ambient`] and need no
//! grant, so a `use` line appears exactly where a real outside dependency does.

mod fault;
mod law;
mod lex;
mod parse;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub use fault::{Fault, Span};
pub use law::Law;

use crate::pattern::{Globs, Pattern};
use crate::survey::Dialect;

/// One height in the stack. Rank 0 is the bottom.
pub struct Zone {
    /// The name the contract gave it.
    pub name: String,
    /// Position in the stack; imports may only point at a rank at or below their own.
    pub rank: usize,
    /// The files it claims.
    pub paths: Globs,
}

/// A directory outsiders may only enter through one file.
///
/// `entry` is resolved, not spelled: a deep module is fronted either from inside
/// (`rank/rank.zig`) or from beside (`sqrt.zig` next to `sqrt/`), and the law must
/// not grade a cosmetic placement choice.
pub struct Seal {
    /// Module-relative directory.
    pub path: String,
    /// Module-relative file — the only way in.
    pub entry: String,
    /// Extra importer globs permitted past the seal.
    pub open: Globs,
}

/// A region only the named importers may reach.
///
/// The complement of a seal. Where a seal says "come in through this door", a keep
/// says "this house is not yours to enter" — and the guest list is closed, so a new
/// caller cannot join by simply writing the import. Two importers are implicit and
/// unwritable: the region's own insiders, and a file's directory sibling.
pub struct Keep {
    /// Glob over the protected region.
    pub subject: Pattern,
    /// Globs over everyone else allowed in. Empty is `to nobody`.
    pub importers: Globs,
}

/// One outside module, and the zones granted it.
///
/// The grant is per module rather than per zone because a dependency is a decision
/// about the dependency: "who is allowed to carry `pcre2`" is one line to read and
/// one line to change, where a `needs` list on every zone would spread the same fact
/// across the whole file and let two zones disagree about it.
pub struct Use {
    /// The module name, as the build system resolves it.
    pub module: String,
    /// Files the grant covers. Empty grants the whole package.
    pub scope: Globs,
    /// The scopes as written — zone names survive here for echoing the contract back.
    pub written: Vec<String>,
    /// `file:line`, so a stale grant can be deleted without a search.
    pub source: String,
}

impl Use {
    /// Does this grant cover the file at `rel`?
    #[must_use]
    pub fn covers(&self, rel: &str) -> bool {
        self.scope.is_empty() || self.scope.matches(rel)
    }
}

/// One ratified exception.
pub struct Variance {
    /// Which law it excuses.
    pub law: Law,
    /// The edge key, or the `+`-joined sorted cycle membership.
    pub subject: String,
    /// Why, and what retires it.
    pub reason: String,
    /// `file:line`, so a stale one can be deleted without a search.
    pub source: String,
}

/// One package's resolved contract.
pub struct Ordinance {
    /// The `.zone` file it was read from.
    pub path: PathBuf,
    /// The package name it governs.
    pub package: String,
    /// The language its code is read in.
    pub dialect: &'static dyn Dialect,
    /// Absolute path the module-relative paths hang off.
    pub module_root: PathBuf,
    /// Files that may reach anywhere, because they re-export everything.
    pub facade: Globs,
    /// Files held out of the judged set entirely.
    pub exclude: Vec<Pattern>,
    /// The stack, low to high.
    pub zones: Vec<Zone>,
    /// Sealed directories.
    pub seals: Vec<Seal>,
    /// Kept regions.
    pub keeps: Vec<Keep>,
    /// Granted outside modules.
    pub uses: Vec<Use>,
    /// The `../` ceiling, if one was set.
    pub max_hops: Option<u32>,
    /// Ratified exceptions, in the order they were written.
    pub variances: Vec<Variance>,
    granted: HashMap<(Law, String), usize>,
}

/// Result of analysing an in-memory `.zone` document.
///
/// Editors own unsaved source buffers, so analysis cannot require a temporary
/// file. A malformed document still returns diagnostics while
/// [`Ordinance::read`] keeps its established fail-fast command-line contract.
pub struct Analysis {
    /// Resolved ordinance when the document is complete and valid.
    pub ordinance: Option<Ordinance>,
    /// Recoverable syntax or resolution diagnostics.
    pub faults: Vec<Fault>,
}

impl Ordinance {
    /// Read, parse, and resolve one `.zone` file.
    ///
    /// `fallback` is the language to read the package in when it does not name one
    /// itself, so a single-language repository never has to say what it obviously is
    /// and a polyglot one settles the question per package.
    ///
    /// # Errors
    /// Returns a [`Fault`] carrying the span of the first problem — unreadable file,
    /// syntax error, an unknown language, or a claim the filesystem contradicts.
    pub fn read(path: &Path, fallback: &'static dyn Dialect) -> Result<Self, Fault> {
        let source =
            fs::read_to_string(path).map_err(|e| Fault::at(e.to_string(), Span::head(path), ""))?;
        Self::from_source(path, &source, fallback)
    }

    /// Resolve a contract from caller-owned source.
    ///
    /// # Errors
    /// Returns the first syntax or semantic fault, matching [`Ordinance::read`].
    pub fn from_source(
        path: &Path,
        source: &str,
        fallback: &'static dyn Dialect,
    ) -> Result<Self, Fault> {
        resolve(parse::parse(source, path)?, path, source, fallback)
    }

    /// Analyse an unsaved editor buffer without touching the filesystem.
    #[must_use]
    pub fn analyze(path: &Path, source: &str, fallback: &'static dyn Dialect) -> Analysis {
        match Self::from_source(path, source, fallback) {
            Ok(ordinance) => Analysis { ordinance: Some(ordinance), faults: Vec::new() },
            Err(fault) => Analysis { ordinance: None, faults: vec![fault] },
        }
    }

    /// The variance ratifying `law` over `subject`, if one was written.
    #[must_use]
    pub fn variance(&self, law: Law, subject: &str) -> Option<&Variance> {
        self.granted.get(&(law, subject.to_owned())).map(|&i| &self.variances[i])
    }

    /// The grant letting the file at `rel` import `module`, if there is one.
    ///
    /// An [ambient](crate::survey::Dialect::ambient) module needs no grant and has
    /// none, so a caller asking about `std` gets `None` and must not read that as a
    /// refusal — [`Ordinance::may_use`] is the question with the whole answer.
    #[must_use]
    pub fn grant(&self, rel: &str, module: &str) -> Option<&Use> {
        self.uses.iter().find(|u| u.module == module && u.covers(rel))
    }

    /// May the file at `rel` import the outside module `module`?
    ///
    /// The facade gets no exemption. It has no zone, so a zone-scoped grant cannot
    /// reach it — but an unscoped `use` covers the whole package, and a public entry
    /// point re-exporting a new dependency is precisely the decision this law exists to
    /// make visible.
    #[must_use]
    pub fn may_use(&self, rel: &str, module: &str) -> bool {
        self.dialect.ambient().contains(&module) || self.grant(rel, module).is_some()
    }

    /// Every grant for `module`, whatever it is scoped to.
    ///
    /// The difference between "nobody may import this" and "somebody else may" is the
    /// difference between two repairs, so a refusal is reported with this in hand.
    pub fn grants_of(&self, module: &str) -> impl Iterator<Item = &Use> {
        self.uses.iter().filter(move |u| u.module == module)
    }

    /// Is this file part of the module's public face?
    #[must_use]
    pub fn is_facade(&self, rel: &str) -> bool {
        self.facade.matches(rel)
    }

    /// Every zone claiming this file. More than one is itself a violation.
    pub fn zones_claiming(&self, rel: &str) -> impl Iterator<Item = &Zone> {
        self.zones.iter().filter(move |z| z.paths.matches(rel))
    }

    /// The single zone claiming this file, if exactly one does.
    #[must_use]
    pub fn zone_of(&self, rel: &str) -> Option<&Zone> {
        let mut found = self.zones_claiming(rel);
        found.next().filter(|_| found.next().is_none())
    }
}

/// The stable name of a cycle: its sorted membership.
///
/// Keying on the whole membership is deliberate — a ratified cycle that grows a
/// member becomes a different subject and fails again, so excusing today's tangle
/// never excuses tomorrow's larger one.
#[must_use]
pub fn cycle_subject(members: &[String]) -> String {
    let mut sorted: Vec<&str> = members.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    sorted.join(" + ")
}

fn resolve(
    tree: parse::Tree,
    path: &Path,
    source: &str,
    fallback: &'static dyn Dialect,
) -> Result<Ordinance, Fault> {
    let pkg = tree.package;
    let dialect = match &pkg.language {
        Some(named) => crate::survey::dialect(&named.text).ok_or_else(|| {
            let known: Vec<&str> = crate::survey::dialects().iter().map(|d| d.name()).collect();
            Fault::at(
                format!(
                    "no language named `{}` — this build reads {}",
                    named.text,
                    known.join(", ")
                ),
                named.span.clone(),
                source,
            )
        })?,
        None => fallback,
    };
    let root = pkg.root.as_ref().map_or("src", |t| t.text.as_str());
    let base = path.parent().and_then(Path::parent).unwrap_or(Path::new("."));
    let module_root = base.join(root);
    if !module_root.is_dir() {
        let blame = pkg.root.as_ref().unwrap_or(&pkg.name);
        return Err(Fault::at(
            format!("source root `{root}` is not a directory"),
            blame.span.clone(),
            source,
        ));
    }
    let module_root = module_root.canonicalize().unwrap_or(module_root);

    let zones = resolve_zones(&tree.zones, source)?;
    let seals = resolve_seals(&tree.seals, &module_root, root, source)?;
    let keeps = resolve_keeps(&tree.keeps, source)?;
    let uses = resolve_uses(&tree.uses, &zones, dialect, path, source)?;
    let (variances, granted) = resolve_variances(&tree.variances, path, source)?;

    Ok(Ordinance {
        path: path.to_path_buf(),
        package: pkg.name.text,
        dialect,
        module_root,
        facade: Globs::new(pkg.facade.iter().map(|t| &t.text)),
        exclude: pkg.exclude.iter().map(|t| Pattern::new(&t.text)).collect(),
        zones,
        seals,
        keeps,
        uses,
        max_hops: tree.reach.map(|(n, _)| n),
        variances,
        granted,
    })
}

/// Rank the zones bottom-up, refusing a name that appears twice.
fn resolve_zones(declared: &[parse::Zone], source: &str) -> Result<Vec<Zone>, Fault> {
    let mut zones = Vec::with_capacity(declared.len());
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for (rank, zone) in declared.iter().enumerate() {
        if let Some(line) = seen.insert(&zone.name.text, zone.name.span.line) {
            return Err(Fault::at(
                format!("zone `{}` was already declared on line {line}", zone.name.text),
                zone.name.span.clone(),
                source,
            ));
        }
        zones.push(Zone {
            name: zone.name.text.clone(),
            rank,
            paths: Globs::new(zone.globs.iter().map(|g| &g.text)),
        });
    }
    Ok(zones)
}

/// Bind each seal to a real directory and a real entry file.
///
/// The entry may sit inside the directory (`rank/rank.zig`) or beside it
/// (`sqrt.zig` next to `sqrt/`) — both spellings of "this module's front door" are
/// in use, and a contract should not have to know which one a package chose.
fn resolve_seals(
    declared: &[parse::Seal],
    module_root: &Path,
    root: &str,
    source: &str,
) -> Result<Vec<Seal>, Fault> {
    let mut seals = Vec::with_capacity(declared.len());
    for seal in declared {
        let directory = &seal.path.text;
        if !module_root.join(directory).is_dir() {
            return Err(Fault::at(
                format!("`{directory}` is not a directory under {root}/"),
                seal.path.span.clone(),
                source,
            ));
        }
        let inside = format!("{directory}/{}", seal.entry.text);
        let beside = match directory.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/{}", seal.entry.text),
            None => seal.entry.text.clone(),
        };
        let Some(entry) = [inside, beside].into_iter().find(|c| module_root.join(c).is_file())
        else {
            return Err(Fault::at(
                format!(
                    "seal entry `{}` is neither inside `{directory}/` nor beside it",
                    seal.entry.text
                ),
                seal.entry.span.clone(),
                source,
            ));
        };
        seals.push(Seal {
            path: directory.clone(),
            entry,
            open: Globs::new(seal.open.iter().map(|g| &g.text)),
        });
    }
    Ok(seals)
}

/// One guest list per kept region — two `keep` lines for the same subject would
/// each look complete while the union silently governed.
fn resolve_keeps(declared: &[parse::Keep], source: &str) -> Result<Vec<Keep>, Fault> {
    let mut keeps = Vec::with_capacity(declared.len());
    let mut kept: HashMap<&str, usize> = HashMap::new();
    for keep in declared {
        if let Some(line) = kept.insert(&keep.subject.text, keep.subject.span.line) {
            return Err(Fault::at(
                format!(
                    "`{}` is already kept on line {line} — merge the guest lists so one \
                     line names everyone who may reach it",
                    keep.subject.text
                ),
                keep.subject.span.clone(),
                source,
            ));
        }
        keeps.push(Keep {
            subject: Pattern::new(&keep.subject.text),
            importers: Globs::new(keep.importers.iter().map(|g| &g.text)),
        });
    }
    Ok(keeps)
}

/// One grant per outside module, with its scopes resolved against the zone names.
///
/// A scope is a zone name where the architecture already has the right word for the
/// region, and a path glob where the grant is narrower than a zone. Resolving names
/// here rather than at judging time means a misspelled zone fails with a caret on the
/// word instead of quietly covering nothing.
fn resolve_uses(
    declared: &[parse::Use],
    zones: &[Zone],
    dialect: &'static dyn Dialect,
    path: &Path,
    source: &str,
) -> Result<Vec<Use>, Fault> {
    let file = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let mut uses: Vec<Use> = Vec::new();
    let mut granted: HashMap<&str, usize> = HashMap::new();
    for statement in declared {
        let mut scope: Vec<String> = Vec::new();
        let mut written: Vec<String> = Vec::new();
        for word in &statement.scope {
            written.push(word.text.clone());
            match zones.iter().find(|z| z.name == word.text) {
                Some(zone) => scope.extend(zone.paths.raw().map(str::to_owned)),
                None if word.text.contains(['/', '*', '?', '[', '.']) => {
                    scope.push(word.text.clone());
                }
                None => {
                    let names: Vec<&str> = zones.iter().map(|z| z.name.as_str()).collect();
                    return Err(Fault::at(
                        format!(
                            "no zone named `{}` — a scope is a zone name or a path glob \
                             (zones here: {})",
                            word.text,
                            names.join(", ")
                        ),
                        word.span.clone(),
                        source,
                    ));
                }
            }
        }
        for module in &statement.modules {
            if dialect.ambient().contains(&module.text.as_str()) {
                return Err(Fault::at(
                    format!(
                        "`{}` is always available in {} and needs no grant — a `use` \
                         line is for a dependency this package chose to carry",
                        module.text,
                        dialect.name()
                    ),
                    module.span.clone(),
                    source,
                ));
            }
            if let Some(&prior) = granted.get(module.text.as_str()) {
                return Err(Fault::at(
                    format!(
                        "`{}` is already granted at {} — merge the scopes so one line \
                         names everyone who may carry it",
                        module.text, uses[prior].source
                    ),
                    module.span.clone(),
                    source,
                ));
            }
            granted.insert(&module.text, uses.len());
            uses.push(Use {
                module: module.text.clone(),
                scope: Globs::new(&scope),
                written: written.clone(),
                source: format!("{file}:{}", statement.lead.span.line),
            });
        }
    }
    Ok(uses)
}

/// Ratified exceptions, indexed by what they excuse.
type Ratified = (Vec<Variance>, HashMap<(Law, String), usize>);

fn resolve_variances(
    declared: &[parse::Variance],
    path: &Path,
    source: &str,
) -> Result<Ratified, Fault> {
    let file = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let mut variances: Vec<Variance> = Vec::with_capacity(declared.len());
    let mut granted: HashMap<(Law, String), usize> = HashMap::new();
    for variance in declared {
        let Some(law) = Law::parse(&variance.law.text) else {
            return Err(Fault::at(
                format!("`{}` is not one of the six laws", variance.law.text),
                variance.law.span.clone(),
                source,
            ));
        };
        let words: Vec<String> = variance.subject.iter().map(|t| t.text.clone()).collect();
        let subject = if law == Law::Cycle {
            cycle_subject(&words)
        } else {
            format!("{} -> {}", words[0], words[1])
        };
        if let Some(&prior) = granted.get(&(law, subject.clone())) {
            return Err(Fault::at(
                format!(
                    "a second `variance {law}` for the same subject — already ratified at {}",
                    variances[prior].source
                ),
                variance.law.span.clone(),
                source,
            ));
        }
        granted.insert((law, subject.clone()), variances.len());
        variances.push(Variance {
            law,
            subject,
            reason: variance.reason.text.trim().to_owned(),
            source: format!("{file}:{}", variance.law.span.line),
        });
    }
    Ok((variances, granted))
}

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

/// Every `contract/*.zone` in the tree, at any depth.
///
/// Depth is not the tool's business: a repository that *is* the package carries
/// `contract/*.zone` at its own root, a monorepo buries them at
/// `libs/kernels/<pkg>/contract/`, and neither should have to say so on the command
/// line. `under` narrows the sweep to named subtrees when a caller wants only part of
/// a large repository; it is a filter, never a requirement.
///
/// The module root always hangs off the zone file's own grandparent, so nothing
/// downstream can tell the two shapes apart.
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
        let home = repo_root.join(&parcel.dir).join("contract");
        parcel.contracts =
            contracts.iter().filter(|c| c.parent() == Some(&home)).cloned().collect();
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
        let within =
            |rel: &str| under.iter().any(|u| rel == u || rel.starts_with(&format!("{u}/")));
        contracts.retain(|c| c.strip_prefix(repo_root).is_ok_and(|r| within(&posix(r))));
        parcels.retain(|p| within(&p.dir));
    }
    contracts.sort();
    parcels.sort_by(|a, b| a.dir.cmp(&b.dir));
    parcels.dedup_by(|a, b| a.dir == b.dir);
    (contracts, parcels)
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
            if dir.rsplit_once('/').map_or(dir, |(_, d)| d) == "contract" {
                contracts.push(repo_root.join(rel));
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

/// A relative path with forward slashes, whatever the platform spells them as.
fn posix(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}
