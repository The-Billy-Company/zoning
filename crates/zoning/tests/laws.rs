//! Each law fires on the fixture built to break it, and on nothing else.
//!
//! A gate that fails for the wrong reason will pass for the wrong reason too, so
//! every case asserts *which* law fired rather than merely that something did.

#![allow(clippy::expect_used, reason = "a test that cannot build its fixture has failed")]

use std::path::{Path, PathBuf};

use zoning::judge::{self, Verdict};
use zoning::ordinance::{Law, Ordinance};
use zoning::survey::{Ask, Survey};

fn verdict(kind: &str, name: &str) -> Verdict {
    let box_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(kind)
        .join(name);
    let contract = box_dir.join("contract").join(format!("{name}.zone"));
    let ordinance = Ordinance::read(&contract)
        .unwrap_or_else(|fault| panic!("fixture {kind}/{name} does not parse:\n{fault}"));
    let survey = Survey::of(&Ask {
        repo_root: &box_dir,
        module_root: &ordinance.module_root,
        exclude: &ordinance.exclude,
        dialect: zoning::survey::dialect("zig").expect("zig ships in-tree"),
        tracked: None,
    });
    assert!(
        !survey.files.is_empty(),
        "fixture {kind}/{name} surveyed no files — did the tree move?"
    );
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
fn an_import_named_only_in_a_comment_or_a_string_is_not_an_import() {
    let found = verdict("pass", "prose");
    assert!(found.ok(), "prose was read as code: {:?}", laws(&found));
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

    let Err(fault) = Ordinance::read(&contract) else {
        panic!("an unclosed `zones` block must not parse");
    };
    let rendered = fault.to_string();
    assert!(rendered.contains("broken.zone:"), "the fault must locate itself: {rendered}");
    assert!(rendered.contains('^'), "the fault must underline the token: {rendered}");
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn discovery_finds_every_fixture_and_ignores_the_rest() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let found = zoning::ordinance::discover(&fixtures, &["pass".into(), "fail".into()]);
    let names: Vec<String> = found
        .iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    for expected in ["layered", "prose", "uphill", "bypass", "trespass", "knot", "stale"] {
        assert!(names.contains(&expected.to_owned()), "discovery missed {expected}: {names:?}");
    }
    assert!(found.iter().all(|p| p.extension() == Some(Path::new("zone").as_os_str())));
}
