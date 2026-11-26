// Copyright (c) 2026 The Billy Company
//! The native Cargo boundary is bounded, fallible and cached only for one survey run.
//!
//! Each CLI process receives its own fake Cargo executable and response files. No
//! test changes the test process's environment, runs a package build script or
//! downloads dependencies. Cargo's real target semantics are checked in rust.rs.

#![allow(clippy::expect_used, reason = "a test that cannot construct its fixture has failed")]

mod dice;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Serialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Serialize)]
struct Package {
    name: String,
    manifest_path: PathBuf,
    targets: Vec<Target>,
    dependencies: Vec<Dependency>,
}

#[derive(Serialize)]
struct Target {
    name: String,
    kind: Vec<String>,
    src_path: PathBuf,
    edition: String,
}

#[derive(Serialize)]
struct Dependency {
    name: String,
    rename: Option<String>,
    path: Option<PathBuf>,
}

struct Fixture {
    scratch: PathBuf,
    root: PathBuf,
    cargo: PathBuf,
    response: PathBuf,
    trace: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let scratch =
            dice::scratch("rust-metadata").canonicalize().expect("canonical scratch path");
        let root = scratch.join("workspace");
        fs::create_dir_all(&root).expect("workspace directory");
        let fixture = Self {
            cargo: scratch.join(format!("cargo-metadata-helper{}", std::env::consts::EXE_SUFFIX)),
            response: scratch.join("metadata.json"),
            trace: scratch.join("queries.txt"),
            scratch,
            root,
        };
        let source = fixture.scratch.join("helper.rs");
        fs::write(&source, HELPER).expect("fake Cargo source");
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2024", "--crate-name=zoning_metadata_helper"])
            .arg(&source)
            .arg("-o")
            .arg(&fixture.cargo)
            .output()
            .expect("compile the portable fake Cargo executable");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        fixture
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.root.join(name);
        fs::create_dir_all(path.parent().expect("a fixture file has a parent")).expect("folder");
        fs::write(path, text).expect("fixture file");
    }

    fn package(&self, name: &str) -> Package {
        let base = self.root.join(name);
        self.write(
            &format!("{name}/Cargo.toml"),
            &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
        );
        self.write(&format!("{name}/src/lib.rs"), "pub struct Value;\n");
        self.write(
            &format!("{name}/charter.zone"),
            &format!("package {name} {{\nroot src\nlanguage rust\n}}\nzones {{\nall **\n}}\n"),
        );
        Package {
            name: name.to_owned(),
            manifest_path: base.join("Cargo.toml"),
            targets: vec![Target {
                name: name.to_owned(),
                kind: vec!["lib".to_owned()],
                src_path: base.join("src/lib.rs"),
                edition: "2024".to_owned(),
            }],
            dependencies: Vec::new(),
        }
    }

    fn metadata(&self, packages: Vec<Package>) {
        fs::write(
            &self.response,
            serde_json::to_vec(&Metadata { packages }).expect("typed metadata"),
        )
        .expect("metadata response");
    }

    fn run(&self, mode: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zone"))
            .args(["verify", "--root", ".", "--untracked", "--complete", "--no-color"])
            .env("ZONING_NO_SETUP", "1")
            .env("CARGO", &self.cargo)
            .env("ZONING_TEST_METADATA_MODE", mode)
            .env("ZONING_TEST_METADATA_RESPONSE", &self.response)
            .env("ZONING_TEST_METADATA_TRACE", &self.trace)
            .current_dir(&self.root)
            .output()
            .expect("the real zoning CLI runs")
    }

    fn queries(&self) -> usize {
        fs::read_to_string(&self.trace)
            .expect("the fake Cargo trace")
            .lines()
            .filter(|line| *line == "query")
            .count()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.scratch);
    }
}

#[test]
fn one_locked_offline_metadata_query_serves_every_package_in_a_session() {
    let fixture = Fixture::new();
    fixture.write("Cargo.toml", "[workspace]\nmembers = [\"alpha\", \"beta\"]\nresolver = \"3\"\n");
    fixture.metadata(vec![fixture.package("alpha"), fixture.package("beta")]);
    let output = fixture.run("success");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.queries(), 1, "workspace members must share their invocation's catalog");
    let trace = fs::read_to_string(&fixture.trace).expect("query trace");
    let arguments: Vec<&str> = trace.lines().collect();
    assert_eq!(
        &arguments[..8],
        [
            "query",
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ]
    );
    let manifest = Path::new(arguments[8]);
    assert!(manifest.is_absolute() && manifest.ends_with("Cargo.toml"));
}

