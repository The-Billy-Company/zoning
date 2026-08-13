//! Two questions a person actually asks, answered before they write the import.
//!
//! A gate that only speaks when it is angry teaches nothing. `verify` says what broke;
//! this says where a file stands and what it may reach — the model itself, for the file
//! in front of you, rather than a document you have to hold in your head and translate.
//!
//! Given one file: its zone, its rank, everything it may import, everything granted to
//! it, whether it sits behind a seal or inside a keep, and who imports it today. Given
//! two: whether that import is allowed, and the exact clause that decides it — for an
//! import that exists *and* for one you are considering, because the useful moment to
//! ask is before the edit.
//!
//! The hypothetical is judged by the real laws over a survey holding just that edge,
//! not by a second copy of the rules. A `zone explain` that could disagree with
//! `zone verify` would be worse than no answer at all.

use std::fmt::Write as _;

use super::Ink;
use crate::judge::{self, dir_of};
use crate::ordinance::{Law, Ordinance};
use crate::survey::{Edge, Survey};

/// Where a field's value starts — `"  imports    "`, so a wrap lines up under it.
const FIELD: usize = 13;
/// Where a wrapped line breaks. Narrow enough to survive a split editor pane.
const WIDTH: usize = 78;

/// A rendered answer, and whether the answer was yes.
///
/// The text is for the person and the flag is for the shell, because "may I write this
/// import" is a question worth asking from a script: `zone explain a.zig b.zig &&
/// $EDITOR a.zig` only reads the way it looks if the verdict reaches the exit code. A
/// diagnostic verb that always exits 0 forces the caller to grep its prose, which makes
/// the prose an interface nobody can change.
pub struct Answer {
    /// What to print.
    pub text: String,
    /// True when nothing is in violation: the file stands clean, or the edge is allowed.
    pub clean: bool,
}

/// Where one file stands: its zone, its reach, and who reaches it.
#[must_use]
pub fn file(rel: &str, ordinance: &Ordinance, survey: &Survey, ink: &Ink) -> Answer {
    let Ink { dim, reset, red, green, .. } = *ink;
    let mut out = format!("{rel}\n");
    let _ = writeln!(
        out,
        "  package    {} {dim}({}, {}){reset}",
        ordinance.package,
        tail(&ordinance.path),
        ordinance.dialect.name()
    );

    if ordinance.is_facade(rel) {
        let _ = writeln!(
            out,
            "  facade     {green}yes{reset} {dim}— the module's public face: it may reach \
             anything, and every law steps aside for it{reset}"
        );
    }
    tenancy(&mut out, rel, ordinance, ink);

    let granted: Vec<&str> =
        ordinance.uses.iter().filter(|u| u.covers(rel)).map(|u| u.module.as_str()).collect();
    // The ambient set is a property of the language, not of this file — hundreds of
    // names in Python — and printing it buried every line under it. The count says the
    // same thing: nothing here is a decision anybody made about this package.
    field(
        &mut out,
        "may use",
        "",
        &granted,
        &format!(
            "plus the {} modules ambient in {}",
            ordinance.dialect.ambient().len(),
            ordinance.dialect.name()
        ),
        ink,
    );

    for seal in ordinance.seals.iter().filter(|s| rel.starts_with(&format!("{}/", s.path))) {
        let role = if seal.entry == rel { "this file is the door" } else { "reach it via" };
        let _ = writeln!(out, "  sealed     {} {dim}({role}: {}){reset}", seal.path, seal.entry);
    }
    for keep in ordinance.keeps.iter().filter(|k| k.subject.matches(rel)) {
        let guests = if keep.importers.is_empty() {
            "nobody".to_owned()
        } else {
            keep.importers.to_string()
        };
        let _ = writeln!(out, "  kept       {} {dim}(open to {guests}){reset}", keep.subject);
    }

    let out_edges: Vec<&Edge> = survey.edges.iter().filter(|e| e.src == rel).collect();
    let in_edges: Vec<&Edge> = survey.edges.iter().filter(|e| e.dst == rel).collect();
    let outs = out_edges.len().to_string();
    let ins = in_edges.len().to_string();
    field(&mut out, "imports", &outs, &ends(&out_edges, |e| &e.dst), "", ink);
    field(&mut out, "imported", &ins, &ends(&in_edges, |e| &e.src), "", ink);

    let verdict = judge::judge(survey, ordinance);
    let here = survey.rel(rel);
    let mine: Vec<&judge::Finding> = verdict.findings.iter().filter(|f| f.path == here).collect();
    let clean = mine.is_empty();
    if clean {
        let _ = writeln!(out, "  standing   {green}clean{reset}");
    } else {
        let _ = writeln!(out, "  standing   {red}{} finding(s){reset}", mine.len());
        for finding in mine {
            let _ = writeln!(out, "    {} {dim}·{reset} {}", finding.law, finding.message);
        }
    }
    Answer { text: out, clean }
}

