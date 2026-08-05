//! Each law fires on the fixture built to break it, and on nothing else.
//!
//! A gate that fails for the wrong reason will pass for the wrong reason too, so
//! every case asserts *which* law fired rather than merely that something did.

#![allow(clippy::expect_used, reason = "a test that cannot build its fixture has failed")]

use std::path::{Path, PathBuf};

use zoning::judge::{self, Verdict};
use zoning::ordinance::{Law, Ordinance};
use zoning::survey::{Ask, Survey};

/// The language a fixture is read in unless its own contract names another.
fn zig() -> &'static dyn zoning::survey::Dialect {
    zoning::survey::dialect("zig").expect("zig ships in-tree")
}

/// Where a fixture package lives.
fn box_of(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures").join(kind).join(name)
}

/// A fixture's contract and the graph it governs.
fn fixture(kind: &str, name: &str) -> (Ordinance, Survey) {
    let box_dir = box_of(kind, name);
    let contract = box_dir.join("contract").join(format!("{name}.zone"));
    let ordinance = Ordinance::read(&contract, zig())
        .unwrap_or_else(|fault| panic!("fixture {kind}/{name} does not parse:\n{fault}"));
    let survey = Survey::of(&Ask {
        repo_root: &box_dir,
        module_root: &ordinance.module_root,
        exclude: &ordinance.exclude,
        dialect: ordinance.dialect,
        package: &ordinance.package,
        tracked: None,
    });
    assert!(
        !survey.files.is_empty(),
        "fixture {kind}/{name} surveyed no files — did the tree move?"
    );
    (ordinance, survey)
}

fn verdict(kind: &str, name: &str) -> Verdict {
    let (ordinance, survey) = fixture(kind, name);
    judge::judge(&survey, &ordinance)
}

/// The laws a verdict actually complained about, deduplicated.
fn laws(found: &Verdict) -> Vec<Law> {
    let mut seen: Vec<Law> = found.findings.iter().map(|f| f.law).collect();
    seen.sort_unstable();
    seen.dedup();
    seen
}

#[test]
fn a_contract_that_uses_every_construct_and_is_obeyed_passes() {
    let found = verdict("pass", "layered");
    assert!(found.ok(), "expected clean, got {:?}", laws(&found));
    assert_eq!(found.ratified.len(), 1, "the one variance should have been spent");
    assert!(found.stale.is_empty());
}

#[test]
fn a_manifest_declares_a_package_without_belonging_to_it() {
    // `pass/host` roots at the package root, so `build.zig` sits beside the module's own
    // code and imports both outside the package and the vendored dependency. Judged, it
    // would break `escape` and `use` at once — a package cannot be governed if declaring
    // it is a violation.
    let (ordinance, survey) = fixture("pass", "host");
    let found = judge::judge(&survey, &ordinance);
    assert!(found.ok(), "the build script was judged as module code: {:?}", laws(&found));
    assert!(
        !survey.files.iter().any(|f| f == "build.zig"),
        "build.zig reached the judged set: {:?}",
        survey.files
    );
}

#[test]
fn a_vendored_dependency_is_governed_where_it_came_from() {
    // No allowlist decides this. `pass/host`'s own manifest calls `borrowed` a path
    // dependency, and a build that had not said so would not link — so the fact is
    // already written down, and coverage reads it rather than asking anybody to
    // maintain a second copy of it.
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let found = zoning::ordinance::parcels(&fixtures, &["pass".into()]);
    let borrowed = found
        .iter()
        .find(|p| p.dir == "pass/host/borrowed")
        .expect("the vendored package is still a package");
    assert_eq!(borrowed.vendored_by.as_deref(), Some("pass/host"));
    let host = found.iter().find(|p| p.dir == "pass/host").expect("the host is a package");
    assert_eq!(host.vendored_by, None, "the host vendors, it is not vendored");
}

#[test]
fn an_import_named_only_in_a_comment_or_a_string_is_not_an_import() {
    let found = verdict("pass", "prose");
    assert!(found.ok(), "prose was read as code: {:?}", laws(&found));
}

