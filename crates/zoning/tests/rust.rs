// Copyright (c) 2026 The Billy Company
//! Rust syntax and target ownership must produce the same seven-law graph.
//!
//! Fixtures exercise valid Rust spellings that can otherwise hide a crossing. One
//! is checked by the real compiler; the others isolate graph and policy boundaries
//! without building a Cargo dependency tree or executing package build scripts.

#![allow(clippy::expect_used, reason = "a test that cannot construct its fixture has failed")]

mod dice;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use zoning::judge::{self, Verdict};
use zoning::ordinance::{Law, Ordinance};
use zoning::survey::{Ask, Dialect, Survey};

fn rust() -> &'static dyn Dialect {
    zoning::survey::dialect("rust").expect("the Rust dialect is registered")
}

struct Fixture {
    scratch: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let scratch = dice::scratch("rust");
        let fixture = Self { root: scratch.join("package"), scratch };
        fixture.write(
            "Cargo.toml",
            "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        fixture
    }

    fn write(&self, path: &str, source: &str) {
        let file = self.root.join(path);
        fs::create_dir_all(file.parent().expect("a source has a parent")).expect("source folder");
        fs::write(file, source).expect("fixture source");
    }

    fn survey(&self, root: &str) -> zoning::Result<Survey> {
        Survey::of(&Ask {
            session: &zoning::survey::Session::default(),
            repo_root: &self.root,
            module_root: &self.root.join(root),
            exclude: &[],
            dialect: rust(),
            package: "sample",
            tracked: None,
        })
    }

    fn verdict(&self, root: &str, facade: bool, clauses: &str) -> Verdict {
        let facade = if facade { "facade lib.rs" } else { "" };
        let text =
            format!("package sample {{\nroot {root}\nlanguage rust\n{facade}\n}}\n{clauses}\n");
        let ordinance = Ordinance::from_source(&self.root.join("sample.zone"), &text, rust())
            .expect("the fixture contract parses");
        let survey = self.survey(root).expect("the Rust graph resolves");
        judge::judge(&survey, &ordinance)
    }

    fn error(&self) -> String {
        match self.survey(".") {
            Ok(_) => panic!("an incomplete Rust survey must not report success"),
            Err(error) => error.to_string(),
        }
    }

    fn compile(&self) {
        self.compile_with_edition("2024");
    }

    fn compile_with_edition(&self, edition: &str) {
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--crate-type=lib", "--crate-name=sample", "--emit=metadata", "--cfg=test"])
            .arg(format!("--edition={edition}"))
            .arg(self.root.join("src/lib.rs"))
            .arg("--out-dir")
            .arg(&self.scratch)
            .output()
            .expect("the Rust compiler is required by this Rust test suite");
        assert!(
            output.status.success(),
            "the independent compiler rejected the graph fixture:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.scratch);
    }
}

fn has_edge(survey: &Survey, from: &str, to: &str) -> bool {
    survey.edges.iter().any(|edge| edge.src == from && edge.dst == to)
}

fn laws(verdict: &Verdict) -> Vec<Law> {
    let mut found: Vec<Law> = verdict.findings.iter().map(|finding| finding.law).collect();
    found.sort_unstable();
    found.dedup();
    found
}

#[test]
fn compiler_accepted_groups_aliases_inline_scopes_and_prose_keep_their_graph() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod leaf;\npub mod api;\n");
    fixture.write("src/leaf/mod.rs", "pub struct Value;\npub fn read() -> u8 { 1 }\n");
    fixture.write(
        "src/api/mod.rs",
        r####"
use crate::leaf::{self as renamed, Value as Alias};
pub fn read() -> u8 {
    let _: Alias = renamed::Value;
    crate::leaf::read()
}
pub fn borrowed<'a>(value: &'a str) -> &'a str { value }
pub const TEXT: &str = r###"use forbidden::Thing; crate::ghost::read();"###;
/* outer /* use hidden::Thing; */ crate::ghost::read(); */
#[cfg(test)]
mod tests {
    use super::super::leaf::Value;
    #[test]
    fn constructs() { let _ = Value; }
}
"####,
    );
    fixture.compile();

    for (root, api, leaf) in
        [("src", "api/mod.rs", "leaf/mod.rs"), (".", "src/api/mod.rs", "src/leaf/mod.rs")]
    {
        let survey = fixture.survey(root).expect("valid Rust resolves");
        assert!(has_edge(&survey, api, leaf));
        assert!(survey.outside.is_empty(), "prose or a local binding became an external crate");
        assert!(
            survey.edges.iter().any(|edge| edge.src == api && edge.dst == leaf && edge.hops == 0),
            "crate-root references must not acquire synthetic parent hops"
        );
        assert!(
            survey.edges.iter().any(|edge| edge.src == api && edge.dst == leaf && edge.hops == 2),
            "super walks two logical modules from the inline test scope"
        );
    }
}

