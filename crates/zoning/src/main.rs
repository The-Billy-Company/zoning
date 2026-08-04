//! The `zoning` command.
//!
//! Five verbs and a closed set of flags. `verify` is the default because that is
//! what CI runs; everything else exists so a person can understand a contract
//! before, or instead of, arguing with it.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zoning::judge::{self, Verdict};
use zoning::ordinance::{self, Fault, Ordinance};
use zoning::report::{self, Ink};
use zoning::survey::{self, Ask, Dialect, Survey};

const USAGE: &str = "\
zoning — declare where a package's imports may go, and judge the real graph.

USAGE
    zoning [VERB] [OPTIONS]

VERBS
    verify    judge every governed package (default)
    status    verify, plus the census: zone counts, hop histogram, burndown queue
    list      which packages are governed, and which are not
    show      the resolved contract, as zoning understood it
    map       the zone stack drawn high to low

OPTIONS
    --package NAME    only this package (repeatable)
    --under DIR       also discover <DIR>/*/contract/*.zone (repeatable, for a monorepo)
    --root PATH       repository root (default: the enclosing git worktree)
    --dialect NAME    language to read (default: zig)
    --untracked       judge files version control does not know about
    --suggest         print draft variances for today's violations, write nothing
    --json            one record per finding on stdout
    --no-color        never colour, even on a terminal
    -h, --help        this
    -V, --version     the version

EXIT
    0  every governed package passes
    1  at least one violation or stale declaration
    2  a contract is malformed, or the invocation is
";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(problem) => {
            eprintln!("zoning: {problem}");
            ExitCode::from(2)
        }
    }
}

struct Options {
    verb: Verb,
    packages: HashSet<String>,
    under: Vec<String>,
    root: Option<PathBuf>,
    dialect: &'static dyn Dialect,
    untracked: bool,
    suggest: bool,
    json: bool,
    ink: Ink,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verb {
    Verify,
    Status,
    List,
    Show,
    Map,
}

fn run() -> Result<ExitCode, String> {
    let Some(options) = parse(std::env::args().skip(1))? else {
        return Ok(ExitCode::SUCCESS);
    };

    let root = match &options.root {
        Some(given) => given.clone(),
        None => zoning::repo_root(Path::new(".")),
    };
    let contracts = ordinance::discover(&root, &options.under);
    if contracts.is_empty() {
        let Ink { green, reset, .. } = options.ink;
        println!("{green}✓{reset} zoning: no package declares a contract/*.zone file");
        return Ok(ExitCode::SUCCESS);
    }

    // One index read for the whole run, not one per package: a monorepo gate judges
    // several packages that all live in the same worktree.
    let tracked =
        if options.untracked { None } else { survey::tracked(&root, options.dialect.extensions()) };

    let mut failed = false;
    let mut verdicts: Vec<Verdict> = Vec::new();
    let mut governed: HashSet<String> = HashSet::new();
    let mut out = String::new();

    for path in &contracts {
        let contract = match Ordinance::read(path) {
            Ok(contract) => contract,
            Err(fault) => {
                report_fault(&fault, options.ink);
                failed = true;
                continue;
            }
        };
        governed.insert(contract.package.clone());
        if !options.packages.is_empty() && !options.packages.contains(&contract.package) {
            continue;
        }

        match options.verb {
            Verb::List => {
                let shown = path.strip_prefix(&root).unwrap_or(path);
                let _ = writeln!(out, "  governed    {:<12} {}", contract.package, shown.display());
                continue;
            }
            Verb::Show => {
                out.push_str(&report::show(&contract, options.ink));
                continue;
            }
            _ => {}
        }

        let found = Survey::of(&Ask {
            repo_root: &root,
            module_root: &contract.module_root,
            exclude: &contract.exclude,
            dialect: options.dialect,
            tracked: tracked.as_ref(),
        });

        if options.verb == Verb::Map {
            out.push_str(&report::map(&contract, &found, options.ink));
            continue;
        }

        let verdict = judge::judge(&found, &contract);
        failed |= !verdict.ok();
        if options.suggest {
            out.push_str(&report::suggest(&verdict));
        } else if !options.json {
            out.push_str(&report::verdict(&verdict, options.ink, options.verb == Verb::Status));
        }
        verdicts.push(verdict);
    }

    if options.json {
        out.push_str(&report::records(&verdicts));
    }
    print!("{out}");
    let _ = std::io::stdout().flush();

    Ok(if failed { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

fn report_fault(fault: &Fault, ink: Ink) {
    let Ink { red, reset, .. } = ink;
    eprintln!("{red}✗{reset} zoning: {fault}");
}

/// Parse argv. `Ok(None)` means the run is already over — help or version was asked
/// for, and answering a question is not an error.
fn parse(argv: impl Iterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options {
        verb: Verb::Verify,
        packages: HashSet::new(),
        under: Vec::new(),
        root: None,
        dialect: survey::dialect("zig").ok_or("the zig dialect is missing from this build")?,
        untracked: false,
        suggest: false,
        json: false,
        ink: if std::io::stdout().is_terminal() { Ink::LIT } else { Ink::PLAIN },
    };
    let mut seen_verb = false;
    let mut rest = argv.peekable();

    while let Some(arg) = rest.next() {
        let mut value = |flag: &str| -> Result<String, String> {
            rest.next().ok_or_else(|| format!("`{flag}` needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("zoning {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--package" => {
                options.packages.insert(value("--package")?);
            }
            "--under" => options.under.push(value("--under")?),
            "--root" => options.root = Some(PathBuf::from(value("--root")?)),
            "--dialect" => {
                let name = value("--dialect")?;
                options.dialect = survey::dialect(&name).ok_or_else(|| {
                    let known: Vec<&str> = survey::dialects().iter().map(|d| d.name()).collect();
                    format!("unknown dialect `{name}` — this build reads {}", known.join(", "))
                })?;
            }
            "--untracked" => options.untracked = true,
            "--suggest" => options.suggest = true,
            "--json" => {
                options.json = true;
                options.ink = Ink::PLAIN;
            }
            "--no-color" | "--no-colour" => options.ink = Ink::PLAIN,
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}` — try `zoning --help`"));
            }
            other if !seen_verb => {
                options.verb = match other {
                    "verify" => Verb::Verify,
                    "status" => Verb::Status,
                    "list" => Verb::List,
                    "show" => Verb::Show,
                    "map" => Verb::Map,
                    _ => {
                        return Err(format!(
                            "unknown verb `{other}` — try verify, status, list, show, or map"
                        ));
                    }
                };
                seen_verb = true;
            }
            other => return Err(format!("unexpected argument `{other}`")),
        }
    }
    Ok(Some(options))
}
