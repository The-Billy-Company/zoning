//! What must hold over graphs nobody wrote by hand.
//!
//! The fixtures next door prove each law fires where it should, one hand-built tree per
//! law. Those cannot prove the claims that are about *every* graph — that a drafted
//! contract is true of the tree it came from, that the cycle law finds what a different
//! algorithm finds, that a verdict does not depend on the order a directory happened to
//! be walked in. Each property below generates its own packages from a seed and prints
//! that seed when it fails, so a red run is reproducible with `ZONING_SEED=…`.

#![allow(clippy::expect_used, reason = "a test that cannot build its fixture has failed")]

mod dice;

use std::collections::BTreeSet;

use dice::Dice;
use zoning::judge::{self, dir_of};
use zoning::ordinance::{Law, Ordinance, cycle_subject};
use zoning::survey::{Ask, Survey};

/// The language every generated package is written in.
fn zig() -> &'static dyn zoning::survey::Dialect {
    zoning::survey::dialect("zig").expect("zig ships in-tree")
}

/// Survey a grown package under the contract it is governed by.
fn survey_of(grown: &dice::Grown, ordinance: &Ordinance) -> Survey {
    Survey::of(&Ask {
        repo_root: &grown.root,
        module_root: &ordinance.module_root,
        exclude: &ordinance.exclude,
        dialect: ordinance.dialect,
        tracked: None,
    })
}

