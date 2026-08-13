//! The machine forms: a verdict, a graph, an explanation.
//!
//! Hand-written rather than borrowed: the shapes are strings, integers, and flat
//! lists, and a serialization framework in a tool with no other dependencies would be
//! the largest thing in the binary. Emit only, never parse — nothing here ever reads
//! JSON back, which is what keeps it this small.
//!
//! A gate that can only be read by eye is a gate nothing can be built on. `verify` and
//! `status` answer with [`records`]; `map` answers with [`graph`], which is the import
//! graph itself — every zone, every edge, every departure, each already carrying the
//! zone it was judged under, so a consumer never has to re-run the glob matching to
//! find out where a file lives. `explain` answers with [`node`] or [`edge`], the same
//! two questions the human form answers, minus the prose.

use std::collections::HashMap;
use std::fmt::Write as _;

use super::explain::{Answer, spelled};
use crate::judge::{self, Dormant, Verdict};
use crate::ordinance::{Law, Ordinance};
use crate::survey::Survey;

/// Every finding and every stale declaration across the judged packages, as a JSON
/// array. Stale declarations carry the law `"stale"`, which is not one of the six —
/// they are the contract failing, not the code.
///
/// A dormant shared grant is filed the same way, under the workspace file that wrote it
/// rather than a package: no package owns it, and every one of them was already asked.
#[must_use]
pub fn records(verdicts: &[Verdict], dormant: &[Dormant]) -> String {
    let mut rows: Vec<String> = Vec::new();
    for shared in dormant {
        let workspace = shared.workspace.display().to_string();
        for grant in &shared.grants {
            rows.push(object(&[
                ("package", Value::Text(&workspace)),
                ("law", Value::Text("stale")),
                ("subject", Value::Text(grant)),
                ("message", Value::Text("no member of the workspace imports it")),
            ]));
        }
    }
    for verdict in verdicts {
        for finding in &verdict.findings {
            rows.push(object(&[
                ("package", Value::Text(&verdict.package)),
                ("law", Value::Text(finding.law.as_str())),
                ("file", Value::Text(&finding.path)),
                ("line", Value::Number(finding.line)),
                ("subject", Value::Text(&finding.subject)),
                ("message", Value::Text(&finding.message)),
            ]));
        }
        for stale in &verdict.stale {
            rows.push(object(&[
                ("package", Value::Text(&verdict.package)),
                ("law", Value::Text("stale")),
                ("subject", Value::Text(stale)),
                ("message", Value::Text("declaration matches nothing")),
            ]));
        }
    }
    if rows.is_empty() {
        return "[]\n".to_owned();
    }
    format!("[\n{}\n]\n", rows.join(",\n"))
}

/// The whole import graph of one package, as the contract judged it.
///
/// The answer `map` draws as a picture. Every zone with the files it holds and the
/// zones it reaches, then every edge and every departure with the zone each end sits
/// in — resolved here rather than left to the reader, because recovering it means
/// re-running the glob matching this pass already did.
#[must_use]
pub fn graph(ordinance: &Ordinance, survey: &Survey) -> String {
    // Every file's zone, resolved once. Each row below wants the zone of one end or the
    // other, and `zone_of` is a walk over every zone: asking per row would re-run the
    // whole glob matching some thousands of times to print a graph we already have.
    let tenancy: HashMap<&str, &str> = survey
        .files
        .iter()
        .filter_map(|f| ordinance.zone_of(f).map(|z| (f.as_str(), z.name.as_str())))
        .collect();

    object(&[
        ("package", Value::Text(&ordinance.package)),
        ("language", Value::Text(ordinance.dialect.name())),
        ("contract", Value::Text(&ordinance.path.display().to_string())),
        ("root", Value::Text(&ordinance.module_root.display().to_string())),
        ("files", Value::Number(survey.files.len())),
        ("imports", Value::Number(survey.edges.len())),
        ("zones", Value::Rows(zone_rows(ordinance, survey, &tenancy))),
        ("seals", Value::Rows(seal_rows(ordinance))),
        ("keeps", Value::Rows(keep_rows(ordinance))),
        ("uses", Value::Rows(use_rows(ordinance))),
        ("edges", Value::Rows(edge_rows(survey, &tenancy))),
        ("outside", Value::Rows(outside_rows(ordinance, survey, &tenancy))),
    ])
}

