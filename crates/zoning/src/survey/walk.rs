//! Which files are even eligible to be judged.
//!
//! Two answers, and the tool wants their intersection: what is on disk under the
//! module root, and what the version-control index admits to. Judging only tracked
//! files is deliberate — an untracked `zz_probe.zig` someone is mid-thought on is
//! not architecture, and failing everyone's build on it would be the worse error.
//! It is also exactly the file set CI checks out, so local and CI agree.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Directories no source walk should ever enter.
const SKIP: [&str; 8] = [
    "zig-out",
    ".zig-cache",
    "zig-cache",
    "target",
    "vendor",
    "node_modules",
    ".git",
    "__pycache__",
];

/// Every source file under `root`, as (absolute path, root-relative posix path).
///
/// Sorted, so a verdict does not depend on directory-entry order.
#[must_use]
pub(super) fn source_files(root: &Path, extensions: &[&str]) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if kind.is_dir() {
                if !SKIP.contains(&name.as_ref()) {
                    stack.push(path);
                }
            } else if extensions.iter().any(|e| has_extension(&name, e))
                && let Ok(rel) = path.strip_prefix(root)
            {
                found.push((path.clone(), posix(rel)));
            }
        }
    }
    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
}

/// Every version-controlled source file in the worktree, or `None` if git cannot say.
///
/// Asked once per worktree by the caller, not once per package: a monorepo gate
/// judges several packages that all live in the same index.
#[must_use]
pub fn tracked(repo_root: &Path, extensions: &[&str]) -> Option<HashSet<PathBuf>> {
    let mut command = Command::new("git");
    command.arg("ls-files").arg("-z").arg("--").current_dir(repo_root);
    for extension in extensions {
        command.arg(format!("*.{extension}"));
    }
    let out = command.output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(|p| repo_root.join(p))
            .collect(),
    )
}

fn has_extension(name: &str, extension: &str) -> bool {
    name.len() > extension.len() + 1
        && name.ends_with(extension)
        && name.as_bytes()[name.len() - extension.len() - 1] == b'.'
}

fn posix(path: &Path) -> String {
    path.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}
