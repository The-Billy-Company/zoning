use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use zoning::ordinance;
use zoning::report::Ink;
use zoning::survey;

use super::Tracked;
use super::args::Options;
use super::scope::{basename, declared, module, probe, tail};

/// Every package in the tree and whether it is governed, with the next command for the
/// ones that are not.
///
/// The question adoption actually asks. A list of the packages that *have* a contract
/// cannot tell you whether you are finished, and the ungoverned ones are why anybody
/// runs this — so they are the rows that carry a command.
pub(super) fn list(
    options: &Options,
    root: &Path,
    tracked: &mut Tracked<'_>,
) -> Result<ExitCode, String> {
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
