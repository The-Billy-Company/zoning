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
//! Six laws, one exception mechanism:
//!
//! ```text
//! zones { … }      ordered low → high. An import may not point up.
//! seal … through   a directory is a deep module: outsiders use its entry file.
//! keep … to        a region only the named importers may reach at all.
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

mod fault;
mod law;
mod lex;
mod parse;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub use fault::{Fault, Span};
pub use law::Law;

use crate::pattern::{Globs, Pattern};

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
    /// Globs over everyone else allowed in.
    pub importers: Globs,
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
    /// The `../` ceiling, if one was set.
    pub max_hops: Option<u32>,
    /// Ratified exceptions, in the order they were written.
    pub variances: Vec<Variance>,
    granted: HashMap<(Law, String), usize>,
}

impl Ordinance {
    /// Read, parse, and resolve one `.zone` file.
    ///
    /// # Errors
    /// Returns a [`Fault`] carrying the span of the first problem — unreadable file,
    /// syntax error, or a claim the filesystem contradicts.
    pub fn read(path: &Path) -> Result<Self, Fault> {
        let source =
            fs::read_to_string(path).map_err(|e| Fault::at(e.to_string(), Span::head(path), ""))?;
        resolve(parse::parse(&source, path)?, path, &source)
    }

    /// The variance ratifying `law` over `subject`, if one was written.
    #[must_use]
    pub fn variance(&self, law: Law, subject: &str) -> Option<&Variance> {
        self.granted.get(&(law, subject.to_owned())).map(|&i| &self.variances[i])
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

fn resolve(tree: parse::Tree, path: &Path, source: &str) -> Result<Ordinance, Fault> {
    let pkg = tree.package;
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
    let (variances, granted) = resolve_variances(&tree.variances, path, source)?;

    Ok(Ordinance {
        path: path.to_path_buf(),
        package: pkg.name.text,
        module_root,
        facade: Globs::new(pkg.facade.iter().map(|t| &t.text)),
        exclude: pkg.exclude.iter().map(|t| Pattern::new(&t.text)).collect(),
        zones,
        seals,
        keeps,
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

/// Every `contract/*.zone` a repository declares.
///
/// Two shapes, and the same answer for both: a repository that *is* the package,
/// carrying `contract/*.zone` at its own root, and a monorepo hosting packages
/// under configured roots (`libs/kernels/<pkg>/contract/`). Neither wins special
/// casing downstream, because the module root always hangs off the zone file's own
/// grandparent.
#[must_use]
pub fn discover(repo_root: &Path, roots: &[String]) -> Vec<PathBuf> {
    let mut found = zone_files(&repo_root.join("contract"));
    for root in roots {
        let base = repo_root.join(root);
        let Ok(entries) = fs::read_dir(&base) else { continue };
        let mut packages: Vec<PathBuf> =
            entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        packages.sort();
        for package in packages {
            found.extend(zone_files(&package.join("contract")));
        }
    }
    found
}

fn zone_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "zone") && p.is_file())
        .collect();
    found.sort();
    found
}