#[test]
fn upward_imports_and_their_exact_variance_use_the_existing_zone_law() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod low; pub mod high;");
    fixture.write("src/low/mod.rs", "pub fn read() -> u8 { crate::high::read() }");
    fixture.write("src/high/mod.rs", "pub fn read() -> u8 { 1 }");
    let clauses = "zones {\nlow low/**\nhigh high/**\n}";
    let rejected = fixture.verdict("src", true, clauses);
    assert_eq!(laws(&rejected), [Law::Zone]);
    let allowed = fixture.verdict(
        "src",
        true,
        &format!(
            "{clauses}\nvariance zone low/mod.rs -> high/mod.rs \
             because \"Temporary call; retire when the shared primitive moves below both.\""
        ),
    );
    assert!(allowed.ok(), "the existing variance should ratify the precise edge");
    assert_eq!(allowed.ratified.len(), 1);
}

fn sealed_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod store; pub mod api;");
    fixture.write("src/store/mod.rs", "pub mod internal; pub use internal::Thing;");
    fixture.write("src/store/internal.rs", "pub struct Thing; pub fn read() -> u8 { 1 }");
    fixture
}

const SEALED: &str = "zones {\nstore store/**\napi api/**\n}\nseal store through mod.rs";

#[test]
fn entering_a_reexport_through_a_seal_passes_and_an_alias_cannot_reach_past_it() {
    let fixture = sealed_fixture();
    fixture.write("src/api/mod.rs", "use crate::store::Thing; pub fn read() { let _ = Thing; }");
    let legal = fixture.verdict("src", true, SEALED);
    assert!(legal.ok(), "a public reexport enters through the sealed module's door");

    fixture.write(
        "src/api/mod.rs",
        "use crate::store as renamed; pub fn read() -> u8 { renamed::internal::read() }",
    );
    let bypass = fixture.verdict("src", true, SEALED);
    assert_eq!(laws(&bypass), [Law::Seal]);
    assert!(
        bypass.findings.iter().any(|finding| finding.subject == "api/mod.rs -> store/internal.rs")
    );
}

#[test]
fn a_kept_region_and_scoped_external_grant_keep_their_existing_meaning() {
    let fixture = Fixture::new();
    fixture.write(
        "Cargo.toml",
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         [dependencies]\nvendor_alias = { package = \"actual-package\", version = \"1\" }\n",
    );
    fixture.write("src/lib.rs", "pub mod value; pub mod api;");
    fixture.write("src/value/mod.rs", "pub struct Value;");
    fixture.write("src/api/mod.rs", "use crate::value::Value; use vendor_alias::Thing;");
    let clauses = "zones {\nvalue value/**\napi api/**\n}";
    let refused = fixture.verdict("src", true, &format!("{clauses}\nkeep value/** to nobody"));
    assert_eq!(laws(&refused), [Law::Keep, Law::Use]);
    let allowed = fixture.verdict("src", true, &format!("{clauses}\nuse vendor_alias by api"));
    assert!(allowed.ok(), "the source-visible Cargo alias must exercise its scoped grant");
}

#[test]
fn module_declarations_are_ownership_and_real_cross_directory_cycles_still_fail() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod parent;");
    fixture.write("src/parent.rs", "pub struct Value; mod child;");
    fixture.write("src/parent/child.rs", "use super::Value; fn read() { let _ = Value; }");
    let survey = fixture.survey("src").expect("parent/child graph");
    assert!(has_edge(&survey, "parent/child.rs", "parent.rs"));
    assert!(!has_edge(&survey, "parent.rs", "parent/child.rs"), "mod declares ownership");

    fixture.write("src/lib.rs", "pub mod a; pub mod b; pub mod parent;");
    fixture
        .write("src/a/mod.rs", "pub fn read() -> u8 { crate::b::VALUE } pub const VALUE: u8 = 1;");
    fixture
        .write("src/b/mod.rs", "pub fn read() -> u8 { crate::a::VALUE } pub const VALUE: u8 = 2;");
    let found = fixture.verdict("src", false, "zones {\nall **\n}");
    assert_eq!(laws(&found), [Law::Cycle]);
}

