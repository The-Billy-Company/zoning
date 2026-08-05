use std::collections::HashSet;
use std::io::IsTerminal as _;
use std::path::PathBuf;

use zoning::Result;
use zoning::report::Ink;
use zoning::survey::{self, Dialect};

const USAGE: &str = "\
zoning — declare where a package's imports may go, and judge the real graph.

USAGE
    zoning [VERB] [ARGS] [OPTIONS]

VERBS
    verify              judge every governed package (default)
    status              verify, plus the census: zone counts, hops, burndown queue
    list                every package in the tree, governed or not
    show                the resolved contract, as zoning understood it
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
    --write             draft: create contract/<name>.zone if it does not exist
    --untracked         judge files version control does not know about
    --suggest           print the declarations that would make today's graph legal
    --json              one record per finding on stdout
    --no-color          never colour, even on a terminal
    -h, --help          this
    -V, --version       the version

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
                println!("zoning {}", env!("CARGO_PKG_VERSION"));
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
                return Err(format!("unknown option `{other}` — try `zoning --help`").into());
            }
            other if !seen_verb => {
                options.verb = match other {
                    "verify" => Verb::Verify,
                    "status" => Verb::Status,
                    "list" => Verb::List,
                    "show" => Verb::Show,
                    "map" => Verb::Map,
                    "explain" => Verb::Explain,
                    "draft" => Verb::Draft,
                    "lsp" => Verb::Lsp,
                    "setup" => Verb::Setup,
                    _ => {
                        return Err(format!(
                            "unknown verb `{other}` — try verify, status, list, show, map, \
                             explain, draft, lsp, or setup"
                        )
                        .into());
                    }
                };
                seen_verb = true;
            }
            other => options.args.push(other.to_owned()),
        }
    }

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
    Ok(Some(options))
}
