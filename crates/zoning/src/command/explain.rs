use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zoning::ordinance::{self, Ordinance};
use zoning::report;
use zoning::survey::{Ask, Survey};
use zoning::Result;

use super::Tracked;
use super::args::Options;

/// One file's standing, or one edge's verdict.
///
/// Paths are taken as typed, resolved against the directory the shell is in — the way
/// every other tool takes a path, and the way a person reading a stack trace or an
/// editor tab has it. Making somebody translate into module-relative coordinates first
/// is the friction that stops a diagnostic verb from being used at all.
pub(super) fn explain(
    options: &Options,
    root: &Path,
    tracked: &mut Tracked<'_>,
) -> Result<ExitCode> {
    let here =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    let wanted: Vec<PathBuf> = options
        .args
        .iter()
        .map(|arg| {
            here.join(arg)
                .canonicalize()
                .map_err(|_| format!("no such file: `{arg}`").into())
        })
        .collect::<Result<_>>()?;

    let mut best: Option<Ordinance> = None;
    for path in ordinance::discover(root, &options.under) {
        let contract = Ordinance::read(&path, options.language)?;
        let holds = wanted.iter().all(|w| w.starts_with(&contract.module_root));
        let deeper =
            best.as_ref().is_none_or(|prior| contract.module_root.starts_with(&prior.module_root));
        if holds && deeper {
            best = Some(contract);
        }
    }
    let Some(contract) = best else {
        return Err(format!(
            "no governed package holds {} — `zoning list` shows what is governed, and \
             `zoning draft <dir>` starts a contract for what is not",
            options.args.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(" and ")
        )
        .into());
    };

    let found = Survey::of(&Ask {
        repo_root: root,
        module_root: &contract.module_root,
        exclude: &contract.exclude,
        dialect: contract.dialect,
        tracked: tracked.of(contract.dialect),
    });
    let named: Vec<String> =
        wanted.iter().map(|path| judged(path, &contract, &found, tracked)).collect::<Result<_>>()?;

    let answer = match named.as_slice() {
        [one] => report::file(one, &contract, &found, &options.ink),
        [from, to] => report::edge(from, to, &contract, &found, &options.ink),
        _ => return Err("`explain` takes one file, or two".into()),
    };
    print!("{}", answer.text);
    let _ = std::io::stdout().flush();
    // The same code `verify` uses for the same news, so "is this legal" is a shell
    // question: `zoning explain a b && …`. A malformed invocation is still 2.
    Ok(if answer.clean { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

/// A path's module-relative name, or exactly why the contract has nothing to say.
///
/// "Not in the judged set" has four causes and they call for four different actions, so
/// naming the cause is the whole job. The one that bites in a shared worktree is an
/// untracked file: it is genuinely part of nobody's committed architecture yet, and a
/// gate that silently ignored it would be right, while a report that silently ignored
/// the *question about it* would just look broken.
fn judged(
    path: &Path,
    contract: &Ordinance,
    found: &Survey,
    tracked: &mut Tracked<'_>,
) -> Result<String> {
    let rel = path
        .strip_prefix(&contract.module_root)
        .map_err(|_| format!("{} is outside {}", path.display(), contract.module_root.display()))?
        .to_string_lossy()
        .replace('\\', "/");
    if found.files.contains(&rel) {
        return Ok(rel);
    }
    let dialect = contract.dialect;
    let why = if !dialect.extensions().iter().any(|e| rel.ends_with(&format!(".{e}"))) {
        format!("{} reads {}", dialect.name(), dialect.extensions().join(", "))
    } else if let Some(pattern) = contract.exclude.iter().find(|p| p.matches(&rel)) {
        format!("the contract excludes `{pattern}`")
    } else if tracked.of(dialect).is_some_and(|known| !known.contains(path)) {
        "version control does not know it yet — commit it, or pass `--untracked`".to_owned()
    } else {
        "it is not in the judged set".to_owned()
    };
    Err(format!("{} holds `{rel}` but does not judge it: {why}", contract.package).into())
}