#[test]
fn malformed_missing_failed_and_oversized_metadata_cannot_produce_a_clean_verdict() {
    let fixture = Fixture::new();
    let package = fixture.package("alpha");
    fixture.metadata(vec![package]);
    for (mode, expected) in [
        ("malformed", "invalid cargo metadata output"),
        ("failure", "deliberate metadata failure"),
        ("oversized", "16 MiB"),
    ] {
        let output = fixture.run(mode);
        assert_eq!(output.status.code(), Some(2), "{mode} must be a technical failure");
        let error = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
        assert!(error.contains(expected), "{mode}: {error}");
        assert!(!String::from_utf8_lossy(&output.stdout).contains('✓'));
    }
    fixture.metadata(Vec::new());
    let missing = fixture.run("success");
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("does not contain"));
    assert_eq!(fixture.queries(), 4, "failed catalogs must not survive into a fresh invocation");
}

#[test]
fn a_fresh_invocation_rereads_metadata_and_source_after_a_failed_attempt() {
    let fixture = Fixture::new();
    let mut package = fixture.package("alpha");
    package.dependencies.push(Dependency { name: "vendor".to_owned(), rename: None, path: None });
    fixture.metadata(vec![package]);
    fixture.write("alpha/src/lib.rs", "use vendor::Thing; pub struct Value;\n");
    let first = fixture.run("success");
    assert_eq!(first.status.code(), Some(1), "the ungranted dependency must be rejected");
    assert!(String::from_utf8_lossy(&first.stdout).contains("[use]"));

    fixture.write("alpha/src/lib.rs", "pub struct Corrected;\n");
    let second = fixture.run("success");
    assert!(second.status.success(), "{}", String::from_utf8_lossy(&second.stderr));
    assert_eq!(fixture.queries(), 2, "a new invocation must start with a fresh Cargo session");
}

#[test]
fn an_exited_metadata_leader_cannot_leave_descendants_holding_capture_pipes() {
    let fixture = Fixture::new();
    fixture.metadata(vec![fixture.package("alpha")]);
    let started = Instant::now();
    let output = fixture.run("inherited");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(fixture.trace.with_extension("ready").is_file(), "the pipe holder actually started");
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "metadata waited for an exited leader's eight-second pipe holder"
    );
    assert!(!fixture.trace.with_extension("done").exists(), "the descendant must be terminated");
}

const HELPER: &str = r#"
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn path(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).expect("test path variable")
}

fn main() -> io::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--hold-pipes") {
        let trace = path("ZONING_TEST_METADATA_TRACE");
        fs::write(trace.with_extension("ready"), b"ready")?;
        std::thread::sleep(Duration::from_secs(8));
        fs::write(trace.with_extension("done"), b"finished")?;
        return Ok(());
    }
    let mut trace = OpenOptions::new().create(true).append(true)
        .open(path("ZONING_TEST_METADATA_TRACE"))?;
    writeln!(trace, "query")?;
    for argument in std::env::args_os().skip(1) {
        writeln!(trace, "{}", argument.to_string_lossy())?;
    }
    match std::env::var("ZONING_TEST_METADATA_MODE").expect("test response mode").as_str() {
        "malformed" => io::stdout().write_all(b"{invalid"),
        "failure" => {
            eprintln!("deliberate metadata failure");
            std::process::exit(23);
        }
        "oversized" => io::copy(&mut io::repeat(b' ').take(16 * 1024 * 1024 + 1), &mut io::stdout()).map(|_| ()),
        "success" => io::stdout().write_all(&fs::read(path("ZONING_TEST_METADATA_RESPONSE"))?),
        "inherited" => {
            let mut holder = Command::new(std::env::current_exe()?).arg("--hold-pipes")
                .stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn()?;
            let deadline = Instant::now() + Duration::from_secs(2);
            while !path("ZONING_TEST_METADATA_TRACE").with_extension("ready").exists() {
                if Instant::now() >= deadline {
                    holder.kill()?;
                    holder.wait()?;
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "pipe holder did not start"));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            io::stdout().write_all(&fs::read(path("ZONING_TEST_METADATA_RESPONSE"))?)
        }
        other => panic!("unknown fake Cargo response mode: {other}"),
    }
}
"#;

#[test]
fn an_unknown_cargo_edition_cannot_be_judged_with_current_resolution_rules() {
    let fixture = Fixture::new();
    let mut package = fixture.package("sample");
    package.targets.first_mut().expect("the fixture has a library").edition = "2099".to_owned();
    fixture.metadata(vec![package]);
    let output = fixture.run("success");
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert!(error.contains("src/lib.rs") && error.contains("edition unknown"), "{error}");
    assert!(
        error.contains("2018, 2021 or 2024") && error.contains("compiler-based analysis"),
        "{error}"
    );
}
