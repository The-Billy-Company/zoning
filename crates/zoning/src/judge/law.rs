//! The seven checks themselves.
//!
//! Each reads the survey and writes findings onto the bench. They are separate
//! passes rather than one fused walk because they answer different questions about
//! the same edge, and a reader should be able to open exactly the one that failed.

use std::collections::{HashMap, HashSet};

use super::{Bench, cycle};
use crate::ordinance::{Law, Seal, cycle_subject};

impl Bench<'_> {
    /// **zone** — every file belongs to exactly one height, and imports point down.
    pub(super) fn zones(&mut self) {
        let (survey, ordinance) = (self.survey, self.ordinance);

        // Totality first: an ungoverned file is a hole in the law, not a pass.
        let mut ranks: HashMap<&str, usize> = HashMap::new();
        let mut claimed: HashMap<&str, usize> = HashMap::new();
        for rel in &survey.files {
            if ordinance.is_facade(rel) {
                continue;
            }
            let matched: Vec<&str> = ordinance
                .zones_claiming(rel)
                .map(|z| {
                    *claimed.entry(z.name.as_str()).or_default() += 1;
                    z.name.as_str()
                })
                .collect();
            match ordinance.zone_of(rel) {
                Some(zone) => {
                    ranks.insert(rel, zone.rank);
                }
                None if matched.is_empty() => self.unwaivable(
                    Law::Zone,
                    rel,
                    "no zone claims this file — every governed file must belong to \
                     exactly one zone (add it to a zone's paths, or to the package's \
                     `exclude` if it is not part of the module)"
                        .to_owned(),
                    format!("unclaimed:{rel}"),
                ),
                None => self.unwaivable(
                    Law::Zone,
                    rel,
                    format!(
                        "claimed by {} zones ({}) — zone paths must partition the module",
                        matched.len(),
                        matched.join(", ")
                    ),
                    format!("ambiguous:{rel}"),
                ),
            }
        }

        self.idle_zones(&claimed);

        let names: HashMap<usize, &str> =
            ordinance.zones.iter().map(|z| (z.rank, z.name.as_str())).collect();
        for edge in &survey.edges {
            if ordinance.is_facade(&edge.dst) && !ordinance.is_facade(&edge.src) {
                self.record(
                    Law::Zone,
                    &edge.src,
                    edge.line,
                    format!(
                        "imports the module facade `{}` — the facade re-exports the whole \
                         module, so importing it from inside closes a cycle over \
                         everything; import the specific file instead",
                        edge.dst
                    ),
                    edge.key(),
                );
                continue;
            }
            if ordinance.is_facade(&edge.src) {
                continue;
            }
            let (Some(&src), Some(&dst)) =
                (ranks.get(edge.src.as_str()), ranks.get(edge.dst.as_str()))
            else {
                continue;
            };
            if dst <= src {
                continue;
            }
            self.record(
                Law::Zone,
                &edge.src,
                edge.line,
                format!(
                    "zone `{}` imports up into `{}` (`{}`) — imports may only point down \
                     the stack",
                    names[&src], names[&dst], edge.dst
                ),
                edge.key(),
            );
        }
    }

    /// A zone that claims nothing is permission outliving the code it was written for.
    ///
    /// Unless what it reached for was the facade, which stands above every zone. Naming
    /// the facade in the stack is the first thing everybody tries, so it is worth saying
    /// why the line did nothing instead of reporting an empty glob and letting the reader
    /// go looking for a file that is right there.
    fn idle_zones(&mut self, claimed: &HashMap<&str, usize>) {
        let (survey, ordinance) = (self.survey, self.ordinance);
        let file = contract_name(self);
        let idle: Vec<String> = ordinance
            .zones
            .iter()
            .filter(|zone| !claimed.contains_key(zone.name.as_str()))
            .map(|zone| {
                let facade = survey
                    .files
                    .iter()
                    .find(|rel| ordinance.is_facade(rel) && zone.paths.matches(rel));
                match facade {
                    Some(rel) => format!(
                        "zone {}  ({file}: matches only the facade `{rel}`, which stands above \
                         every zone — delete the line)",
                        zone.name
                    ),
                    None => {
                        format!(
                            "zone {}  ({file}: matches no file under {})",
                            zone.name, zone.paths
                        )
                    }
                }
            })
            .collect();
        self.stale.extend(idle);
    }

    /// **seal** — a sealed directory is entered through its entry file, or not at all.
    pub(super) fn seals(&mut self) {
        let (survey, ordinance) = (self.survey, self.ordinance);
        for edge in &survey.edges {
            let Some(seal) = innermost(&ordinance.seals, &edge.dst) else { continue };
            if edge.dst == seal.entry {
                continue;
            }
            // A seal fronted from beside (`sqrt.zig` next to `sqrt/`) sits outside its
            // own directory, so it needs naming as well as containment.
            if inside(&edge.src, &seal.path) || edge.src == seal.entry {
                continue;
            }
            if ordinance.is_facade(&edge.src) || seal.open.matches(&edge.src) {
                continue;
            }
            self.record(
                Law::Seal,
                &edge.src,
                edge.line,
                format!(
                    "reaches past the seal on `{}/` into `{}` — enter through `{}`",
                    seal.path, edge.dst, seal.entry
                ),
                edge.key(),
            );
        }
    }

    /// **keep** — a kept region admits only the importers it names.
    pub(super) fn keeps(&mut self) {
        let (survey, ordinance) = (self.survey, self.ordinance);
        let file = contract_name(self);
        for keep in &ordinance.keeps {
            let held: HashSet<&str> = survey
                .files
                .iter()
                .map(String::as_str)
                .filter(|f| keep.subject.matches(f))
                .collect();
            if held.is_empty() {
                self.stale.push(format!("keep {}  ({file}: matches no file)", keep.subject));
                continue;
            }
            let guests = if keep.importers.is_empty() {
                "nobody".to_owned()
            } else {
                keep.importers.raw().map(|i| format!("`{i}`")).collect::<Vec<_>>().join(", ")
            };
            for edge in &survey.edges {
                if !held.contains(edge.dst.as_str()) || held.contains(edge.src.as_str()) {
                    continue;
                }
                if ordinance.is_facade(&edge.src) || one_module(&edge.src, &edge.dst) {
                    continue;
                }
                if keep.importers.matches(&edge.src) {
                    continue;
                }
                self.record(
                    Law::Keep,
                    &edge.src,
                    edge.line,
                    format!(
                        "reaches into `{}` (`{}`), which is kept to {guests} — this caller is \
                         not on the guest list; go through a module both sides already stand on",
                        keep.subject, edge.dst
                    ),
                    edge.key(),
                );
            }
        }
    }

    /// **cycle** — no import cycle may cross a directory boundary.
    pub(super) fn cycles(&mut self) {
        for component in cycle::tangles(self.survey, &self.ordinance.facade) {
            let mut dirs: Vec<&str> = component.iter().map(|m| dir_of(m)).collect();
            dirs.sort_unstable();
            dirs.dedup();
            let plural = if dirs.len() == 1 { "y" } else { "ies" };
            self.record(
                Law::Cycle,
                &component[0],
                1,
                format!(
                    "import cycle across {} director{plural} ({}) over {} files: {} -> … \
                     — analysis is lazy, so this compiles; it still makes the directories \
                     one indivisible unit",
                    dirs.len(),
                    dirs.join(", "),
                    component.len(),
                    component.join(" -> ")
                ),
                cycle_subject(&component),
            );
        }
    }

    /// **reach** — an import may not climb more directories than the ceiling allows.
    pub(super) fn reach(&mut self) {
        let (survey, ordinance) = (self.survey, self.ordinance);
        let Some(ceiling) = ordinance.max_hops else { return };
        for edge in &survey.edges {
            if edge.hops <= ceiling {
                continue;
            }
            self.record(
                Law::Reach,
                &edge.src,
                edge.line,
                format!(
                    "the import `{}` climbs {} directories (ceiling {ceiling}) — the \
                     file's physical home disagrees with what it depends on",
                    edge.spec, edge.hops
                ),
                edge.key(),
            );
        }
    }

    /// **use** — an outside module is imported only where it was granted.
    ///
    /// The other six laws partition what this package owns; this one is the only
    /// statement it makes about what it does not. A file naming a module nobody
    /// granted its zone has added a dependency to the package by writing one line,
    /// which is exactly the decision a contract exists to make visible.
    ///
    /// One refusal per **scope and module**, not per import site. An ungranted module is
    /// a single missing decision however many files took it — sixty identical lines
    /// saying so would be a report nobody reads, and would price one omission as sixty
    /// violations in the burndown. The count rides along, and the first site is the
    /// location, so an editor still lands somewhere real.
    pub(super) fn uses(&mut self) {
        let (survey, ordinance) = (self.survey, self.ordinance);
        let mut spent: HashSet<usize> = HashSet::new();
        let mut refusals: Vec<Refusal<'_>> = Vec::new();
        let mut seen: HashMap<(&str, &str), usize> = HashMap::new();

        for outside in &survey.outside {
            if ordinance.dialect.ambient().contains(&outside.spec.as_str()) {
                continue;
            }
            let granted = ordinance
                .uses
                .iter()
                .position(|u| u.module == outside.spec && u.covers(&outside.src));
            if let Some(at) = granted {
                spent.insert(at);
                continue;
            }
            let zone = ordinance.zone_of(&outside.src).map(|z| z.name.as_str());
            let key = (outside.spec.as_str(), zone.unwrap_or_default());
            if let Some(&at) = seen.get(&key) {
                refusals[at].count += 1;
            } else {
                seen.insert(key, refusals.len());
                refusals.push(Refusal {
                    module: &outside.spec,
                    zone,
                    file: &outside.src,
                    line: outside.line,
                    count: 1,
                });
            }
        }

        for refusal in refusals {
            let (message, subject) = refusal.told(ordinance);
            self.record(Law::Use, refusal.file, refusal.line, message, subject);
        }

        // A grant nobody exercised is permission outliving the dependency it was
        // written for — the same debt a spent-out variance is, and deleted the same way.
        // Counted per line, not per module: two scoped grants for one module are two
        // separate claims, and one of them going quiet is exactly what should surface.
        let file = contract_name(self);
        let idle: Vec<String> = ordinance
            .uses
            .iter()
            .enumerate()
            .filter(|(at, _)| !spent.contains(at))
            .map(|(_, u)| format!("use {}  ({file}: nothing imports it)", written(u)))
            .collect();
        self.stale.extend(idle);
    }

    /// **escape** — an import may not climb out of the module root.
    pub(super) fn escapes(&mut self) {
        let survey = self.survey;
        let remedy = survey.dialect.escape_remedy();
        for escape in &survey.escapes {
            self.record(
                Law::Escape,
                &escape.src,
                escape.line,
                format!("the import `{}` climbs out of the module root — {remedy}", escape.spec),
                escape.key(),
            );
        }
    }
}