/// The zone of one file, or null where no zone claims it — itself a finding.
fn sited<'a>(tenancy: &HashMap<&str, &'a str>, rel: &str) -> Value<'a> {
    tenancy.get(rel).map_or(Value::Null, |name| Value::Text(name))
}

/// Each zone with what it holds and, from the edges, which zones it actually reaches.
fn zone_rows(ordinance: &Ordinance, survey: &Survey, tenancy: &HashMap<&str, &str>) -> Vec<String> {
    let mut held: HashMap<&str, usize> = HashMap::new();
    for name in tenancy.values() {
        *held.entry(name).or_default() += 1;
    }
    let mut reaches: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &survey.edges {
        if let (Some(&src), Some(&dst)) =
            (tenancy.get(edge.src.as_str()), tenancy.get(edge.dst.as_str()))
            && src != dst
            && !reaches.entry(src).or_default().contains(&dst)
        {
            reaches.entry(src).or_default().push(dst);
        }
    }
    ordinance
        .zones
        .iter()
        .map(|zone| {
            let name = zone.name.as_str();
            let mut into = reaches.get(name).cloned().unwrap_or_default();
            into.sort_unstable();
            inline(&[
                ("name", Value::Text(name)),
                ("rank", Value::Number(zone.rank + 1)),
                ("paths", Value::List(zone.paths.raw().map(str::to_owned).collect())),
                ("files", Value::Number(held.get(name).copied().unwrap_or_default())),
                ("reaches", Value::List(into.into_iter().map(str::to_owned).collect())),
            ])
        })
        .collect()
}

/// Every import inside the package, each end carrying the zone it was judged under.
fn edge_rows(survey: &Survey, tenancy: &HashMap<&str, &str>) -> Vec<String> {
    survey
        .edges
        .iter()
        .map(|edge| {
            inline(&[
                ("src", Value::Text(&edge.src)),
                ("dst", Value::Text(&edge.dst)),
                ("line", Value::Number(edge.line)),
                ("col", Value::Number(edge.col)),
                ("spec", Value::Text(&edge.spec)),
                ("hops", Value::Number(edge.hops as usize)),
                ("from", sited(tenancy, &edge.src)),
                ("to", sited(tenancy, &edge.dst)),
            ])
        })
        .collect()
}

/// Every import that leaves the package, with why it is allowed to: the language put
/// the module there, or a grant did.
fn outside_rows(
    ordinance: &Ordinance,
    survey: &Survey,
    tenancy: &HashMap<&str, &str>,
) -> Vec<String> {
    let ambient = survey.dialect.ambient();
    survey
        .outside
        .iter()
        .map(|away| {
            inline(&[
                ("src", Value::Text(&away.src)),
                ("module", Value::Text(&away.spec)),
                ("line", Value::Number(away.line)),
                ("from", sited(tenancy, &away.src)),
                ("ambient", Value::Bool(ambient.contains(&away.spec.as_str()))),
                ("granted", Value::Bool(ordinance.may_use(&away.src, &away.spec))),
            ])
        })
        .collect()
}

/// The sealed directories: the door, and whoever may go around it.
fn seal_rows(ordinance: &Ordinance) -> Vec<String> {
    ordinance
        .seals
        .iter()
        .map(|seal| {
            inline(&[
                ("path", Value::Text(&seal.path)),
                ("entry", Value::Text(&seal.entry)),
                ("open", Value::List(seal.open.raw().map(str::to_owned).collect())),
            ])
        })
        .collect()
}

