//! The contract as a picture.
//!
//! A zone stack is a tall, ordered thing, and a list of globs does not read like
//! one. The map draws it the way gravity works — highest zone at the top, everything
//! it may depend on below it — so "imports point down the page" stops being a rule
//! you memorise and becomes a thing you can see. Beside each zone: how many files it
//! holds, and how many distinct zones beneath it it actually reaches into. A zone
//! that reaches into every zone below it is a zone that is not a layer.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use super::Ink;
use crate::judge::inside;
use crate::ordinance::Ordinance;
use crate::survey::Survey;

const RULE: usize = 74;
const BAR: usize = 14;

/// Draw the stack.
#[must_use]
pub fn map(ordinance: &Ordinance, survey: &Survey, ink: Ink) -> String {
    let Ink { dim, yellow, reset, .. } = ink;
    let depth = ordinance.zones.len();

    let ranks: HashMap<&str, usize> = survey
        .files
        .iter()
        .filter_map(|f| ordinance.zone_of(f).map(|z| (f.as_str(), z.rank)))
        .collect();

    let mut held = vec![0_usize; depth];
    for rank in ranks.values() {
        held[*rank] += 1;
    }
    let mut reaches: Vec<HashSet<usize>> = vec![HashSet::new(); depth];
    for edge in &survey.edges {
        if let (Some(&src), Some(&dst)) =
            (ranks.get(edge.src.as_str()), ranks.get(edge.dst.as_str()))
            && dst < src
        {
            reaches[src].insert(dst);
        }
    }

    let widest = held.iter().copied().max().unwrap_or(1).max(1);
    let name_width = ordinance.zones.iter().map(|z| z.name.len()).max().unwrap_or(4).min(14);
    let count_width = widest.to_string().len();

    let mut out = format!(
        "zone map · {} · {depth} zones, high to low\n{dim}{}{reset}\n",
        ordinance.package,
        "─".repeat(RULE)
    );

    for zone in ordinance.zones.iter().rev() {
        let count = held[zone.rank];
        let filled = if count == 0 { 0 } else { (count * BAR).div_ceil(widest).max(1) };
        let bar = format!("{}{}", "█".repeat(filled), "·".repeat(BAR - filled));
        let down = reaches[zone.rank].len();
        let arrow = if down == 0 { "     ".to_owned() } else { format!("↓{down:<4}") };
        let mark = marker(ordinance, zone.rank, &ranks);
        let _ = writeln!(
            out,
            "{:>3} │ {:<name_width$} {dim}{bar}{reset} {count:>count_width$} {dim}{arrow}{reset}{mark} {dim}{}{reset}",
            zone.rank,
            truncate(&zone.name, name_width),
            truncate_words(
                &zone.paths.to_string(),
                RULE.saturating_sub(name_width + BAR + count_width + 16)
            ),
        );
    }

    let reach =
        ordinance.max_hops.map_or_else(|| "unbounded".to_owned(), |n| format!("≤ {n} hops"));
    let _ = write!(
        out,
        "{dim}{}{reset}\n {} files · {} imports · {} seals · {} keeps · {} grants · \
         reach {reach}\n",
        "─".repeat(RULE),
        survey.files.len(),
        survey.edges.len(),
        ordinance.seals.len(),
        ordinance.keeps.len(),
        ordinance.uses.len(),
    );
    if !ordinance.variances.is_empty() {
        let _ = writeln!(
            out,
            " {yellow}{}{reset} ratified variance(s) — each one names what would retire it",
            ordinance.variances.len()
        );
    }
    out
}

/// `⊙` when a sealed directory lives at this height, `⊘` when a kept region does.
///
/// A keep written `**/*_test.zig` governs a rule about tests, not about a place, and
/// marking every zone it touches would mark the whole stack. Only a keep anchored at
/// a path earns the glyph.
fn marker(ordinance: &Ordinance, rank: usize, ranks: &HashMap<&str, usize>) -> &'static str {
    let here: Vec<&str> = ranks.iter().filter(|(_, r)| **r == rank).map(|(f, _)| *f).collect();
    let sealed = ordinance.seals.iter().any(|s| here.iter().any(|f| inside(f, &s.path)));
    let kept = ordinance
        .keeps
        .iter()
        .filter(|k| !k.subject.as_str().starts_with("**/"))
        .any(|k| here.iter().any(|f| k.subject.matches(f)));
    match (sealed, kept) {
        (true, true) => "⊙⊘",
        (true, false) => "⊙ ",
        (false, true) => " ⊘",
        (false, false) => "  ",
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let head: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{head}…")
}

/// [`truncate`], but for a space-separated list of file names — dropping a whole
/// word instead of slicing through it.
///
/// A width-cut `syn…` reads as a typo, not as "and more files that didn't fit":
/// `syntax.zig` and `syncopate.zig` truncate to the same three letters, so the
/// glyph that is supposed to say "there is more" instead erases the one piece
/// of information (which file) a reader came here for.
fn truncate_words(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    for word in text.split(' ') {
        let grown = out.chars().count() + usize::from(!out.is_empty()) + word.chars().count();
        // Room is reserved for " …" up front rather than trimmed after, so the
        // marker itself never becomes the character that overflows the column.
        if grown > width.saturating_sub(2) {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        return truncate(text, width);
    }
    out.push_str(" …");
    out
}
