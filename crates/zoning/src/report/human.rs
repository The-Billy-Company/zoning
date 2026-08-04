//! The report a person reads.
//!
//! Verdict first, then every violation as a `file:line:col:` line an editor can
//! jump to, then — once per law that failed — the one sentence saying what to do
//! about it. The remedy is the part that matters: a gate that only reports gets
//! disabled, and a gate that explains gets used.

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
    let named = join(stats.named_modules.iter().cloned());
    let _ = writeln!(out, "  zones (low to high)  {}", dash(&zones));
    let _ = writeln!(out, "  imports by ../ hops  {}", dash(&hops));
    let _ = writeln!(out, "  named modules        {}", dash(&named));
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

/// The `variance` stanzas that would ratify today's violations.
///
/// Suggest-only, never write: an exception's whole value is its reason, and a
/// machine cannot supply one. Paste, then say why.
#[must_use]
pub fn suggest(found: &Verdict) -> String {
    if found.findings.is_empty() {
        return "// no violations to grant a variance for\n".to_owned();
    }
    let mut findings: Vec<&crate::judge::Finding> = found.findings.iter().collect();
    findings.sort_by(|a, b| (a.law, &a.subject).cmp(&(b.law, &b.subject)));

    let mut out =
        "// Draft variances — paste into the .zone file and write every reason.\n".to_owned();
    for finding in findings {
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

/// Echo the resolved contract — what zoning believes, not what was typed.
#[must_use]
pub fn show(ordinance: &Ordinance, ink: Ink) -> String {
    let Ink { dim, reset, .. } = ink;
    let mut out = format!("package {}  ({})\n", ordinance.package, ordinance.path.display());
    let _ = writeln!(out, "  root      {}", ordinance.module_root.display());
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
        let _ = writeln!(out, "  keep      {} to {}", keep.subject, keep.importers);
    }
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

fn dash(text: &str) -> &str {
    if text.is_empty() { "—" } else { text }
}