/// One scope reaching for one module it was not granted, however many files did it.
struct Refusal<'a> {
    module: &'a str,
    /// The zone doing the reaching, or `None` for the facade, which has no zone.
    zone: Option<&'a str>,
    /// The first site, so the finding still points at code.
    file: &'a str,
    line: usize,
    count: usize,
}

impl Refusal<'_> {
    /// The finding's message, and the subject a variance would have to name.
    ///
    /// The subject reads as an edge from a scope to a module (`exec -> pcre2`) because
    /// that is what a `variance use` ratifies: a dependency somebody means to retire,
    /// as against a `use` grant, which is one they mean to keep. Two spellings, and the
    /// difference between them is whether the line carries a reason.
    fn told(&self, ordinance: &crate::ordinance::Ordinance) -> (String, String) {
        let here = self.zone.map_or_else(|| "the facade".to_owned(), |z| format!("zone `{z}`"));
        let spread = match self.count {
            1 => String::new(),
            n => format!(" from {n} files"),
        };
        let elsewhere: Vec<String> = ordinance
            .grants_of(self.module)
            .flat_map(|u| u.written.iter().map(|scope| format!("`{scope}`")))
            .collect();
        let message = if elsewhere.is_empty() {
            format!(
                "{here} imports the outside module `{}`{spread}, and nothing in this package is \
                 granted it — a dependency leaving the package is a decision, so it is written \
                 down or it does not happen",
                self.module
            )
        } else {
            format!(
                "{here} imports the outside module `{}`{spread}, granted to {} but not here — \
                 the grant is the list of who carries this dependency, and this scope is not on it",
                self.module,
                elsewhere.join(", ")
            )
        };
        (message, format!("{} -> {}", self.zone.unwrap_or("facade"), self.module))
    }
}