#[test]
fn custom_test_targets_have_their_own_crate_root_and_can_use_the_package_library() {
    let fixture = Fixture::new();
    fixture.write(
        "Cargo.toml",
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         [lib]\nname = \"engine\"\n\
         [[test]]\nname = \"oracle\"\npath = \"checks/oracle.rs\"\n",
    );
    fixture.write("src/lib.rs", "pub mod library;");
    fixture.write("src/library.rs", "pub fn read() -> u8 { 1 }");
    fixture.write(
        "checks/oracle.rs",
        "mod helpers; use crate::helpers::run; use engine::library::read; \
         #[test] fn checks() { assert_eq!(run(), read()); }",
    );
    fixture.write("checks/helpers.rs", "pub fn run() -> u8 { 1 }");
    let survey = fixture.survey(".").expect("custom Cargo targets resolve");
    assert!(has_edge(&survey, "checks/oracle.rs", "checks/helpers.rs"));
    assert!(!has_edge(&survey, "checks/oracle.rs", "src/library.rs"));
    assert!(
        survey
            .outside
            .iter()
            .any(|outside| { outside.src == "checks/oracle.rs" && outside.spec == "engine" })
    );
    let refused = fixture.verdict(".", false, "zones {\nall **\n}");
    assert_eq!(laws(&refused), [Law::Use]);
    let granted = fixture.verdict(".", false, "zones {\nall **\n}\nuse engine by checks/**");
    assert!(granted.ok(), "a separate target enters the package library through an external grant");
}

#[test]
fn cargo_owns_nonstandard_library_binary_example_benchmark_and_build_roots() {
    let fixture = Fixture::new();
    fixture.write(
        "Cargo.toml",
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         build = \"build/entry.rs\"\n\
         [lib]\nname = \"engine\"\npath = \"library/entry.rs\"\n\
         [[bin]]\nname = \"runner\"\npath = \"commands/run.rs\"\n\
         [[example]]\nname = \"demo\"\npath = \"showcase/demo.rs\"\n\
         [[bench]]\nname = \"speed\"\npath = \"perf/speed.rs\"\n",
    );
    for (entry, helper) in [
        ("library/entry.rs", "library/local.rs"),
        ("commands/run.rs", "commands/local.rs"),
        ("showcase/demo.rs", "showcase/local.rs"),
        ("perf/speed.rs", "perf/local.rs"),
        ("build/entry.rs", "build/local.rs"),
    ] {
        fixture.write(entry, "mod local; fn main() { crate::local::run(); }");
        fixture.write(helper, "pub fn run() {}");
    }
    let survey = fixture.survey(".").expect("Cargo's real target source paths resolve");
    for (entry, helper) in [
        ("library/entry.rs", "library/local.rs"),
        ("commands/run.rs", "commands/local.rs"),
        ("showcase/demo.rs", "showcase/local.rs"),
        ("perf/speed.rs", "perf/local.rs"),
        ("build/entry.rs", "build/local.rs"),
    ] {
        assert!(has_edge(&survey, entry, helper), "{entry} must resolve its own crate root");
    }
}

#[test]
fn conditional_module_alternatives_are_a_source_union() {
    let fixture = Fixture::new();
    fixture.write(
        "src/lib.rs",
        "#[cfg(feature = \"fast\")] #[path = \"fast.rs\"] mod selected;\n\
         #[cfg(not(feature = \"fast\"))] #[path = \"slow.rs\"] mod selected;\n\
         pub mod api;\n",
    );
    fixture.write("src/fast.rs", "pub fn read() -> u8 { 1 }");
    fixture.write("src/slow.rs", "pub fn read() -> u8 { 2 }");
    fixture.write("src/api.rs", "pub fn read() -> u8 { crate::selected::read() }");
    let survey = fixture.survey("src").expect("conditional modules resolve as alternatives");
    assert!(has_edge(&survey, "api.rs", "fast.rs"));
    assert!(has_edge(&survey, "api.rs", "slow.rs"));
}

