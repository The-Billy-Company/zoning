//! The report a person reads.
//!
//! Verdict first, then every violation as a `file:line:col:` line an editor can
//! jump to, then — once per law that failed — the one sentence saying what to do
//! about it. The remedy is the part that matters: a gate that only reports gets
//! disabled, and a gate that explains gets used.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use super::Ink;
use crate::judge::Verdict;
use crate::ordinance::{Law, Ordinance};

/// The verdict for one package, with the census when `census` is set.
#[must_use]
pub fn verdict(found: &Verdict, ink: Ink, census: bool) -> String {
    let Ink { red, yellow, green, dim, reset } = ink;
    let mark = if found.ok() { format!("{green}✓{reset}") } else { format!("{red}✗{reset}") };
    let stats = &found.census;
    let mut out = format!(
        "{mark} zoning [{}]: {} files, {} imports, {} violation(s), {} allowed\n",
        found.package,
        stats.files,
        stats.edges,
        found.findings.len(),
        found.ratified.len()
    );

    let mut findings: Vec<&crate::judge::Finding> = found.findings.iter().collect();
    findings.sort_by(|a, b| (a.law, &a.path, a.line).cmp(&(b.law, &b.path, b.line)));
    for finding in &findings {
        let _ = writeln!(
            out,
            "{}:{}:1: [{}] {}",
            finding.path, finding.line, finding.law, finding.message
        );
    }

    if !found.stale.is_empty() {
        let _ = writeln!(
            out,
            "\n{red}✗{reset} {} declaration(s) no longer match anything — the debt was \
             paid; delete them in the same change:",
            found.stale.len()
        );
        for line in &found.stale {
            let _ = writeln!(out, "  {line}");
        }
    }

    for law in Law::ALL {
        if findings.iter().any(|f| f.law == law) {
            let _ = write!(out, "\n{yellow}{law}{reset}: {}\n", law.remedy());
        }
    }

    if !census {
        return out;
    }

    let _ = write!(out, "\n{dim}— census ({}) —{reset}\n", found.package);
    let zones = join(stats.zones.iter().map(|(name, n)| format!("{name}:{n}")));
    let hops = join(stats.hops.iter().map(|(hop, n)| format!("{hop}:{n}")));
    let modules = join(stats.modules.iter().map(|(name, how)| format!("{name}:{}", how.as_str())));
    let _ = writeln!(out, "  zones (low to high)  {}", dash(&zones));
    let _ = writeln!(out, "  imports by ../ hops  {}", dash(&hops));
    let _ = writeln!(out, "  outside modules      {}", dash(&modules));
    if stats.unjudged_imports > 0 {
        let _ = writeln!(
            out,
            "  unjudged imports     {} {dim}(targets outside the committed set){reset}",
            stats.unjudged_imports
        );
    }

    if !found.ratified.is_empty() {
        let mut ratified: Vec<&(crate::judge::Finding, String)> = found.ratified.iter().collect();
        ratified.sort_by(|a, b| (a.0.law, &a.0.subject).cmp(&(b.0.law, &b.0.subject)));
        let _ = writeln!(out, "  {} ratified variance(s):", ratified.len());
        for (finding, reason) in ratified {
            let _ =
                writeln!(out, "    [{}] {}{dim} — {reason}{reset}", finding.law, finding.subject);
        }
    }
    if !stats.sealable.is_empty() {
        let _ = writeln!(out, "  {} director(ies) sealable for free today:", stats.sealable.len());
        for directory in &stats.sealable {
            let _ = writeln!(out, "    {directory}/");
        }
    }
    if !stats.seal_debt.is_empty() {
        let _ =
            writeln!(out, "  entry-file bypass counts for the unsealed rest (the burndown queue):");
        for (directory, count) in &stats.seal_debt {
            let _ = writeln!(out, "    {count:4}  {directory}/");
        }
    }
    out
}