/// Whether one file may import another, and the clause that decides it.
#[must_use]
pub fn edge(from: &str, to: &str, ordinance: &Ordinance, survey: &Survey, ink: &Ink) -> Answer {
    let Ink { dim, reset, red, green, yellow, .. } = *ink;
    let real: Vec<&Edge> = survey.edges.iter().filter(|e| e.src == from && e.dst == to).collect();
    let mut out = format!("{from} -> {to}\n");
    let _ = writeln!(
        out,
        "  today      {}",
        match real.first() {
            Some(edge) => format!(
                "imported at {from}:{} as `{}`{dim} ({} hop{}){reset}",
                edge.line,
                edge.spec,
                edge.hops,
                if edge.hops == 1 { "" } else { "s" }
            ),
            None => format!("{dim}no such import — judging it as a hypothetical{reset}"),
        }
    );

    let probe = real.first().map_or_else(|| spelled(from, to), |edge| (*edge).clone());
    let verdict = judge::judge(&survey.hypothetically(probe), ordinance);
    let against: Vec<&judge::Finding> =
        verdict.findings.iter().filter(|f| f.subject == format!("{from} -> {to}")).collect();

    if against.is_empty() {
        let _ = writeln!(out, "  allowed    {green}yes{reset}");
        if let (Some(src), Some(dst)) = (ordinance.zone_of(from), ordinance.zone_of(to)) {
            let _ = writeln!(
                out,
                "  because    zone `{}` (rank {}) may import zone `{}` (rank {})",
                src.name,
                src.rank + 1,
                dst.name,
                dst.rank + 1
            );
        }
        if let Some(granted) = ordinance.variance(Law::Zone, &format!("{from} -> {to}")) {
            let _ =
                writeln!(out, "  variance   {} {dim}({}){reset}", granted.reason, granted.source);
        }
        return Answer { text: out, clean: true };
    }

    let _ = writeln!(out, "  allowed    {red}no{reset}");
    for finding in &against {
        let _ = writeln!(out, "  law        {red}{}{reset} — {}", finding.law, finding.message);
        let _ = writeln!(out, "  remedy     {yellow}{}{reset}", finding.law.remedy());
    }
    // Seal is the one law with a mechanical answer: the target has a door, and naming it
    // instead is the whole fix. Worth printing only when that is news — when the caller
    // asked about the door itself, repeating it back says nothing.
    if against.iter().any(|f| f.law == Law::Seal)
        && let Some(seal) = ordinance.seals.iter().find(|s| to.starts_with(&format!("{}/", s.path)))
        && seal.entry != to
    {
        let _ = writeln!(out, "  instead    import {} {dim}(the seal's door){reset}", seal.entry);
    }
    Answer { text: out, clean: false }
}

/// The shortest module-relative spelling of an import from `from` to `to`.
///
/// A hypothetical edge still needs a hop count, since the reach ceiling is about how
/// far a spelling climbs. The shortest spelling is the fairest one to judge: if even
/// that exceeds the ceiling, no way of writing the import would pass.
pub(super) fn spelled(from: &str, to: &str) -> Edge {
    let (here, there) = (dir_of(from), dir_of(to));
    let shared = here
        .split('/')
        .zip(there.split('/'))
        .take_while(|(a, b)| a == b)
        .filter(|(a, _)| !a.is_empty())
        .count();
    let depth = if here == "." { 0 } else { here.split('/').count() };
    let hops = u32::try_from(depth.saturating_sub(shared)).unwrap_or(u32::MAX);
    let spec = format!("{}{to}", "../".repeat(hops as usize));
    Edge { src: from.to_owned(), dst: to.to_owned(), line: 0, col: 1, width: 1, hops, spec }
}