#[test]
fn conditional_path_attributes_preserve_default_and_selected_files() {
    let fixture = Fixture::new();
    fixture.write(
        "src/lib.rs",
        "#[cfg_attr(feature = \"fast\", path = \"fast.rs\")] mod selected; pub mod api;",
    );
    fixture.write("src/fast.rs", "pub fn read() -> u8 { 1 }");
    fixture.write("src/selected.rs", "pub fn read() -> u8 { 2 }");
    fixture.write("src/api.rs", "pub fn read() -> u8 { crate::selected::read() }");
    let survey = fixture.survey("src").expect("conditional paths resolve as alternatives");
    assert!(has_edge(&survey, "api.rs", "fast.rs"));
    assert!(has_edge(&survey, "api.rs", "selected.rs"));
}

#[test]
fn paths_written_in_macro_arguments_cannot_hide_a_dependency() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod leaf; pub mod api;");
    fixture.write("src/leaf.rs", "pub fn read() -> u8 { 1 }");
    fixture.write(
        "src/api.rs",
        "pub fn check() { assert_eq!(crate::leaf::read(), 1); \
         let _ = r###\"crate::ghost::read()\"###; }",
    );
    let survey = fixture.survey("src").expect("macro argument source resolves");
    assert!(has_edge(&survey, "api.rs", "leaf.rs"));
    assert!(survey.outside.is_empty(), "macro prose is not a dependency");
}

#[test]
fn a_local_module_named_std_cannot_bypass_a_seal_as_an_ambient_crate() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod api;");
    fixture.write("src/api/mod.rs", "pub mod std; pub fn read() -> u8 { std::secret::read() }");
    fixture.write("src/api/std/mod.rs", "pub mod secret;");
    fixture.write("src/api/std/secret.rs", "pub fn read() -> u8 { 1 }");
    fixture.compile();
    let survey = fixture.survey("src").expect("local modules shadow the extern prelude");
    assert!(has_edge(&survey, "api/mod.rs", "api/std/secret.rs"));
    let found = fixture.verdict("src", false, "zones {\nall **\n}\nseal api/std through mod.rs");
    assert_eq!(laws(&found), [Law::Seal]);
}

#[test]
fn an_extern_crate_self_alias_cannot_hide_a_seal_crossing() {
    let fixture = Fixture::new();
    fixture.write(
        "src/lib.rs",
        "extern crate self as alias; pub mod store; \
         pub fn read() -> u8 { alias::store::internal::read() }",
    );
    fixture.write("src/store/mod.rs", "pub mod internal;");
    fixture.write("src/store/internal.rs", "pub fn read() -> u8 { 1 }");
    fixture.compile();
    let survey = fixture.survey("src").expect("the self alias names this crate's real module tree");
    assert!(has_edge(&survey, "lib.rs", "store/internal.rs"));
    let found = fixture.verdict("src", false, "zones {\nall **\n}\nseal store through mod.rs");
    assert_eq!(laws(&found), [Law::Seal]);
}

#[test]
fn a_block_local_inline_module_cannot_hide_its_qualified_references() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod store; pub mod api;");
    fixture.write("src/store/mod.rs", "pub mod internal;");
    fixture.write("src/store/internal.rs", "pub fn read() -> u8 { 1 }");
    fixture.write(
        "src/api.rs",
        "pub fn read() -> u8 { \
         mod local { pub fn read() -> u8 { super::super::store::internal::read() } } \
         local::read() }",
    );
    fixture.compile();
    let survey = fixture.survey("src").expect("block-local modules retain their logical parents");
    assert!(has_edge(&survey, "api.rs", "store/internal.rs"));
    assert!(
        survey.edges.iter().any(|edge| {
            edge.src == "api.rs" && edge.dst == "store/internal.rs" && edge.hops == 2
        })
    );
    let found = fixture.verdict("src", false, "zones {\nall **\n}\nseal store through mod.rs");
    assert_eq!(laws(&found), [Law::Seal]);
}

#[test]
fn path_modules_that_leave_the_source_root_hit_the_existing_escape_law() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "#[path = \"../shared.rs\"] mod shared;");
    fixture.write("shared.rs", "pub struct Shared;");
    let survey = fixture.survey("src").expect("escaping path is a located departure");
    assert_eq!(survey.escapes.len(), 1);
    let found = fixture.verdict("src", false, "zones {\nroot lib.rs\n}");
    assert_eq!(laws(&found), [Law::Escape]);
}

#[test]
fn malformed_rust_and_cargo_manifests_are_technical_errors() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub fn broken( {");
    let source_error = fixture.error();
    assert!(source_error.contains("lib.rs"), "the parse failure must locate its source");

    fixture.write("src/lib.rs", "pub struct Valid;");
    fixture.write("Cargo.toml", "[package\nname = \"sample\"\n");
    let manifest_error = fixture.error();
    assert!(manifest_error.contains("Cargo.toml"), "the manifest failure must locate its source");
}

#[test]
fn missing_declared_modules_are_not_silently_dropped() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "mod missing;");
    let error = fixture.error();
    assert!(error.contains("missing"), "the missing module must be named: {error}");
}

