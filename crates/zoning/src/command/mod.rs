mod args;
mod draft;
mod explain;
mod list;
mod scope;

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use args::{Options, Verb};
use scope::{basename, module, probe, scope, tail};
use zoning::Result;
use zoning::judge::{self, Verdict};
use zoning::ordinance::{self, Fault, Ordinance};
use zoning::report::{self, Ink};
use zoning::survey::{self, Ask, Dialect, Survey};

/// Which files version control knows about, per language, read at most once each.
///
/// A monorepo gate judges several packages out of one worktree, and asking git the same
/// question per package is the difference between a gate people run and one they skip.
/// Keyed by language because a polyglot run asks about different extensions.
struct Tracked<'a> {
    root: &'a Path,
    off: bool,
    seen: HashMap<&'static str, Option<HashSet<PathBuf>>>,
}

impl<'a> Tracked<'a> {
    fn new(root: &'a Path, off: bool) -> Self {
        Self { root, off, seen: HashMap::new() }
    }

    fn of(&mut self, dialect: &'static dyn Dialect) -> Option<&HashSet<PathBuf>> {
        if self.off {
            return None;
        }
        self.seen
            .entry(dialect.name())
            .or_insert_with(|| survey::tracked(self.root, dialect.extensions()))
            .as_ref()
    }
}

pub(crate) fn run() -> Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !matches!(args.first().map(String::as_str), Some("lsp" | "setup")) {
        zoning::setup::auto();
    }
    let Some(options) = args::parse(args.into_iter())? else {
        return Ok(ExitCode::SUCCESS);
    };
    if options.verb == Verb::Lsp {
        return zoning::lsp::serve_stdio().map(|()| ExitCode::SUCCESS);
    }
    if options.verb == Verb::Setup {
        let action = match options.args[0].as_str() {
            "status" => zoning::setup::Action::Status,
            "run" => zoning::setup::Action::Run,
            "repair" => zoning::setup::Action::Repair,
            "uninstall" => zoning::setup::Action::Uninstall,
            other => {
                return Err(format!(
                    "unknown setup action `{other}` — try status, run, repair, or uninstall"
                )
                .into());
            }
        };
        for line in zoning::setup::execute(action)? {
            println!("{line}");
        }
        return Ok(ExitCode::SUCCESS);
    }
    let here = match &options.root {
        Some(given) => given.clone(),
        None => std::env::current_dir()
            .map_err(|e| format!("cannot read the current directory: {e}"))?,
    };
    let (root, contracts) = scope(&here, options.root.is_some(), &options.under);
    // Rooted at the worktree, not at the scope: "what does version control know" is one
    // question per checkout, and a scope-sized answer would go blind the moment a verb
    // was pointed at a directory beside the one that set the scope.
    let anchor = zoning::repo_root(&here);
    let mut tracked = Tracked::new(&anchor, options.untracked);

    match options.verb {
        Verb::Draft => return draft::draft(&options, &mut tracked),
        Verb::List => return list::list(&options, &root, &mut tracked),
        Verb::Explain => return explain::explain(&options, &root, &mut tracked),
        _ => {}
    }

    if contracts.is_empty() && !options.complete {
        let Ink { green, reset, dim, .. } = options.ink;
        println!(
            "{green}✓{reset} zoning: nothing governed under {} {dim}— `zoning list` shows what \
             could be, `zoning draft .` writes the first one{reset}",
            tail(&root)
        );
        return Ok(ExitCode::SUCCESS);
    }

    let mut failed = false;
    let mut verdicts: Vec<Verdict> = Vec::new();
    let mut out = String::new();

    for path in &contracts {
        let contract = match Ordinance::read(path, options.language) {
            Ok(contract) => contract,
            Err(fault) => {
                report_fault(&fault, options.ink);
                failed = true;
                continue;
            }
        };
        if !options.packages.is_empty() && !options.packages.contains(&contract.package) {
            continue;
        }
        if options.verb == Verb::Show {
            out.push_str(&report::show(&contract, options.ink));
            continue;
        }

        let found = Survey::of(&Ask {
            repo_root: &root,
            module_root: &contract.module_root,
            exclude: &contract.exclude,
            dialect: contract.dialect,
            tracked: tracked.of(contract.dialect),
        });

        if options.verb == Verb::Map {
            out.push_str(&report::map(&contract, &found, options.ink));
            continue;
        }
        let verdict = judge::judge(&found, &contract);
        failed |= !verdict.ok();
        if options.suggest {
            out.push_str(&report::suggest(&verdict, &contract));
        } else if !options.json {
            out.push_str(&report::verdict(&verdict, options.ink, options.verb == Verb::Status));
        }
        verdicts.push(verdict);
    }

    if options.complete {
        failed |= !ungoverned(&options, &root, &mut tracked, &mut out)?;
    }
    if options.json {
        out.push_str(&report::records(&verdicts));
    }
    print!("{out}");
    let _ = std::io::stdout().flush();

    Ok(if failed { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

/// Whether every package in scope has a contract, printing the ones that do not.
///
/// The laws are all claims about a governed package, so a clean `verify` says nothing
/// about the package somebody added last week — and adoption that cannot notice a new
/// ungoverned package is adoption that quietly rots back to zero. This is the coverage
/// claim, kept behind a flag because it is a different question from the seven laws and
/// belongs to a repository that has finished adopting.
fn ungoverned(
    options: &Options,
    root: &Path,
    tracked: &mut Tracked<'_>,
    out: &mut String,
) -> Result<bool> {
    let Ink { red, dim, reset, .. } = options.ink;
    let mut clean = true;
    for parcel in ordinance::parcels(root, &options.under) {
        if !parcel.contracts.is_empty() || parcel.vendored_by.is_some() {
            continue;
        }
        let dialect = survey::dialect(parcel.language)
            .ok_or_else(|| format!("no dialect named `{}`", parcel.language))?;
        let dir = root.join(&parcel.dir);
        let (source, inside) = module(&dir);
        let found = probe(&dir, source, dialect, tracked, &inside);
        // A manifest with no source behind it declares a package without being a module,
        // so there is nothing for a contract to say about it and nothing missing.
        if found.files.is_empty() {
            continue;
        }
        clean = false;
        let _ = writeln!(
            out,
            "{red}✗{reset} zoning [{}]: no contract {dim}({} {} files ungoverned) \
             → zoning draft {}{reset}",
            basename(root, &parcel.dir),
            found.files.len(),
            parcel.language,
            parcel.dir
        );
    }
    Ok(clean)
}

fn report_fault(fault: &Fault, ink: Ink) {
    let Ink { red, reset, .. } = ink;
    eprintln!("{red}✗{reset} zoning: {fault}");
}