/// The kept files and their guest lists.
fn keep_rows(ordinance: &Ordinance) -> Vec<String> {
    ordinance
        .keeps
        .iter()
        .map(|keep| {
            inline(&[
                ("subject", Value::Text(keep.subject.as_str())),
                ("importers", Value::List(keep.importers.raw().map(str::to_owned).collect())),
            ])
        })
        .collect()
}

/// The outside modules this package is allowed to reach, and whether it said so itself.
fn use_rows(ordinance: &Ordinance) -> Vec<String> {
    ordinance
        .uses
        .iter()
        .map(|grant| {
            inline(&[
                ("module", Value::Text(&grant.module)),
                ("by", Value::List(grant.written.clone())),
                ("inherited", Value::Bool(grant.inherited)),
            ])
        })
        .collect()
}

/// Where one file stands, as data: the machine form of `explain FILE`.
#[must_use]
pub fn node(rel: &str, ordinance: &Ordinance, survey: &Survey) -> Answer {
    let zone = ordinance.zone_of(rel);
    let below: Vec<String> = zone.map_or_else(Vec::new, |here| {
        ordinance.zones.iter().filter(|z| z.rank <= here.rank).map(|z| z.name.clone()).collect()
    });
    let out: Vec<String> = survey
        .edges
        .iter()
        .filter(|e| e.src == rel)
        .map(|e| {
            inline(&[
                ("dst", Value::Text(&e.dst)),
                ("line", Value::Number(e.line)),
                ("spec", Value::Text(&e.spec)),
                ("hops", Value::Number(e.hops as usize)),
            ])
        })
        .collect();
    let into: Vec<String> = survey
        .edges
        .iter()
        .filter(|e| e.dst == rel)
        .map(|e| inline(&[("src", Value::Text(&e.src)), ("line", Value::Number(e.line))]))
        .collect();

    let verdict = judge::judge(survey, ordinance);
    let here = survey.rel(rel);
    let mine: Vec<&judge::Finding> = verdict.findings.iter().filter(|f| f.path == here).collect();
    let clean = mine.is_empty();

    let text = object(&[
        ("file", Value::Text(rel)),
        ("package", Value::Text(&ordinance.package)),
        ("language", Value::Text(ordinance.dialect.name())),
        ("facade", Value::Bool(ordinance.is_facade(rel))),
        ("zone", zone.map_or(Value::Null, |z| Value::Text(&z.name))),
        ("rank", zone.map_or(Value::Null, |z| Value::Number(z.rank + 1))),
        ("may_import", Value::List(below)),
        (
            "may_use",
            Value::List(
                ordinance.uses.iter().filter(|u| u.covers(rel)).map(|u| u.module.clone()).collect(),
            ),
        ),
        (
            "sealed_by",
            Value::List(
                ordinance
                    .seals
                    .iter()
                    .filter(|s| rel.starts_with(&format!("{}/", s.path)))
                    .map(|s| s.entry.clone())
                    .collect(),
            ),
        ),
        (
            "kept_by",
            Value::List(
                ordinance
                    .keeps
                    .iter()
                    .filter(|k| k.subject.matches(rel))
                    .map(|k| k.subject.as_str().to_owned())
                    .collect(),
            ),
        ),
        ("imports", Value::Rows(out)),
        ("imported_by", Value::Rows(into)),
        ("findings", Value::Rows(mine.iter().map(|f| finding(f)).collect())),
        ("clean", Value::Bool(clean)),
    ]);
    Answer { text: format!("{text}\n"), clean }
}