/// Directory-spanning import cycles, found by an oracle that shares no code with the law.
///
/// Transitive closure by relaxation to a fixed point, then mutual reachability as the
/// equivalence relation — O(n³) and far too slow to ship, which is exactly why it is a
/// trustworthy second opinion. `judge` uses Tarjan; agreeing with a different algorithm
/// on the same graph is the whole point of asserting it.
fn knots(survey: &Survey) -> BTreeSet<String> {
    let nodes: Vec<&str> = survey.files.iter().map(String::as_str).collect();
    let n = nodes.len();
    let at = |path: &str| nodes.iter().position(|node| *node == path);
    let mut reach = vec![false; n * n];
    for edge in &survey.edges {
        if let (Some(from), Some(to)) = (at(&edge.src), at(&edge.dst)) {
            reach[from * n + to] = true;
        }
    }
    loop {
        let mut grew = false;
        for a in 0..n {
            for b in 0..n {
                if !reach[a * n + b] {
                    continue;
                }
                for c in 0..n {
                    if reach[b * n + c] && !reach[a * n + c] {
                        reach[a * n + c] = true;
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }

    let mut found = BTreeSet::new();
    let mut seen = vec![false; n];
    for a in 0..n {
        if seen[a] || !reach[a * n + a] {
            continue;
        }
        let group: Vec<usize> = (0..n).filter(|&b| reach[a * n + b] && reach[b * n + a]).collect();
        for &member in &group {
            seen[member] = true;
        }
        let spread: BTreeSet<&str> = group.iter().map(|&m| dir_of(nodes[m])).collect();
        if group.len() > 1 && spread.len() > 1 {
            let mut members: Vec<String> = group.iter().map(|&m| nodes[m].to_owned()).collect();
            members.sort();
            found.insert(cycle_subject(&members));
        }
    }
    found
}

/// Run `case` over generated packages, collecting the seeds that failed.
fn over(
    what: &str,
    tangled: bool,
    default: usize,
    mut case: impl FnMut(&dice::Grown) -> Option<String>,
) {
    let scratch = dice::scratch(what);
    let mut broken = Vec::new();
    for index in 0..dice::cases(default) {
        let seed = dice::seed() ^ (index as u64).wrapping_mul(0x1000_0001B3);
        let mut rolls = Dice::new(seed);
        let here = scratch.join(format!("case-{index}"));
        let grown = dice::grow(&mut rolls, &here, tangled);
        if let Some(why) = case(&grown) {
            broken.push(format!("  ZONING_SEED={seed} — {why}"));
        }
        let _ = std::fs::remove_dir_all(&here);
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(broken.is_empty(), "{} case(s) failed:\n{}", broken.len(), broken.join("\n"));
}

#[test]
fn a_drafted_contract_is_true_of_the_graph_it_was_taken_from() {
    // The claim `draft` is sold on: the first `verify` after adopting one is green, so
    // the gate never arrives red and teaches its reader that it is noise. Over a DAG
    // there is nothing a draft is allowed to leave behind — no cycle needing a reason,
    // no zone pointing the wrong way, no grant it forgot to write.
    over("draft-is-true", false, 150, |grown| {
        let bare =
            Ordinance::read(&dice::file(&grown.root, "grown", &dice::one_zone("grown")), zig())
                .expect("the control contract parses");
        let text = zoning::draft::contract(&survey_of(grown, &bare), "grown", "src", &[]);
        let path = dice::file(&grown.root, "grown", &text);
        let ordinance = match Ordinance::read(&path, zig()) {
            Ok(ordinance) => ordinance,
            Err(fault) => return Some(format!("a draft over a DAG must parse:\n{fault}\n{text}")),
        };
        let found = judge::judge(&survey_of(grown, &ordinance), &ordinance);
        (!found.ok()).then(|| {
            format!(
                "drafted contract is not true of its own graph: {:?} {:?}\n{text}",
                found.findings.iter().map(|f| (f.law, &f.message)).collect::<Vec<_>>(),
                found.stale
            )
        })
    });
}

#[test]
fn a_tangle_is_never_drafted_into_silence() {
    // The inverse, and the one a draft must not get wrong in the generous direction: a
    // cycle cannot be described away, so the stanza it emits carries an empty reason and
    // does not parse. A draft that quietly swallowed a tangle would hand somebody a
    // green gate over a graph that has none of the property the gate claims.
    over("tangle-refuses", true, 150, |grown| {
        let bare =
            Ordinance::read(&dice::file(&grown.root, "grown", &dice::one_zone("grown")), zig())
                .expect("the control contract parses");
        let survey = survey_of(grown, &bare);
        let tangled = !knots(&survey).is_empty();
        let text = zoning::draft::contract(&survey, "grown", "src", &[]);
        let read = Ordinance::read(&dice::file(&grown.root, "grown", &text), zig());
        match (tangled, read) {
            (true, Ok(_)) => {
                Some(format!("a tangled graph drafted a contract that parses:\n{text}"))
            }
            (true, Err(fault)) => (!fault.to_string().contains("because"))
                .then(|| format!("the fault must point at the missing reason:\n{fault}")),
            (false, Err(fault)) => Some(format!("an untangled graph must draft cleanly:\n{fault}")),
            (false, Ok(ordinance)) => {
                let found = judge::judge(&survey_of(grown, &ordinance), &ordinance);
                (!found.ok()).then(|| format!("drafted but not true:\n{text}"))
            }
        }
    });
}

#[test]
fn the_cycle_law_finds_what_a_slower_algorithm_finds() {
    // Two implementations of the same claim, and the assertion is that they never
    // disagree — not that either matches a number somebody wrote down. Membership, not
    // just existence: a detector that finds *a* cycle where there are two, or that folds
    // two components into one, is wrong in a way a count would not catch.
    over("cycle-oracle", true, 150, |grown| {
        let ordinance =
            Ordinance::read(&dice::file(&grown.root, "grown", &dice::one_zone("grown")), zig())
                .expect("the control contract parses");
        let survey = survey_of(grown, &ordinance);
        let found = judge::judge(&survey, &ordinance);
        let law: BTreeSet<String> = found
            .findings
            .iter()
            .filter(|f| f.law == Law::Cycle)
            .map(|f| f.subject.clone())
            .collect();
        let oracle = knots(&survey);
        (law != oracle).then(|| format!("the law says {law:?}, the oracle says {oracle:?}"))
    });
}

#[test]
fn a_verdict_does_not_depend_on_the_order_the_tree_was_walked_in() {
    // Nothing promises the order a directory yields its entries in, and a verdict that
    // moves with it is a gate that passes on one machine and fails on another. The
    // cheapest way to catch a stray `HashMap` iteration leaking into an answer is to
    // judge the same graph twice with the edges stirred.
    over("order-free", true, 100, |grown| {
        let ordinance =
            Ordinance::read(&dice::file(&grown.root, "grown", &dice::one_zone("grown")), zig())
                .expect("the control contract parses");
        let mut survey = survey_of(grown, &ordinance);
        let first = judge::judge(&survey, &ordinance);

        let mut rolls = Dice::new(0xC0FF_EE00_1234_5678);
        for index in (1..survey.edges.len()).rev() {
            survey.edges.swap(index, rolls.below(index + 1));
        }
        survey.files.reverse();
        let again = judge::judge(&survey, &ordinance);

        let shape = |found: &judge::Verdict| {
            let mut rows: Vec<String> = found
                .findings
                .iter()
                .map(|f| format!("{} {} {}", f.law, f.subject, f.message))
                .collect();
            rows.sort();
            rows
        };
        (shape(&first) != shape(&again)).then(|| {
            format!(
                "stirring the walk changed the verdict:\n{:?}\n{:?}",
                shape(&first),
                shape(&again)
            )
        })
    });
}

#[test]
fn every_finding_is_visible_from_some_file_that_explains_itself() {
    // `verify` and `explain` are separate renderings of one judgement, and the failure
    // worth guarding is a finding that `verify` reports but no file's explanation can
    // show — a violation you cannot navigate to. So the two verbs are held to agreeing
    // at the package level, in both directions.
    over("explain-agrees", true, 100, |grown| {
        let ordinance =
            Ordinance::read(&dice::file(&grown.root, "grown", &dice::one_zone("grown")), zig())
                .expect("the control contract parses");
        let survey = survey_of(grown, &ordinance);
        let found = judge::judge(&survey, &ordinance);
        let ink = zoning::report::Ink::PLAIN;
        let dirty: Vec<&String> = survey
            .files
            .iter()
            .filter(|rel| !zoning::report::file(rel, &ordinance, &survey, &ink).clean)
            .collect();
        match (found.findings.is_empty(), dirty.is_empty()) {
            (true, false) => Some(format!("verify is clean but {dirty:?} explain as not")),
            (false, true) => Some(format!(
                "{} finding(s) no file can show: {:?}",
                found.findings.len(),
                found.findings.iter().map(|f| &f.subject).collect::<Vec<_>>()
            )),
            _ => None,
        }
    });
}