/// The declarations that would make today's violations legal.
///
/// Two kinds, because the two have different remedies. A `use` violation is answered
/// by the grant it is missing, so that is what gets drafted — routing a dependency
/// through the exception mechanism would file the architecture under "exceptions".
/// Everything else is answered by a `variance`, drafted with an empty reason so the
/// paste does not parse until a person has written one: an exception's whole value is
/// its reason, and a machine cannot supply one.
#[must_use]
pub fn suggest(found: &Verdict, ordinance: &Ordinance) -> String {
    if found.findings.is_empty() {
        return "// nothing to declare — no violations\n".to_owned();
    }
    let mut findings: Vec<&crate::judge::Finding> = found.findings.iter().collect();
    findings.sort_by(|a, b| (a.law, &a.subject).cmp(&(b.law, &b.subject)));

    let mut out = String::new();
    let wanted = grants(found, ordinance);
    if !wanted.is_empty() {
        out.push_str("// Draft grants — the dependency each caller is missing.\n");
        for line in wanted {
            let _ = writeln!(out, "{line}");
        }
    }
    if findings.iter().all(|f| f.law == Law::Use) {
        return out;
    }

    out.push_str("\n// Draft variances — paste, then write every reason.\n");
    for finding in findings {
        if finding.law == Law::Use {
            continue;
        }
        if finding.law == Law::Cycle {
            let mut members = String::new();
            for member in finding.subject.split(" + ") {
                let _ = write!(members, "\n    {member}");
            }
            let _ = write!(
                out,
                "\nvariance cycle {{{members}\n}} because \"\"  // WHY, and what retires it?\n"
            );
        } else if let Some((src, dst)) = finding.subject.split_once(" -> ") {
            let _ = write!(
                out,
                "\nvariance {} {src} -> {dst}\n    because \"\"  // WHY is this acceptable, \
                 and what would retire it?\n",
                finding.law
            );
        }
    }
    out
}

/// One `use` line per ungranted module, in the shortest spelling that is still true.
///
/// Scoped by zone name when only some zones take the dependency — the architecture
/// already has a word for the region, and a grant spelled as a zone says more than one
/// spelled as a list of paths. Unscoped when the answer is "everywhere", which is both
/// shorter to read and the only spelling that reaches the facade, since the facade
/// stands above the stack and has no zone name to put in a list.
fn grants(found: &Verdict, ordinance: &Ordinance) -> Vec<String> {
    let mut wanted: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for finding in found.findings.iter().filter(|f| f.law == Law::Use) {
        let Some((scope, module)) = finding.subject.split_once(" -> ") else { continue };
        wanted.entry(module).or_default().insert(scope);
    }
    wanted
        .into_iter()
        .map(|(module, scopes)| {
            let everywhere = scopes.contains("facade")
                || ordinance.zones.iter().all(|z| scopes.contains(z.name.as_str()));
            if everywhere {
                format!("use {module}")
            } else {
                format!("use {module} by {}", scopes.into_iter().collect::<Vec<_>>().join(" "))
            }
        })
        .collect()
}

/// Echo the resolved contract — what zoning believes, not what was typed.
#[must_use]
pub fn show(ordinance: &Ordinance, ink: Ink) -> String {
    let Ink { dim, reset, .. } = ink;
    let mut out = format!("package {}  ({})\n", ordinance.package, ordinance.path.display());
    let _ = writeln!(out, "  root      {}", ordinance.module_root.display());
    let _ = writeln!(out, "  language  {}", ordinance.dialect.name());
    let _ = writeln!(out, "  facade    {}", dash(&ordinance.facade.to_string()));
    let exclude = join(ordinance.exclude.iter().map(ToString::to_string));
    let _ = writeln!(out, "  exclude   {}", dash(&exclude));
    let reach = ordinance.max_hops.map_or_else(|| "unbounded".to_owned(), |n| n.to_string());
    let _ = writeln!(out, "  reach     {reach}");
    let _ = writeln!(out, "  cycles    forbidden across directories");
    let _ = writeln!(out, "  zones     (low to high)");
    for zone in &ordinance.zones {
        let _ = writeln!(out, "    {:>2}. {:<12} {}", zone.rank, zone.name, zone.paths);
    }
    for seal in &ordinance.seals {
        let opened =
            if seal.open.is_empty() { String::new() } else { format!("  open to {}", seal.open) };
        let _ = writeln!(out, "  seal      {}/ through {}{opened}", seal.path, seal.entry);
    }
    for keep in &ordinance.keeps {
        let _ = writeln!(out, "  keep      {} to {}", keep.subject, guests(&keep.importers));
    }
    for grant in &ordinance.uses {
        let scope = if grant.written.is_empty() {
            "every zone".to_owned()
        } else {
            grant.written.join(" ")
        };
        let _ = writeln!(out, "  use       {} by {scope}", grant.module);
    }
    let ambient = ordinance.dialect.ambient().join(" ");
    let _ = writeln!(out, "  ambient   {}{dim} (granted by the language){reset}", dash(&ambient));
    for variance in &ordinance.variances {
        let _ = writeln!(
            out,
            "  variance  [{}] {}{dim} — {}{reset}",
            variance.law, variance.subject, variance.reason
        );
    }
    out
}

fn join(items: impl Iterator<Item = String>) -> String {
    items.collect::<Vec<_>>().join(" ")
}

/// A guest list, or the word for a region with no way in from outside.
fn guests(importers: &crate::pattern::Globs) -> String {
    if importers.is_empty() { "nobody".to_owned() } else { importers.to_string() }
}

fn dash(text: &str) -> &str {
    if text.is_empty() { "—" } else { text }
}
