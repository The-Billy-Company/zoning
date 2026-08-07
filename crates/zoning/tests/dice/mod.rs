//! A seeded source of arbitrary packages, so a failure is a number you can replay.
//!
//! There is no `proptest` here for the same reason there are no dependencies anywhere
//! else in this crate: a gate that runs in everyone's CI should be a static binary you
//! can audit in an afternoon. What a property test actually needs is a generator, a
//! deterministic seed, and invariants worth asserting — none of which requires a
//! framework. `ZONING_CASES` and `ZONING_SEED` turn the same code into a long soak when
//! somebody wants one, and every failure prints the seed that produced it, which is the
//! only part of shrinking that matters at this size.

#![allow(dead_code, reason = "each test binary reaches for a different part of the harness")]
#![allow(clippy::expect_used, reason = "a harness that cannot write its fixture has failed")]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// `splitmix64` — deterministic, four lines, and good enough to find parser bugs.
pub(crate) struct Dice {
    state: u64,
}

impl Dice {
    /// A generator pinned to one seed.
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next value in the stream.
    pub(crate) fn roll(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Below `n`, or zero when there is nothing to choose from.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { usize::try_from(self.roll() % n as u64).unwrap_or(0) }
    }

    /// Somewhere in `low..=high`.
    pub(crate) fn between(&mut self, low: usize, high: usize) -> usize {
        low + self.below(high.saturating_sub(low) + 1)
    }

    /// True one time in `n`.
    pub(crate) fn odds(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }
}

/// How many cases a property runs — `ZONING_CASES`, else the argument.
pub(crate) fn cases(default: usize) -> usize {
    std::env::var("ZONING_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// The base seed — `ZONING_SEED`, else a constant, so CI runs the same stream twice.
pub(crate) fn seed() -> u64 {
    std::env::var("ZONING_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0x5A5A_1234_DEAD_BEEF)
}

/// A generated package on disk, and the graph it was built to have.
pub(crate) struct Grown {
    /// The package root — `contract/` and `src/` live under it.
    pub(crate) root: PathBuf,
    /// Module-relative paths, the facade first.
    pub(crate) files: Vec<String>,
    /// Importer → imported, as indices into [`Grown::files`].
    pub(crate) edges: Vec<(usize, usize)>,
    /// Every import naming a module outside the package: (importer index, module).
    pub(crate) outsiders: Vec<(usize, &'static str)>,
}

/// Directory names, chosen to be legal zone names so a draft can name them.
const HOUSES: [&str; 6] = ["kernel", "surface", "exec", "corpus", "folio", "press"];
/// File stems, likewise.
const ROOMS: [&str; 6] = ["core", "fold", "span", "tally", "weave", "glean"];
/// Outside module names a grown file may reach for. Few on purpose: the `use`-law
/// properties want every one of them either granted or refused, not a vocabulary wide
/// enough to make that bookkeeping the point.
pub(crate) const STRANGERS: [&str; 4] = ["acme", "flux", "orbit", "quark"];

/// Grow a package under `at`, and write it to disk.
///
/// `tangled` decides whether an import may point at a file generated before it. With it
/// off every edge points forward, so the graph is a DAG by construction and any cycle a
/// law reports is a bug in the law. With it on cycles appear, and the property is what
/// the tool does about them.
pub(crate) fn grow(d: &mut Dice, at: &Path, tangled: bool) -> Grown {
    let module = at.join("src");
    let mut files = vec!["root.zig".to_owned()];
    let mut doors = Vec::new();
    let houses = d.between(1, 4);
    for house in &HOUSES[..houses] {
        doors.push(files.len());
        let rooms = d.between(1, 3);
        for room in &ROOMS[..rooms] {
            files.push(format!("{house}/{room}.zig"));
        }
    }

    // The facade reaches one file per directory, so nothing is unreachable and the
    // package has an obvious front door for `draft` to find.
    let mut edges: Vec<(usize, usize)> = doors.iter().map(|&door| (0, door)).collect();
    for from in 1..files.len() {
        for _ in 0..d.between(0, 2) {
            let to = if tangled {
                d.between(1, files.len() - 1)
            } else if from + 1 < files.len() {
                d.between(from + 1, files.len() - 1)
            } else {
                continue;
            };
            if to != from {
                edges.push((from, to));
            }
        }
    }
    edges.sort_unstable();
    edges.dedup();

    // Every file, the facade included, may also reach outside the package — the
    // facade case is the one `draft` got wrong (a grant it cannot scope to anything),
    // so it is not special-cased away here.
    let mut outsiders: Vec<(usize, &'static str)> = Vec::new();
    for from in 0..files.len() {
        for _ in 0..d.between(0, 2) {
            outsiders.push((from, STRANGERS[d.below(STRANGERS.len())]));
        }
    }
    outsiders.sort_unstable();
    outsiders.dedup();

    for (index, rel) in files.iter().enumerate() {
        let path = module.join(rel);
        std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("mkdir");
        let mut text = String::new();
        for (alias, (_, to)) in edges.iter().filter(|(from, _)| *from == index).enumerate() {
            let _ = writeln!(text, "const p{alias} = @import(\"{}\");", spec(rel, &files[*to]));
        }
        for (alias, (_, module_name)) in
            outsiders.iter().filter(|(from, _)| *from == index).enumerate()
        {
            let _ = writeln!(text, "const o{alias} = @import(\"{module_name}\");");
        }
        text.push_str("pub const value: usize = 1;\n");
        std::fs::write(&path, text).expect("write a generated file");
    }

    Grown { root: at.to_path_buf(), files, edges, outsiders }
}

/// How `from` spells an import of `to`, both module-relative.
fn spec(from: &str, to: &str) -> String {
    let here: Vec<&str> = from.split('/').collect();
    let there: Vec<&str> = to.split('/').collect();
    let (from_dirs, to_dirs) = (&here[..here.len() - 1], &there[..there.len() - 1]);
    let shared = from_dirs.iter().zip(to_dirs).take_while(|(a, b)| a == b).count();
    let mut out = "../".repeat(from_dirs.len() - shared);
    for segment in &to_dirs[shared..] {
        out.push_str(segment);
        out.push('/');
    }
    out.push_str(there[there.len() - 1]);
    out
}

/// The smallest contract that governs a grown package, so one law can be studied alone.
///
/// One zone over every directory and no reach ceiling: a violation under this contract is
/// the cycle law or nothing, which is what makes it usable as a control.
pub(crate) fn one_zone(package: &str) -> String {
    format!(
        "package {package} {{\n\
         \x20   root     src\n\
         \x20   language zig\n\
         \x20   facade   root.zig\n\
         }}\n\n\
         zones {{\n\
         \x20   all  */**\n\
         }}\n\n\
         forbid cycles across directories\n"
    )
}

/// Put `text` at `<root>/contract/<package>.zone` and hand back the path.
pub(crate) fn file(root: &Path, package: &str, text: &str) -> PathBuf {
    let dir = root.join("contract");
    std::fs::create_dir_all(&dir).expect("contract dir");
    let path = dir.join(format!("{package}.zone"));
    std::fs::write(&path, text).expect("write a contract");
    path
}

/// A private scratch directory, emptied first so a rerun cannot read a stale tree.
pub(crate) fn scratch(what: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zoning-{what}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}
