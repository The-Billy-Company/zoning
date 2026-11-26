// Copyright (c) 2026 The Billy Company
//! Cargo owns target discovery and inherited dependency declarations.
//!
//! One locked, offline metadata query fills the invocation's cache for every
//! package in that workspace. No build script, target build or dependency fetch
//! runs, and a new editor survey gets a new cache instead of stale global state.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use process_wrap::std::{ChildWrapper, CommandWrap, KillOnDrop};
use serde::Deserialize;

use crate::Result;
use crate::survey::{Context, Source};

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct Manifest {
    package: Option<DeclaredPackage>,
}

#[derive(Deserialize)]
struct DeclaredPackage {
    name: String,
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Clone, Deserialize)]
struct Package {
    manifest_path: PathBuf,
    targets: Vec<Target>,
    dependencies: Vec<Dependency>,
}

#[derive(Clone, Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
    src_path: PathBuf,
    edition: Edition,
}

#[derive(Clone, Copy, Deserialize)]
enum Edition {
    #[serde(rename = "2015")]
    Rust2015,
    #[serde(rename = "2018")]
    Rust2018,
    #[serde(rename = "2021")]
    Rust2021,
    #[serde(rename = "2024")]
    Rust2024,
    #[serde(other)]
    Unknown,
}

impl Edition {
    fn unsupported(self) -> Option<&'static str> {
        match self {
            Self::Rust2018 | Self::Rust2021 | Self::Rust2024 => None,
            Self::Rust2015 => Some("2015"),
            Self::Unknown => Some("unknown"),
        }
    }
}

#[derive(Clone, Deserialize)]
struct Dependency {
    name: String,
    rename: Option<String>,
    path: Option<PathBuf>,
}

/// Metadata shared only by surveys in one invocation.
#[derive(Default)]
pub(crate) struct Cache {
    packages: BTreeMap<PathBuf, Package>,
}

pub(super) struct Cargo {
    pub roots: Vec<String>,
    pub library: Option<(String, String)>,
    pub externs: BTreeSet<String>,
}

impl Manifest {
    pub(super) fn parse(text: &str) -> Option<Self> {
        toml::from_str(text).ok()
    }
    pub(super) fn name(&self) -> Option<String> {
        self.package.as_ref().map(|package| package.name.clone()).filter(|name| !name.is_empty())
    }
}

impl Cargo {
    pub(super) fn read(context: &Context<'_>) -> Result<Self> {
        let manifest = context
            .module_root
            .ancestors()
            .map(|parent| parent.join("Cargo.toml"))
            .find(|candidate| candidate.is_file());
        let Some(manifest) = manifest else {
            return Ok(Self {
                roots: fallback_roots(context),
                library: None,
                externs: BTreeSet::new(),
            });
        };
        let text = fs::read_to_string(&manifest).map_err(|error| {
            format!("{}: cannot read Cargo manifest: {error}", manifest.display())
        })?;
        let declared: Manifest = toml::from_str(&text)
            .map_err(|error| format!("{}: invalid Cargo manifest: {error}", manifest.display()))?;
        if declared.package.is_none() {
            return Ok(Self {
                roots: fallback_roots(context),
                library: None,
                externs: BTreeSet::new(),
            });
        }
        let manifest = manifest.canonicalize().map_err(|error| {
            format!("{}: cannot resolve Cargo manifest: {error}", manifest.display())
        })?;
        let mut cache = context.session.cargo.borrow_mut();
        if !cache.packages.contains_key(&manifest) {
            let metadata = metadata(&manifest)?;
            for package in metadata.packages {
                cache.packages.insert(package.manifest_path.clone(), package);
            }
        }
        let package = cache
            .packages
            .get(&manifest)
            .ok_or_else(|| format!("Cargo metadata does not contain {}", manifest.display()))?;
        let module_root = context.module_root.canonicalize().map_err(|error| {
            format!("{}: cannot resolve Rust source root: {error}", context.module_root.display())
        })?;
        let relative = |path: &Path| normalize(path).strip_prefix(&module_root).ok().map(posix);
        let roots = admitted_roots(package, &module_root, context.sources)?;
        let library =
            package.targets.iter().find(|target| library_target(target)).and_then(|target| {
                relative(&target.src_path).map(|path| (target.name.clone(), path))
            });
        let externs = package
            .dependencies
            .iter()
            .map(|dependency| {
                if let Some(rename) = &dependency.rename {
                    return rename.replace('-', "_");
                }
                if let Some(path) = &dependency.path
                    && let Some(target) =
                        cache.packages.get(&path.join("Cargo.toml")).and_then(|package| {
                            package.targets.iter().find(|target| library_target(target))
                        })
                {
                    return target.name.clone();
                }
                dependency.name.replace('-', "_")
            })
            .collect();
        Ok(Self { roots, library, externs })
    }
}