#[test]
fn a_second_dialect_is_read_and_judged_through_the_same_pipeline() {
    // `pass/snake` declares `language python` itself, overriding the `zig()`
    // fallback `fixture` passes every other case — proof the contract's own word
    // wins, not a special path for this test. It exercises an absolute import into
    // a zone, a relative import within one, a standard-library import that needs
    // no grant, and one external dependency that does.
    let (ordinance, survey) = fixture("pass", "snake");
    assert_eq!(ordinance.dialect.name(), "python");
    let found = judge::judge(&survey, &ordinance);
    assert!(found.ok(), "expected clean, got {:?}", laws(&found));
}

#[test]
fn each_law_fires_on_the_fixture_built_to_break_it() {
    for (fixture, expected) in [
        ("uphill", Law::Zone),
        ("bypass", Law::Seal),
        ("trespass", Law::Keep),
        ("knot", Law::Cycle),
        ("faraway", Law::Reach),
        ("escapee", Law::Escape),
        ("orphan", Law::Zone),
        ("stranger", Law::Use),
    ] {
        let found = verdict("fail", fixture);
        assert!(!found.ok(), "fixture fail/{fixture} was expected to fail");
        assert_eq!(
            laws(&found),
            vec![expected],
            "fail/{fixture} should trip {expected} and nothing else"
        );
    }
}

#[test]
fn reaching_outside_the_package_needs_a_grant_and_the_facade_is_no_exception() {
    let found = verdict("fail", "stranger");
    assert_eq!(found.findings.len(), 2, "an ungranted module and a misscoped one both count");
    let refused: Vec<&str> = found.findings.iter().map(|f| f.path.as_str()).collect();
    assert!(
        refused.iter().any(|p| p.ends_with("lib.zig")),
        "a module nobody was granted: {refused:?}"
    );
    assert!(
        refused.iter().any(|p| p.ends_with("root.zig")),
        "the facade has no zone, so a zone-scoped grant must not reach it: {refused:?}"
    );
    assert!(found.stale.is_empty(), "the grant was exercised once, so it is not stale");
}

#[test]
fn an_ambient_module_needs_no_grant_and_a_scoped_one_is_enough() {
    let found = verdict("pass", "outward");
    assert!(found.ok(), "expected clean, got {:?}", laws(&found));
    assert!(found.stale.is_empty(), "both grants were spent: {:?}", found.stale);
}

#[test]
fn a_grant_nobody_exercised_is_stale_like_any_other_dead_permission() {
    let found = verdict("fail", "idle");
    assert!(found.findings.is_empty(), "the code is clean; only the contract is wrong");
    assert_eq!(found.stale.len(), 1, "the unspent grant must be reported");
    assert!(found.stale[0].starts_with("use ledger"), "named as written: {:?}", found.stale);
}

#[test]
fn a_variance_that_no_longer_matches_anything_is_itself_a_failure() {
    let found = verdict("fail", "stale");
    assert!(found.findings.is_empty(), "the code is clean; only the contract is wrong");
    assert_eq!(found.stale.len(), 1, "the unspent variance must be reported");
    assert!(!found.ok(), "a stale declaration has to fail the build, or nobody deletes it");
}

#[test]
fn a_malformed_contract_names_the_word_that_broke_it() {
    let scratch = std::env::temp_dir().join("zoning-malformed-contract");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(scratch.join("contract")).expect("scratch dir");
    std::fs::create_dir_all(scratch.join("src")).expect("scratch src");
    let contract = scratch.join("contract/broken.zone");
    std::fs::write(&contract, "package broken {\n    root src\n}\n\nzones {\n    low   low.zig\n")
        .expect("write contract");

    let Err(fault) = Ordinance::read(&contract, zig()) else {
        panic!("an unclosed `zones` block must not parse");
    };
    let rendered = fault.to_string();
    assert!(rendered.contains("broken.zone:"), "the fault must locate itself: {rendered}");
    assert!(rendered.contains('^'), "the fault must underline the token: {rendered}");
    let _ = std::fs::remove_dir_all(&scratch);
}

/// A fixture tree copied somewhere writable, so a test may draft into it.
fn cloned(kind: &str, name: &str) -> PathBuf {
    let scratch = std::env::temp_dir().join(format!("zoning-draft-{kind}-{name}"));
    let _ = std::fs::remove_dir_all(&scratch);
    copy_tree(&box_of(kind, name), &scratch);
    scratch
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("scratch dir");
    for entry in std::fs::read_dir(from).expect("fixture dir").flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().is_ok_and(|k| k.is_dir()) {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("copy fixture file");
        }
    }
}

