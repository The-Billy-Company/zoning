//! Advisory cartography: what the contract *could* govern, and doesn't yet.
//!
//! A gate that only says no is a gate people route around. The census is the other
//! half — the shape of the module as it stands, plus two burndown queues: the
//! directories already behaving like deep modules that nobody has sealed, and the
//! bypass count for the rest, ranked so the worst offender is the obvious next move.
//! None of it can fail a build.

use std::collections::HashMap;

use super::law::{dir_of, inside};
use crate::ordinance::Ordinance;
use crate::survey::Survey;

/// The shape of a module, measured.
pub struct Census {
    /// Files in the judged set.
    pub files: usize,
    /// Resolved intra-module imports.
    pub edges: usize,
    /// Imports pointing outside the judged set — usually a coworker's new file.
    pub unjudged_imports: usize,
    /// External modules imported by name.
    pub named_modules: Vec<String>,
    /// How many imports climb how many directories, ascending.
    pub hops: Vec<(u32, usize)>,
    /// Files per zone, low to high.
    pub zones: Vec<(String, usize)>,
    /// Directories that already have exactly one entry point and no bypasses —
    /// sealable today, for free.
    pub sealable: Vec<String>,
    /// Directories with an entry file that callers reach past, worst first.
    pub seal_debt: Vec<(String, usize)>,
}

/// Measure the module.
#[must_use]
pub(super) fn take(survey: &Survey, ordinance: &Ordinance) -> Census {
    let entries = fronted(survey);
    let sealed: Vec<&str> = ordinance.seals.iter().map(|s| s.path.as_str()).collect();

    let mut bypassed: HashMap<&str, usize> = HashMap::new();
    for edge in &survey.edges {
        let parent = dir_of(&edge.dst);
        let Some(entry) = entries.get(parent) else { continue };
        if edge.dst == *entry || inside(&edge.src, parent) || edge.src == *entry {
            continue;
        }
        if ordinance.is_facade(&edge.src) {
            continue;
        }
        *bypassed.entry(parent).or_default() += 1;
    }

    let mut sealable: Vec<String> = entries
        .keys()
        .filter(|d| !sealed.contains(d) && !bypassed.contains_key(*d))
        .map(|d| (*d).to_owned())
        .collect();
    sealable.sort();

    let mut seal_debt: Vec<(String, usize)> = bypassed
        .iter()
        .filter(|(d, _)| !sealed.contains(*d))
        .map(|(d, n)| ((*d).to_owned(), *n))
        .collect();
    seal_debt.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut hops: HashMap<u32, usize> = HashMap::new();
    for edge in &survey.edges {
        *hops.entry(edge.hops).or_default() += 1;
    }
    let mut hops: Vec<(u32, usize)> = hops.into_iter().collect();
    hops.sort_unstable();

    Census {
        files: survey.files.len(),
        edges: survey.edges.len(),
        unjudged_imports: survey.skipped,
        named_modules: survey.named.clone(),
        hops,
        zones: ordinance
            .zones
            .iter()
            .map(|z| {
                let held = survey
                    .files
                    .iter()
                    .filter(|f| ordinance.zone_of(f).is_some_and(|found| found.rank == z.rank))
                    .count();
                (z.name.clone(), held)
            })
            .collect(),
        sealable,
        seal_debt,
    }
}

/// Directory → the file that already fronts it, in either spelling.
///
/// Inside (`rank/rank.zig`) wins over beside (`sqrt.zig` next to `sqrt/`) when a
/// package somehow has both, because the inner file is the one an outsider would
/// reach for. A directory with neither is not a candidate for a seal.
fn fronted(survey: &Survey) -> HashMap<&str, &str> {
    let extension = survey.dialect.extensions().first().copied().unwrap_or_default();
    let have: Vec<&str> = survey.files.iter().map(String::as_str).collect();
    let mut directories: Vec<&str> = have.iter().map(|f| dir_of(f)).filter(|d| *d != ".").collect();
    directories.sort_unstable();
    directories.dedup();

    let mut out = HashMap::new();
    for directory in directories {
        let name = directory.rsplit_once('/').map_or(directory, |(_, n)| n);
        let candidates =
            [format!("{directory}/{name}.{extension}"), format!("{directory}.{extension}")];
        if let Some(entry) = candidates.iter().find_map(|c| have.iter().find(|f| **f == c)) {
            out.insert(directory, *entry);
        }
    }
    out
}