fn admitted_roots(package: &Package, root: &Path, sources: &[Source]) -> Result<Vec<String>> {
    let mut roots = Vec::new();
    for target in &package.targets {
        let path = normalize(&target.src_path);
        let Ok(relative) = path.strip_prefix(root) else { continue };
        let relative = posix(relative);
        if !sources.iter().any(|source| source.path == relative) {
            continue;
        }
        if let Some(edition) = target.edition.unsupported() {
            return Err(format!(
                "{}: Rust edition {edition} is not supported by this source survey; use Cargo edition 2018, 2021 or 2024, or use compiler-based analysis for this target",
                target.src_path.display()
            ).into());
        }
        roots.push(relative);
    }
    Ok(roots)
}

fn library_target(target: &Target) -> bool {
    target.kind.iter().any(|kind| {
        matches!(kind.as_str(), "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro")
    })
}

fn metadata(manifest: &Path) -> Result<Metadata> {
    const LIMIT: u64 = 16 * 1024 * 1024;
    let executable = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(executable);
    command
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest)
        .current_dir(manifest.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut command = CommandWrap::from(command);
    #[cfg(unix)]
    command.wrap(process_wrap::std::ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(process_wrap::std::JobObject);
    command.wrap(KillOnDrop);
    let mut child = command
        .spawn()
        .map_err(|error| format!("{}: cannot run cargo metadata: {error}", manifest.display()))?;
    let stdout = child.stdout().take().ok_or("cargo metadata stdout was not captured")?;
    let stderr = child.stderr().take().ok_or("cargo metadata stderr was not captured")?;
    let result = std::thread::scope(move |scope| -> Result<(ExitStatus, Vec<u8>, Vec<u8>)> {
        let output = scope.spawn(move || read_bounded(stdout, LIMIT));
        let errors = scope.spawn(move || read_bounded(stderr, LIMIT));
        let deadline = Instant::now() + Duration::from_secs(20);
        let status: Result<ExitStatus> = loop {
            match leader_status(child.as_mut()) {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    break Err("cargo metadata exceeded its 20-second deadline".into());
                }
                Err(error) => {
                    break Err(format!("cannot wait for cargo metadata: {error}").into());
                }
            }
        };
        let cleanup = terminate_tree(child.as_mut());
        let stdout = output.join().map_err(|_| "cargo metadata stdout reader failed")??;
        let stderr = errors.join().map_err(|_| "cargo metadata stderr reader failed")??;
        cleanup?;
        Ok((status?, stdout, stderr))
    });
    let (status, stdout, stderr) = result?;
    if !status.success() {
        return Err(format!(
            "{}: cargo metadata failed: {}",
            manifest.display(),
            String::from_utf8_lossy(&stderr).trim()
        )
        .into());
    }
    let metadata = serde_json::from_slice(&stdout).map_err(|error| {
        format!("{}: invalid cargo metadata output: {error}", manifest.display())
    })?;
    Ok(metadata)
}

fn leader_status(child: &mut dyn ChildWrapper) -> io::Result<Option<ExitStatus>> {
    // KillOnDrop wraps our group/job wrapper, which wraps the native child. Waiting
    // on the native leader first lets us terminate inherited-pipe holders before
    // the group/job's own wait tries to wait for every descendant.
    #[cfg(any(unix, windows))]
    let leader = child.inner_mut().inner_mut();
    #[cfg(not(any(unix, windows)))]
    let leader = child.inner_mut();
    leader.try_wait()
}

fn terminate_tree(child: &mut dyn ChildWrapper) -> Result<()> {
    if let Err(error) = child.start_kill() {
        #[cfg(unix)]
        let already_gone = error.raw_os_error() == Some(libc::ESRCH);
        #[cfg(not(unix))]
        let already_gone = error.kind() == io::ErrorKind::NotFound;
        if !already_gone {
            return Err(format!("cannot terminate cargo metadata process tree: {error}").into());
        }
    }
    child.wait().map_err(|error| format!("cannot reap cargo metadata process tree: {error}"))?;
    Ok(())
}

fn read_bounded(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("cargo metadata output exceeds the 16 MiB limit".into());
    }
    Ok(bytes)
}

fn fallback_roots(context: &Context<'_>) -> Vec<String> {
    context
        .sources
        .iter()
        .filter(|source| {
            matches!(source.path.as_str(), "lib.rs" | "main.rs" | "src/lib.rs" | "src/main.rs")
        })
        .map(|source| source.path.clone())
        .collect()
}

pub(super) fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

pub(super) fn posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
