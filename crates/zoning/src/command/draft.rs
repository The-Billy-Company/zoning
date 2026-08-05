use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use zoning::ordinance;
use zoning::report::Ink;

use super::Tracked;
use super::args::Options;
use super::scope::{barren, declared, module, pathdiff, probe, tail};

/// A contract describing what a directory's graph already does.
///
/// The directory is taken as typed, from the shell's own working directory — `draft .`
/// means here, whatever subtree a verify would have chosen to read.
pub(super) fn draft(options: &Options, tracked: &mut Tracked<'_>) -> Result<ExitCode, String> {
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