#[test]
fn literal_source_inclusion_keeps_the_including_logical_module() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod leaf; pub mod api;");
    fixture.write("src/leaf.rs", "pub fn read() -> u8 { 1 }");
    fixture.write("src/api.rs", "include!(\"shared/body.rs\");");
    fixture.write("src/shared/body.rs", "pub fn read() -> u8 { super::leaf::read() }");
    fixture.compile();
    let survey = fixture.survey("src").expect("literal included source is parsed in its module");
    assert!(has_edge(&survey, "api.rs", "shared/body.rs"));
    assert!(has_edge(&survey, "shared/body.rs", "leaf.rs"));
    assert!(survey.escapes.is_empty(), "physical include depth must not change logical scope");
}

#[test]
fn nonliteral_source_inclusion_reports_its_unsupported_boundary() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "include!(concat!(\"body\", \".rs\"));");
    fixture.write("src/body.rs", "pub struct Value;");
    let error = fixture.error();
    assert!(error.contains("include"), "the unsupported source expansion must be named: {error}");
}

#[test]
fn unreadable_utf8_source_is_a_technical_error() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "pub mod broken;");
    fs::write(fixture.root.join("src/broken.rs"), [0xff, 0xfe]).expect("malformed UTF-8 fixture");
    let error = fixture.error();
    assert!(error.contains("broken.rs"), "the unreadable source must be located: {error}");
}

#[test]
fn absolute_macro_paths_keep_the_extern_anchor_when_a_local_module_shadows_it() {
    let fixture = Fixture::new();
    fixture.write(
        "src/lib.rs",
        r"mod std;
macro_rules! import { ($($path:tt)*) => { use $($path)*; }; }
import!(::std::{mem::size_of as measure});
pub fn check() {
    assert_eq!(::std::mem::size_of::<u8>(), 1);
    assert_eq!(1, ::std::mem::size_of::<u8>());
    assert_eq!(measure::<u8>(), 1);
    match true { true => ::std::mem::drop(()), false => () }
}
",
    );
    fixture.write("src/std.rs", "pub fn local() {}\n");
    fixture.compile();
    let survey = fixture.survey("src").expect("absolute extern paths resolve");
    assert!(
        !has_edge(&survey, "lib.rs", "std.rs"),
        "an absolute extern path must not enter the shadowing local module"
    );
    assert!(survey.outside.iter().any(|reference| reference.spec == "std"));
    let verdict = fixture.verdict("src", false, "zones {\ncore lib.rs\nshadow std.rs\n}");
    assert!(
        verdict.ok(),
        "absolute standard-library paths must remain ambient: {:?}",
        laws(&verdict)
    );
}

#[test]
fn legacy_rust_edition_paths_fail_explicitly_instead_of_becoming_external_grants() {
    let fixture = Fixture::new();
    fixture.write(
        "Cargo.toml",
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2015\"\n",
    );
    fixture.write("src/lib.rs", "mod leaf; mod api;\n");
    fixture.write("src/leaf.rs", "pub struct Thing;\n");
    fixture.write("src/api.rs", "use leaf::Thing; pub fn read() { let _ = Thing; }\n");
    fixture.compile_with_edition("2015");
    let error = fixture.error();
    assert!(
        error.replace('\\', "/").contains("src/lib.rs") && error.contains("edition 2015"),
        "{error}"
    );
    assert!(
        error.contains("2018, 2021 or 2024") && error.contains("compiler-based analysis"),
        "{error}"
    );
}

#[test]
fn edition_validation_follows_the_governed_cargo_target_scope() {
    let fixture = Fixture::new();
    fixture.write("Cargo.toml", "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[[example]]\nname = \"legacy\"\npath = \"examples/legacy.rs\"\nedition = \"2015\"\n");
    fixture.write("src/lib.rs", "pub fn read() {}\n");
    fixture.write("examples/legacy.rs", "fn main() {}\n");
    fixture.survey("src").expect("an out-of-scope example must not restrict the library survey");
    let error = fixture.error();
    assert!(
        error.replace('\\', "/").contains("examples/legacy.rs") && error.contains("edition 2015"),
        "{error}"
    );
}