/// A grant spelled the way its author wrote it, so a stale line can be found by eye.
fn written(grant: &crate::ordinance::Use) -> String {
    if grant.written.is_empty() {
        grant.module.clone()
    } else {
        format!("{} by {}", grant.module, grant.written.join(" "))
    }
}

fn contract_name(bench: &Bench<'_>) -> String {
    bench.ordinance.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Is module-relative file or directory `child` at or under `directory`?
pub(crate) fn inside(child: &str, directory: &str) -> bool {
    child == directory
        || (child.len() > directory.len()
            && child.starts_with(directory)
            && child.as_bytes()[directory.len()] == b'/')
}

/// The innermost seal whose directory contains `dst`, if any.
fn innermost<'a>(seals: &'a [Seal], dst: &str) -> Option<&'a Seal> {
    seals.iter().filter(|s| inside(dst, &s.path)).max_by_key(|s| s.path.len())
}

/// The directory holding `path`, or `.` for a file at the module root.
#[must_use]
pub fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(".", |(dir, _)| dir)
}

/// `a/b.zig` → `a/b`. Only the last extension, and only after the last separator.
pub(crate) fn stem(path: &str) -> &str {
    let start = path.rfind('/').map_or(0, |i| i + 1);
    match path[start..].rfind('.') {
        Some(dot) if dot > 0 => &path[..start + dot],
        _ => path,
    }
}

