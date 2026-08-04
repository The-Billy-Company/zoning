//! The `zoning` command.
//!
//! Seven verbs and a closed set of flags. `verify` is the default and takes no
//! arguments on purpose — a gate you have to configure at the call site is a gate that
//! gets configured differently in CI than on a laptop. Everything else exists so a
//! person can understand a contract before, or instead of, arguing with it.

use std::collections::{HashMap, HashSet};
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

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(problem) => {
            eprintln!("zoning: {problem}");
            ExitCode::from(2)
        }
    }
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "argv is a bag of independent switches; grouping them would only hide that"
)]
struct Options {
    verb: Verb,
    args: Vec<String>,
    packages: HashSet<String>,
    under: Vec<String>,
    root: Option<PathBuf>,
    language: &'static dyn Dialect,
    complete: bool,
    write: bool,
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
    Explain,
    Draft,
}

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

fn run() -> Result<ExitCode, String> {
    let Some(options) = parse(std::env::args().skip(1))? else {
        return Ok(ExitCode::SUCCESS);
    };
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
        Verb::Draft => return draft(&options, &mut tracked),
        Verb::List => return list(&options, &root, &mut tracked),
        Verb::Explain => return explain(&options, &root, &mut tracked),
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
) -> Result<bool, String> {
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

/// Which subtree this run governs, and every contract in it.
///
/// The scope is where you are standing. A gate that answers about the whole repository
/// no matter which directory you invoke it from cannot be used *inside* one package —
/// and in a monorepo it is also the slow answer, since it reads every other package to
/// tell you about yours. At a repository root, which is where CI stands, the two models
/// are the same run.
///
/// Two completions keep that from ever being a surprise. A directory with nothing
/// governed beneath it looks *up* for the package that encloses it, so a question asked
/// from `src/kernel/regex` is still answered by the contract that governs it. And an
/// explicit `--root` never climbs: a caller who names the subtree means it, including
/// when the answer is "nothing here".
///
/// The climb stops at the worktree, and that boundary is load-bearing rather than
/// tidy: above it lies a directory git cannot enumerate, where discovery falls back to
/// reading the filesystem — and a tool that walked out of the repository and up toward
/// `/` looking for a contract would not be slow, it would be hung.
fn scope(here: &Path, pinned: bool, under: &[String]) -> (PathBuf, Vec<PathBuf>) {
    let here = here.canonicalize().unwrap_or_else(|_| here.to_path_buf());
    let found = ordinance::discover(&here, under);
    if pinned || !found.is_empty() {
        return (here, found);
    }
    let ceiling = zoning::repo_root(&here);
    for parent in here.ancestors().skip(1).take_while(|p| p.starts_with(&ceiling)) {
        let contracts = ordinance::discover(parent, under);
        if !contracts.is_empty() {
            return (parent.to_path_buf(), contracts);
        }
    }
    (here, Vec::new())
}

/// What to call a package whose directory is `rel`, for a column of names.
///
/// A package at the root of its own repository has `.` for a directory, and a column of
/// dots names nothing — so the enclosing directory answers for it, which is also the
/// name it will have when somebody drafts its contract.
fn basename(root: &Path, rel: &str) -> String {
    let dir = if rel == "." { root.to_path_buf() } else { root.join(rel) };
    dir.file_name().map_or_else(|| rel.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// A path as short as it can be while still saying where it is: relative to the shell's
/// own directory when it lies under it, its last two components otherwise.
fn tail(path: &Path) -> String {
    if let Ok(here) = std::env::current_dir() {
        if path == here {
            return ".".to_owned();
        }
        if let Ok(rel) = path.strip_prefix(&here) {
            return rel.display().to_string();
        }
    }
    let mut last: Vec<_> = path.components().rev().take(2).collect();
    last.reverse();
    last.iter().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// One file's standing, or one edge's verdict.
///
/// Paths are taken as typed, resolved against the directory the shell is in — the way
/// every other tool takes a path, and the way a person reading a stack trace or an
/// editor tab has it. Making somebody translate into module-relative coordinates first
/// is the friction that stops a diagnostic verb from being used at all.
fn explain(options: &Options, root: &Path, tracked: &mut Tracked<'_>) -> Result<ExitCode, String> {
    let here =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    let wanted: Vec<PathBuf> = options
        .args
        .iter()
        .map(|arg| here.join(arg).canonicalize().map_err(|_| format!("no such file: `{arg}`")))
        .collect::<Result<_, _>>()?;

    let mut best: Option<Ordinance> = None;
    for path in ordinance::discover(root, &options.under) {
        let contract = Ordinance::read(&path, options.language).map_err(|f| f.to_string())?;
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
        ));
    };

    let found = Survey::of(&Ask {
        repo_root: root,
        module_root: &contract.module_root,
        exclude: &contract.exclude,
        dialect: contract.dialect,
        tracked: tracked.of(contract.dialect),
    });
    let named: Vec<String> = wanted
        .iter()
        .map(|path| judged(path, &contract, &found, tracked))
        .collect::<Result<_, _>>()?;

    let answer = match named.as_slice() {
        [one] => report::file(one, &contract, &found, &options.ink),
        [from, to] => report::edge(from, to, &contract, &found, &options.ink),
        _ => return Err("`explain` takes one file, or two".to_owned()),
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
) -> Result<String, String> {
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
    Err(format!("{} holds `{rel}` but does not judge it: {why}", contract.package))
}

/// Every package in the tree and whether it is governed, with the next command for the
/// ones that are not.
///
/// The question adoption actually asks. A list of the packages that *have* a contract
/// cannot tell you whether you are finished, and the ungoverned ones are why anybody
/// runs this — so they are the rows that carry a command.
fn list(options: &Options, root: &Path, tracked: &mut Tracked<'_>) -> Result<ExitCode, String> {
    let Ink { green, yellow, dim, reset, .. } = options.ink;
    let parcels = ordinance::parcels(root, &options.under);
    let mut out = String::new();
    let (mut governed, mut open) = (0, 0);

    for parcel in &parcels {
        if parcel.contracts.is_empty() {
            // Somebody else's package, said so by the manifest that vendored it. Its
            // architecture is decided upstream, so it is neither a gap nor this
            // repository's to close — but silence would read as an oversight.
            if let Some(owner) = &parcel.vendored_by {
                let _ = writeln!(
                    out,
                    "  {dim}vendored    {:<14} {} (a dependency of {owner}, governed \
                     upstream){reset}",
                    basename(root, &parcel.dir),
                    parcel.dir
                );
                continue;
            }
            let dialect = survey::dialect(parcel.language)
                .ok_or_else(|| format!("no dialect named `{}`", parcel.language))?;
            let dir = root.join(&parcel.dir);
            let (source, inside) = module(&dir);
            let found = probe(&dir, source, dialect, tracked, &inside);
            // The name a draft will actually give it, so the two agree before anybody has
            // run one — a listing that calls the package by its directory and a contract
            // that calls it by its manifest is one more thing to reconcile by hand.
            let name = declared(&dir, dialect).unwrap_or_else(|| basename(root, &parcel.dir));
            // A package with no source of its own is not a gap in coverage. Build-time
            // chassis packages exist, and so do trees whose only content is a nested
            // dependency — telling somebody to draft a contract for either is telling
            // them to write a file that would govern nothing.
            if found.files.is_empty() {
                let _ = writeln!(
                    out,
                    "  {dim}no module   {name:<14} {} (declares a package, holds no \
                     {} source of its own){reset}",
                    parcel.dir, parcel.language
                );
                continue;
            }
            open += 1;
            // Five of the seven laws are claims about how a module's files sit relative to
            // each other, so a one-file module has almost nothing for a contract to say.
            // Saying that up front is cheaper than letting somebody draft it and wonder
            // why the zone stack is one line long.
            let worth = if found.files.len() == 1 {
                " — one file, so only `use` and `escape` bind".to_owned()
            } else {
                format!(", {} files", found.files.len())
            };
            let _ = writeln!(
                out,
                "  {yellow}ungoverned{reset}  {name:<14} {} {dim}({}{worth}){reset}\n\
                 \x20                            {dim}→ zoning draft {}{reset}",
                parcel.dir, parcel.language, parcel.dir
            );
            continue;
        }
        for path in &parcel.contracts {
            governed += 1;
            let shown = path.strip_prefix(root).unwrap_or(path);
            let name =
                path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            let _ = writeln!(out, "  {green}governed{reset}    {name:<14} {}", shown.display());
        }
    }

    if parcels.is_empty() {
        let known: Vec<String> = survey::dialects()
            .iter()
            .map(|d| format!("{} for {}", d.manifests().join(" or "), d.name()))
            .collect();
        let _ = writeln!(
            out,
            "  {dim}no package manifest anywhere under {} — zoning finds a package by the\n\
             \x20 file that declares one ({}){reset}",
            tail(root),
            known.join("; ")
        );
    }
    print!("{out}");
    if open > 0 {
        println!("\n  {governed} governed, {yellow}{open} ungoverned{reset}");
    }
    let _ = std::io::stdout().flush();
    Ok(ExitCode::SUCCESS)
}

/// A contract describing what a directory's graph already does.
///
/// The directory is taken as typed, from the shell's own working directory — `draft .`
/// means here, whatever subtree a verify would have chosen to read.
fn draft(options: &Options, tracked: &mut Tracked<'_>) -> Result<ExitCode, String> {
    let here =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    let target = options.args.first().map_or(".", String::as_str);
    let dir = here.join(target).canonicalize().map_err(|e| format!("`{target}`: {e}"))?;
    let declares = |at: &Path| options.language.manifests().iter().any(|m| at.join(m).exists());
    // `draft src` is the natural mistake, because `src` is where the code is. It is also
    // the one mistake that succeeds quietly: a package named after the source directory,
    // its contract filed one level too deep, and every later `--package` invocation
    // spelled wrong. The package is the directory somebody declared, so say so.
    if let Some(parent) = dir.parent().filter(|p| !declares(&dir) && declares(p)) {
        let up = pathdiff(&here, parent);
        return Err(format!(
            "`{target}` is where the code lives, not where the package is declared — `{}` is, \
             and `root {target}` inside its contract is how that gets said. Draft the package: \
             `zoning draft {up}`",
            tail(parent)
        ));
    }
    let name = declared(&dir, options.language)
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .ok_or_else(|| format!("`{target}` has no directory name to take the package name from"))?;

    let (source, nested) = module(&dir);
    let held = ordinance::parcels(&dir, &[]);
    if !declares(&dir) && !held.is_empty() {
        let names: Vec<&str> = held.iter().map(|p| p.dir.as_str()).collect();
        return Err(format!(
            "{name} is not a package — it holds {}: {}. Draft each of them: a contract \
             governs one module's imports, and these do not share a module",
            names.len(),
            names.join(", ")
        ));
    }
    let found = probe(&dir, source, options.language, tracked, &nested);
    if found.files.is_empty() {
        return Err(barren(&dir, source, options.language, &nested));
    }
    let text = zoning::draft::contract(&found, &name, source, &nested);

    if !options.write {
        print!("{text}");
        let _ = std::io::stdout().flush();
        return Ok(ExitCode::SUCCESS);
    }
    let home = dir.join("contract");
    let file = home.join(format!("{name}.zone"));
    if file.exists() {
        return Err(format!(
            "{} already exists — a draft never overwrites a contract somebody wrote; \
             drop `--write` to read the draft instead",
            file.display()
        ));
    }
    std::fs::create_dir_all(&home).map_err(|e| format!("{}: {e}", home.display()))?;
    std::fs::write(&file, &text).map_err(|e| format!("{}: {e}", file.display()))?;
    let Ink { green, reset, dim, .. } = options.ink;
    println!(
        "{green}✓{reset} wrote {} {dim}— now run `zoning verify --package {name}`{reset}",
        file.display()
    );
    Ok(ExitCode::SUCCESS)
}

/// The name this package's own manifest gives it, if it gives it one.
fn declared(dir: &Path, dialect: &'static dyn Dialect) -> Option<String> {
    dialect
        .manifests()
        .iter()
        .filter_map(|m| std::fs::read_to_string(dir.join(m)).ok())
        .find_map(|text| dialect.declared(&text))
        .filter(|name| !name.is_empty())
}

/// `to`, spelled the way you would have to type it from `from`.
///
/// Only ever one level up here, so this is the difference between a hint you can paste
/// and a hint you have to translate.
fn pathdiff(from: &Path, to: &Path) -> String {
    if to == from {
        return ".".to_owned();
    }
    match to.strip_prefix(from) {
        Ok(rel) => rel.display().to_string(),
        Err(_) if from.parent() == Some(to) => "..".to_owned(),
        Err(_) => to.display().to_string(),
    }
}

/// Why a draft found no module, in the terms that decide what to do about it.
///
/// Three different situations arrive here and they call for opposite next moves, so a
/// single message covering all of them would be wrong twice. A directory holding a
/// manifest and nothing else is complete as it stands. A directory whose source is all
/// nested packages wants each of those drafted. And a directory with source this build
/// cannot read is a dialect problem, where naming a manifest nobody wrote — `build.zig`,
/// in a Rust tree — sends the reader looking for a file that was never the issue.
fn barren(dir: &Path, source: &str, dialect: &'static dyn Dialect, nested: &[String]) -> String {
    // `tail` renders the shell's own directory as `.`, which is right in a path column and
    // reads badly mid-sentence, so the place carries its own preposition.
    let shown = tail(&dir.join(source));
    let where_ = if shown == "." { "here".to_owned() } else { format!("under {shown}") };
    let lang = dialect.name();
    if !nested.is_empty() {
        return format!(
            "no {lang} source {where_} of its own — every file belongs to a nested package \
             ({}). Draft each of those instead",
            nested.join(", ")
        );
    }
    if dialect.manifests().iter().any(|m| dir.join(m).exists()) {
        return format!(
            "no {lang} source {where_}. A contract governs a module's imports, and a manifest \
             is not a module: `{}` declares this package rather than belonging to it, so there \
             is nothing left here to govern",
            dialect.manifests().join("` / `")
        );
    }
    let known: Vec<&str> = zoning::survey::dialects().iter().map(|d| d.name()).collect();
    format!(
        "no {lang} source {where_}. If the code is in another language, `--language NAME` \
         reads it — this build knows {}",
        known.join(", ")
    )
}

/// Survey a package directory that may have no contract yet.
fn probe(
    dir: &Path,
    source: &str,
    dialect: &'static dyn Dialect,
    tracked: &mut Tracked<'_>,
    nested: &[String],
) -> Survey {
    let exclude: Vec<zoning::pattern::Pattern> =
        nested.iter().map(|glob| zoning::pattern::Pattern::new(glob)).collect();
    Survey::of(&Ask {
        repo_root: dir,
        module_root: &dir.join(source),
        exclude: &exclude,
        dialect,
        tracked: tracked.of(dialect),
    })
}

/// Where a package's module lives, and globs for the packages nested inside it.
///
/// `src/` when there is one, the directory itself otherwise — the convention the `root`
/// setting defaults to, so a draft and the contract it becomes read the same tree.
///
/// A vendored dependency with its own `build.zig` is a different package that happens to
/// sit in this directory tree. Judging its files as this package's would blame it for an
/// architecture it never agreed to, and would put a second package's directories in this
/// one's zone stack — so they are excluded, and the enclosing package keeps only the
/// dependency it genuinely has: the module, declared with `use`.
fn module(dir: &Path) -> (&'static str, Vec<String>) {
    let source = if dir.join("src").is_dir() { "src" } else { "." };
    let inside = if source == "." { String::new() } else { format!("{source}/") };
    let nested = ordinance::parcels(dir, &[])
        .into_iter()
        .filter(|p| p.dir != ".")
        .filter_map(|p| p.dir.strip_prefix(&inside).map(|within| format!("{within}/**")))
        .collect();
    (source, nested)
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
                    "explain" => Verb::Explain,
                    "draft" => Verb::Draft,
                    _ => {
                        return Err(format!(
                            "unknown verb `{other}` — try verify, status, list, show, map, \
                             explain, or draft"
                        ));
                    }
                };
                seen_verb = true;
            }
            other => options.args.push(other.to_owned()),
        }
    }

    match (options.verb, options.args.len()) {
        (Verb::Explain, 1 | 2) | (Verb::Draft, 0 | 1) => {}
        (Verb::Explain, _) => {
            return Err(
                "`explain` takes one file, or two to judge the import between them".to_owned()
            );
        }
        (Verb::Draft, _) => return Err("`draft` takes one directory".to_owned()),
        (_, 0) => {}
        _ => {
            return Err(format!(
                "unexpected argument `{}` — only `explain` and `draft` take arguments",
                options.args.join(" ")
            ));
        }
    }
    Ok(Some(options))
}
