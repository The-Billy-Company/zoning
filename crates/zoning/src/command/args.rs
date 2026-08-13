use std::collections::HashSet;
use std::io::IsTerminal as _;
use std::path::PathBuf;

use zoning::Result;
use zoning::report::Ink;
use zoning::survey::{self, Dialect};

const USAGE: &str = "\
zone — declare where a package's imports may go, and judge the real graph.

USAGE
    zone [VERB] [ARGS] [OPTIONS]

VERBS
    verify              judge every governed package (default)
    status              verify, plus the census: zone counts, hops, burndown queue
    list                every package in the tree, governed or not
    show                the resolved contract, as zone understood it
    map                 the zone stack drawn high to low
    explain FILE        where one file stands: zone, reach, grants, who imports it
    explain FROM TO     whether that import is allowed, and the clause that decides
    draft DIR           a contract describing what DIR's graph already does
    lsp --stdio         serve editor language intelligence over standard I/O
    setup ACTION        status, run, repair, or uninstall editor integrations

OPTIONS
    --package NAME      only this package (repeatable)
    --under DIR         only contracts under DIR (repeatable; default: all of them)
    --root PATH         the subtree to govern (default: here, or the package enclosing it)
    --language NAME     language for packages that do not name one (default: zig)
    --complete          verify: also fail if a package in scope has no contract
    --write             draft: create <name>.zone at the package root if none governs it
    --untracked         judge files version control does not know about
    --suggest           print the declarations that would make today's graph legal
    --json              the machine form: one record per finding from verify/status,
                        the whole import graph from map, one object from explain
    --no-color          never colour, even on a terminal
    -h, --help          this
    -V, --version       the version

CONTRACTS
    pkg/<name>.zone     a contract sits at the root of what it organizes, beside the
                        manifest (a file in pkg/contract/ still governs pkg/ too)
    workspace { … }     a file above them claims members and says the shared settings
                        once — language, root, facade, use, limit reach — so each
                        member's contract holds only what makes it different

EXIT
    0  every governed package passes
    1  at least one violation or stale declaration
    2  a contract is malformed, or the invocation is
";

#[allow(
    clippy::struct_excessive_bools,
    reason = "argv is a bag of independent switches; grouping them would only hide that"
)]
pub(super) struct Options {
    pub(super) verb: Verb,
    pub(super) args: Vec<String>,
    pub(super) packages: HashSet<String>,
    pub(super) under: Vec<String>,
    pub(super) root: Option<PathBuf>,
    pub(super) language: &'static dyn Dialect,
    pub(super) complete: bool,
    pub(super) write: bool,
    pub(super) untracked: bool,
    pub(super) suggest: bool,
    pub(super) json: bool,
    stdio: bool,
    pub(super) ink: Ink,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Verb {
    Verify,
    Status,
    List,
    Show,
    Map,
    Explain,
    Draft,
    Lsp,
    Setup,
}

/// Parse argv. `Ok(None)` means the run is already over — help or version was asked
/// for, and answering a question is not an error.
pub(super) fn parse(argv: impl Iterator<Item = String>) -> Result<Option<Options>> {
    let mut options = Options {
        verb: Verb::Verify,
        args: Vec::new(),
        packages: HashSet::new(),
        under: Vec::new(),
        root: None,
        language: survey::dialect("zig").ok_or("the zig dialect is missing from this build")?,
        complete: false,
        write: false,
        untracked: false,
        suggest: false,
        json: false,
        stdio: false,
        ink: if std::io::stdout().is_terminal() { Ink::LIT } else { Ink::PLAIN },
    };
    let mut seen_verb = false;
    let mut rest = argv.peekable();

    while let Some(arg) = rest.next() {
        let mut value = |flag: &str| -> Result<String> {
            rest.next().ok_or_else(|| format!("`{flag}` needs a value").into())
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("zone {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--package" => {
                options.packages.insert(value("--package")?);
            }
            "--under" => options.under.push(value("--under")?),
            "--root" => options.root = Some(PathBuf::from(value("--root")?)),
            "--language" | "--dialect" => {
                let name = value("--language")?;
                options.language = survey::dialect(&name).ok_or_else(|| {
                    let known: Vec<&str> = survey::dialects().iter().map(|d| d.name()).collect();
                    format!("no language named `{name}` — this build reads {}", known.join(", "))
                })?;
            }
            "--complete" => options.complete = true,
            "--write" => options.write = true,
            "--untracked" => options.untracked = true,
            "--suggest" => options.suggest = true,
            "--stdio" => options.stdio = true,
            "--json" => {
                options.json = true;
                options.ink = Ink::PLAIN;
            }
            "--no-color" | "--no-colour" => options.ink = Ink::PLAIN,
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}` — try `zone --help`").into());
            }
            other if !seen_verb => {
                options.verb = verb(other)?;
                seen_verb = true;
            }
            other => options.args.push(other.to_owned()),
        }
    }

    // A flag that is quietly ignored is worse than one that is refused: a script asking
    // for JSON and getting prose has no way to tell, so it parses a report as data and
    // is wrong somewhere else entirely. Only the verbs with a machine form accept it.
    if options.json
        && !matches!(options.verb, Verb::Verify | Verb::Status | Verb::Map | Verb::Explain)
    {
        return Err("`--json` is the machine form of a verdict, a graph, or an explanation \
                    — `verify`, `status`, `map`, and `explain` answer in it"
            .into());
    }

    arity(&options)?;
    Ok(Some(options))
}

/// The verb by name, or the roster it was measured against.
fn verb(name: &str) -> Result<Verb> {
    match name {
        "verify" => Ok(Verb::Verify),
        "status" => Ok(Verb::Status),
        "list" => Ok(Verb::List),
        "show" => Ok(Verb::Show),
        "map" => Ok(Verb::Map),
        "explain" => Ok(Verb::Explain),
        "draft" => Ok(Verb::Draft),
        "lsp" => Ok(Verb::Lsp),
        "setup" => Ok(Verb::Setup),
        _ => Err(format!(
            "unknown verb `{name}` — try verify, status, list, show, map, explain, draft, \
             lsp, or setup"
        )
        .into()),
    }
}

/// Whether this verb was given the operands it takes — refused here rather than
/// half-run, since a verb that ignores an extra argument silently answers about
/// something other than what was asked.
fn arity(options: &Options) -> Result<()> {
    match (options.verb, options.args.len()) {
        (Verb::Explain, 1 | 2) | (Verb::Draft, 0 | 1) | (Verb::Lsp, 0)
            if options.verb != Verb::Lsp || options.stdio => {}
        (Verb::Setup, 1) => {}
        (Verb::Explain, _) => {
            return Err("`explain` takes one file, or two to judge the import between them".into());
        }
        (Verb::Draft, _) => return Err("`draft` takes one directory".into()),
        (Verb::Lsp, _) => return Err("`lsp` requires `--stdio`".into()),
        (Verb::Setup, _) => return Err("`setup` takes one action".into()),
        (_, 0) => {}
        _ => {
            return Err(format!(
                "unexpected argument `{}` — only `explain`, `draft`, and `setup` take arguments",
                options.args.join(" ")
            )
            .into());
        }
    }
    Ok(())
}