/// Whether one file may import another, as data: the machine form of `explain A B`.
#[must_use]
pub fn edge(from: &str, to: &str, ordinance: &Ordinance, survey: &Survey) -> Answer {
    let real = survey.edges.iter().find(|e| e.src == from && e.dst == to);
    let probe = real.map_or_else(|| spelled(from, to), Clone::clone);
    let verdict = judge::judge(&survey.hypothetically(probe.clone()), ordinance);
    let subject = format!("{from} -> {to}");
    let against: Vec<&judge::Finding> =
        verdict.findings.iter().filter(|f| f.subject == subject).collect();
    let clean = against.is_empty();

    let text = object(&[
        ("from", Value::Text(from)),
        ("to", Value::Text(to)),
        ("package", Value::Text(&ordinance.package)),
        ("exists", Value::Bool(real.is_some())),
        ("line", real.map_or(Value::Null, |e| Value::Number(e.line))),
        ("spec", Value::Text(&probe.spec)),
        ("hops", Value::Number(probe.hops as usize)),
        ("from_zone", ordinance.zone_of(from).map_or(Value::Null, |z| Value::Text(&z.name))),
        ("to_zone", ordinance.zone_of(to).map_or(Value::Null, |z| Value::Text(&z.name))),
        ("allowed", Value::Bool(clean)),
        ("findings", Value::Rows(against.iter().map(|f| finding(f)).collect())),
        (
            "variance",
            ordinance
                .variance(Law::Zone, &subject)
                .map_or(Value::Null, |granted| Value::Text(&granted.reason)),
        ),
    ]);
    Answer { text: format!("{text}\n"), clean }
}

fn finding(found: &judge::Finding) -> String {
    inline(&[
        ("law", Value::Text(found.law.as_str())),
        ("file", Value::Text(&found.path)),
        ("line", Value::Number(found.line)),
        ("subject", Value::Text(&found.subject)),
        ("message", Value::Text(&found.message)),
        ("remedy", Value::Text(found.law.remedy())),
    ])
}

enum Value<'a> {
    Text(&'a str),
    Number(usize),
    Bool(bool),
    /// A JSON array of strings.
    List(Vec<String>),
    /// A JSON array whose items are already rendered.
    Rows(Vec<String>),
    /// Absent, and said so — a missing key would make every consumer guess.
    Null,
}

impl Value<'_> {
    /// The value alone, at `pad` spaces of indent for anything that wraps.
    fn render(&self, pad: usize) -> String {
        match self {
            Self::Text(text) => format!("\"{}\"", escape(text)),
            Self::Number(n) => n.to_string(),
            Self::Bool(yes) => yes.to_string(),
            Self::Null => "null".to_owned(),
            Self::List(items) => {
                let quoted: Vec<String> =
                    items.iter().map(|i| format!("\"{}\"", escape(i))).collect();
                format!("[{}]", quoted.join(", "))
            }
            Self::Rows(rows) if rows.is_empty() => "[]".to_owned(),
            Self::Rows(rows) => {
                let inner = " ".repeat(pad + 2);
                format!("[\n{inner}{}\n{}]", rows.join(&format!(",\n{inner}")), " ".repeat(pad))
            }
        }
    }
}

/// A multi-line object, indented one step in from the array that holds it.
fn object(fields: &[(&str, Value<'_>)]) -> String {
    let body: Vec<String> =
        fields.iter().map(|(key, value)| format!("    \"{key}\": {}", value.render(4))).collect();
    format!("  {{\n{}\n  }}", body.join(",\n"))
}

/// A one-line object, for a row inside an array.
fn inline(fields: &[(&str, Value<'_>)]) -> String {
    let body: Vec<String> =
        fields.iter().map(|(key, value)| format!("\"{key}\": {}", value.render(0))).collect();
    format!("{{{}}}", body.join(", "))
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_and_quotes_survive_a_round_trip_by_eye() {
        assert_eq!(escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(escape("line\nbreak"), "line\\nbreak");
        assert_eq!(escape("bell\u{7}"), "bell\\u0007");
        assert_eq!(escape("plain — dash"), "plain — dash", "utf-8 passes through");
    }

    #[test]
    fn no_findings_is_an_empty_array_not_an_empty_string() {
        assert_eq!(records(&[], &[]), "[]\n");
    }
}