/// Do these two files belong to a single directory-level unit?
///
/// Siblings obviously do. So does a facade `x/foo.zig` and anything inside the
/// folder `x/foo/` it fronts: that pair is one module split over a file and a
/// directory, and the parent/child imports are its internal wiring. The other
/// spelling of the same split puts the facade *inside* as `x/foo/foo.zig`, where
/// these edges are plainly intra-directory — both spellings must earn the same
/// verdict, or this law grades a cosmetic placement choice.
pub(crate) fn one_module(src: &str, dst: &str) -> bool {
    let (src_dir, dst_dir) = (dir_of(src), dir_of(dst));
    src_dir == dst_dir || stem(src) == dst_dir || stem(dst) == src_dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containment_does_not_match_a_prefix_of_a_sibling() {
        assert!(inside("assay/span.zig", "assay"));
        assert!(inside("assay", "assay"));
        assert!(!inside("assayed/span.zig", "assay"));
        assert!(!inside("assay.zig", "assay"));
    }

    #[test]
    fn both_spellings_of_a_fronted_directory_are_one_module() {
        assert!(one_module("a/b.zig", "a/c.zig"), "siblings");
        assert!(one_module("a/foo.zig", "a/foo/inner.zig"), "fronted from beside");
        assert!(one_module("a/foo/inner.zig", "a/foo.zig"), "and back");
        assert!(one_module("a/foo/foo.zig", "a/foo/inner.zig"), "fronted from inside");
        assert!(!one_module("a/b.zig", "x/c.zig"));
        assert!(!one_module("a/foo.zig", "a/bar/inner.zig"));
    }

    #[test]
    fn a_root_file_lives_in_the_root_directory() {
        assert_eq!(dir_of("root.zig"), ".");
        assert_eq!(dir_of("a/b/c.zig"), "a/b");
        assert_eq!(stem("a/b.zig"), "a/b");
        assert_eq!(stem("a/b.tar.gz"), "a/b.tar");
        assert_eq!(stem("noext"), "noext");
        assert_eq!(stem(".hidden"), ".hidden", "a leading dot is not an extension");
    }
}