/// Draft a contract over a copied fixture and judge the package by it.
fn drafted(kind: &str, name: &str) -> (PathBuf, Result<Ordinance, zoning::ordinance::Fault>) {
    let box_dir = cloned(kind, name);
    let module_root = box_dir.join("src");
    let survey = Survey::of(&Ask {
        repo_root: &box_dir,
        module_root: &module_root,
        exclude: &[],
        dialect: zig(),
        package: name,
        tracked: None,
    });
    let text = zoning::draft::contract(&survey, name, "src", &[]);
    let contract = box_dir.join("contract").join(format!("{name}.zone"));
    std::fs::create_dir_all(contract.parent().expect("contract dir")).expect("contract dir");
    std::fs::write(&contract, &text).expect("write the draft");
    let read = Ordinance::read(&contract, zig());
    (box_dir, read)
}

#[test]
fn a_draft_describes_the_graph_it_was_taken_from() {
    let (box_dir, read) = drafted("pass", "layered");
    let ordinance = read.unwrap_or_else(|fault| panic!("a draft must parse:\n{fault}"));
    let survey = Survey::of(&Ask {
        repo_root: &box_dir,
        module_root: &ordinance.module_root,
        exclude: &ordinance.exclude,
        dialect: ordinance.dialect,
        package: &ordinance.package,
        tracked: None,
    });
    let found = judge::judge(&survey, &ordinance);
    assert!(
        found.ok(),
        "a contract drafted from a graph must be true of that graph, got {:?}\n{:?}",
        laws(&found),
        found.stale
    );
    let _ = std::fs::remove_dir_all(&box_dir);
}

#[test]
fn a_draft_over_a_tangled_package_refuses_to_parse_until_somebody_writes_the_reason() {
    let (box_dir, read) = drafted("fail", "knot");
    let Err(fault) = read else {
        panic!("a cycle cannot be drafted away — the variance it needs has no reason yet");
    };
    let rendered = fault.to_string();
    assert!(rendered.contains("because"), "the fault must point at the empty reason: {rendered}");
    let _ = std::fs::remove_dir_all(&box_dir);
}

#[test]
fn an_explained_edge_carries_its_verdict_for_the_shell() {
    // `zone explain a b && …` is only worth writing if the answer reaches the exit
    // code, so the rendered answer says which it was rather than making the caller grep
    // its own prose. Judged over the fixture whose whole purpose is an uphill import.
    let (ordinance, survey) = fixture("fail", "uphill");
    let ink = zoning::report::Ink::PLAIN;
    let uphill = survey
        .edges
        .iter()
        .find(|e| {
            let (src, dst) = (ordinance.zone_of(&e.src), ordinance.zone_of(&e.dst));
            matches!((src, dst), (Some(a), Some(b)) if a.rank < b.rank)
        })
        .expect("the uphill fixture has an uphill edge");

    let refused = zoning::report::edge(&uphill.src, &uphill.dst, &ordinance, &survey, &ink);
    assert!(!refused.clean, "an uphill import is not clean:\n{}", refused.text);
    assert!(refused.text.contains("allowed    no"), "and it says so:\n{}", refused.text);

    // The same edge the other way round is the legal one, by construction.
    let allowed = zoning::report::edge(&uphill.dst, &uphill.src, &ordinance, &survey, &ink);
    assert!(allowed.clean, "downhill is allowed:\n{}", allowed.text);
}

#[test]
fn a_package_is_named_what_its_manifest_names_it() {
    // A contract filed under the directory's name while the build calls the package
    // something else is a name nobody can use twice: `--package` would want one and
    // `build.zig.zon` the other. The fixture's manifest declares `host`.
    let manifest = box_of("pass", "host").join("build.zig.zon");
    let text = std::fs::read_to_string(&manifest).expect("the host fixture has a manifest");
    assert_eq!(zig().declared(&text).as_deref(), Some("host"));
}

#[test]
fn discovery_finds_every_fixture_and_ignores_the_rest() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let found = zoning::ordinance::discover(&fixtures, &["pass".into(), "fail".into()]);
    let names: Vec<String> = found
        .iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    for expected in ["layered", "prose", "snake", "uphill", "bypass", "trespass", "knot", "stale"] {
        assert!(names.contains(&expected.to_owned()), "discovery missed {expected}: {names:?}");
    }
    assert!(found.iter().all(|p| p.extension() == Some(Path::new("zone").as_os_str())));
}