/// Which zone holds this file, on whose authority, and how far that lets it reach.
fn tenancy(out: &mut String, rel: &str, ordinance: &Ordinance, ink: &Ink) {
    let Ink { dim, reset, red, .. } = *ink;
    let Some(zone) = ordinance.zone_of(rel) else {
        // A facade stands outside the zones on purpose; anything else is unclaimed.
        if !ordinance.is_facade(rel) {
            let _ = writeln!(
                out,
                "  zone       {red}none{reset} {dim}— no zone claims this file, which is \
                 itself a violation{reset}"
            );
        }
        return;
    };
    let _ = writeln!(
        out,
        "  zone       {} {dim}(rank {} of {}){reset}",
        zone.name,
        zone.rank + 1,
        ordinance.zones.len()
    );
    // Which line of the contract reached this file, not all of them: a drafted
    // contract's tangle zone holds hundreds of globs, and printing them buries the
    // answer under the population it was drawn from.
    if let Some(glob) = zone.paths.claiming(rel) {
        let total = zone.paths.raw().len();
        let of = if total == 1 {
            "the zone's only path".to_owned()
        } else {
            format!("1 of the zone's {total} paths")
        };
        let _ = writeln!(out, "  claimed    {glob} {dim}({of}){reset}");
    }
    let below: Vec<&str> =
        ordinance.zones.iter().filter(|z| z.rank <= zone.rank).map(|z| z.name.as_str()).collect();
    field(out, "may import", "", &below, "its own zone and everything under it", ink);
}

/// Every name on the other end of a set of edges, deduped and in order.
///
/// This used to print three and `+6`. Three of nine is not an answer to "who imports
/// this file" — it is the shape of an answer — and the missing six were exactly the
/// ones somebody was about to grep for. A file with many callers prints many lines,
/// which is the honest picture of a file with many callers.
fn ends<'a>(edges: &[&'a Edge], pick: impl Fn(&'a Edge) -> &'a String) -> Vec<&'a str> {
    let mut names: Vec<&str> = edges.iter().map(|e| pick(e).as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// One `name  value (gloss)` row, wrapped under the field when the value overruns.
///
/// Every list in this report is unbounded — a zone may claim two hundred globs, a file
/// may import a hundred others — and a list that runs off the right edge is a list
/// nobody can read the end of. The gloss trails the value while the row fits and moves
/// up beside the field name when it doesn't, because a parenthetical stranded after
/// eleven wrapped rows reads as a note about the last name rather than about the field.
fn field(out: &mut String, name: &str, lead: &str, names: &[&str], gloss: &str, ink: &Ink) {
    let Ink { dim, reset, .. } = *ink;
    let note = if gloss.is_empty() { String::new() } else { format!(" {dim}({gloss}){reset}") };
    let head = format!("  {name:<10}{}{lead}", if lead.is_empty() { "" } else { " " });
    let rows = wrapped(names);
    match rows.as_slice() {
        // A count already says "none"; a dash beside it would say it twice.
        [] if !lead.is_empty() => {
            let _ = writeln!(out, "{head}{note}");
        }
        [] => {
            let _ = writeln!(out, "{head} {dim}—{reset}{note}");
        }
        [only] if FIELD + only.chars().count() + gloss.chars().count() + 3 <= WIDTH => {
            let _ = writeln!(out, "{head} {only}{note}");
        }
        rows => {
            let _ = writeln!(out, "{head}{note}");
            for row in rows {
                let _ = writeln!(out, "{}{row}", " ".repeat(FIELD));
            }
        }
    }
}

/// Names joined into lines narrow enough to sit under a field without wrapping again.
fn wrapped(names: &[&str]) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for name in names {
        match rows.last_mut() {
            Some(row) if FIELD + row.chars().count() + 3 + name.chars().count() <= WIDTH => {
                row.push_str(" · ");
                row.push_str(name);
            }
            _ => rows.push((*name).to_owned()),
        }
    }
    rows
}

/// The last two path components — enough to name a contract beside its package name.
fn tail(path: &std::path::Path) -> String {
    let parts: Vec<String> = path
        .components()
        .rev()
        .take(2)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.into_iter().rev().collect::<Vec<_>>().join("/")
}
