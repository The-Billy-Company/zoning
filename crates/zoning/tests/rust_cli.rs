// Copyright (c) 2026 The Billy Company
//! Real CLI coverage and editor diagnostics for Rust use the same fallible survey.

#![allow(clippy::expect_used, reason = "a test that cannot construct its fixture has failed")]

mod dice;
mod editor;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use editor::Editor;

struct Package(PathBuf);

impl Package {
    fn new() -> Self {
        let root = dice::scratch("rust-cli");
        fs::create_dir_all(root.join("src")).expect("source directory");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("manifest");
        Self(root.canonicalize().expect("canonical fixture"))
    }

    fn write(&self, name: &str, source: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().expect("source parent")).expect("source directory");
        fs::write(path, source).expect("source");
    }

    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zone"))
            .args(arguments)
            .env("ZONING_NO_SETUP", "1")
            .current_dir(&self.0)
            .output()
            .expect("the real CLI runs")
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rust_discovery_draft_and_complete_verification_cannot_pass_an_ungoverned_crate() {
    let package = Package::new();
    package.write("src/lib.rs", "pub fn answer() -> u32 { 42 }\n");
    let listed = package.run(&["list", "--root", ".", "--untracked", "--no-color"]);
    assert!(listed.status.success(), "{}", String::from_utf8_lossy(&listed.stderr));
    let list = String::from_utf8(listed.stdout).expect("UTF-8 output");
    assert!(
        list.contains("ungoverned") && list.contains("sample") && list.contains("rust"),
        "{list}"
    );
    let missing = package.run(&["verify", "--root", ".", "--untracked", "--complete"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stdout).contains("no contract"));
    let drafted = package.run(&["draft", ".", "--write", "--untracked"]);
    assert!(drafted.status.success(), "{}", String::from_utf8_lossy(&drafted.stderr));
    let contract = fs::read_to_string(package.0.join("sample.zone")).expect("the drafted contract");
    assert!(contract.contains("language rust") && contract.contains("root     ."), "{contract}");
    let clean = package.run(&["verify", "--root", ".", "--untracked", "--complete"]);
    assert!(
        clean.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&clean.stdout),
        String::from_utf8_lossy(&clean.stderr)
    );
    package.write("new_layer/extra.rs", "pub fn extra() {}\n");
    let uncovered = package.run(&["verify", "--root", ".", "--untracked", "--complete"]);
    assert_eq!(uncovered.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&uncovered.stdout).contains("no zone claims"));
}

#[test]
fn editor_reads_root_layout_rust_contracts_and_reports_parser_failures() {
    let package = Package::new();
    let contract =
        "package sample {\nroot src\nlanguage rust\n}\nzones {\nlow low/**\nhigh high/**\n}\n";
    package.write("charter.zone", contract);
    package.write("src/lib.rs", "mod low; mod high;\n");
    package.write("src/low/mod.rs", "use crate::high::Thing;\n");
    package.write("src/high/mod.rs", "pub struct Thing;\n");
    // The crate root is deliberately unclaimed as well: source diagnostics must
    // locate the low/high crossing, rather than stopping at that other finding.
    let source = package.0.join("src/low/mod.rs");
    let uri = format!("file://{}", source.display());
    let mut editor = Editor::attached();
    editor.hello();
    let diagnostics = editor.open(&uri, "use crate::high::Thing;\n");
    assert!(diagnostics.iter().any(|diagnostic| diagnostic["code"] == "zone"), "{diagnostics:?}");
    editor.goodbye();

    package.write("src/low/mod.rs", "use crate::{;\n");
    let mut editor = Editor::attached();
    editor.hello();
    let diagnostics = editor.open(&uri, "use crate::{;\n");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("cannot parse Rust"))),
        "{diagnostics:?}"
    );
    editor.goodbye();
    let invalid = package.run(&["verify", "--root", ".", "--untracked"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("cannot parse Rust"));
}

#[test]
fn coverage_uses_present_manifests_when_the_git_index_still_names_a_removed_crate() {
    let package = Package::new();
    package.write("src/lib.rs", "pub fn answer() -> u32 { 42 }\n");
    package.write(
        "sample.zone",
        "package sample {\nroot .\nlanguage rust\n}\nzones {\nsource src/**\n}\n",
    );
    package.write("removed/Cargo.toml", "[package]\nname = \"removed\"\nversion = \"0.1.0\"\n");
    package.write("removed/src/lib.rs", "");
    for arguments in [
        vec!["init", "--quiet"],
        vec![
            "add",
            "--",
            "Cargo.toml",
            "src/lib.rs",
            "sample.zone",
            "removed/Cargo.toml",
            "removed/src/lib.rs",
        ],
    ] {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(&package.0)
            .status()
            .expect("fixture git index");
        assert!(status.success());
    }
    fs::remove_dir_all(package.0.join("removed"))
        .expect("remove the old crate from the working tree");
    let checked = package.run(&["verify", "--root", ".", "--complete", "--untracked"]);
    assert!(
        checked.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
    let listed = package.run(&["list", "--root", ".", "--untracked"]);
    assert!(listed.status.success());
    assert!(!String::from_utf8_lossy(&listed.stdout).contains("removed"));
}
