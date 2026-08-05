//! The inverse of judging: read a graph, write the contract it already obeys.
//!
//! Adoption is the hard part of any boundary tool. A package with nine hundred files
//! has an architecture already — it is simply undeclared — and the honest first
//! contract is the one that *describes* it. So this derives the stack from the real
//! dependency graph rather than proposing an ideal: zones come out of a topological
//! order over directories, seals out of the directories that are already fronted, and
//! grants out of the modules the code is already importing.
//!
//! That makes the first `verify` green, which is the point. A contract that arrives
//! red teaches its reader that the gate is noise; one that arrives green and is then
//! *tightened* — merge two zones, seal a directory, drop a grant — turns every step of
//! the cleanup into a decision somebody made on purpose.
//!
//! The one thing it cannot derive is a reason. Cycles are emitted as `variance`
//! stanzas with an empty reason, which does not parse, so a draft carrying real
//! tangles refuses to be adopted until a person has written why each one is there.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use crate::judge::{condensation, dir_of, tangles};
use crate::pattern::Globs;
use crate::survey::Survey;

/// A `.zone` contract describing what `survey` already does.
///
/// Deliberately silent about seals and keeps. Those are claims — "this directory is a
/// deep module", "these peers are independent" — and a machine inferring them from
/// today's call sites would be guessing at intent and would guess wrong the first time
/// somebody adds a legitimate second caller. `zone status` lists the directories that
/// *could* be sealed, which is the right place for a suggestion: a burndown queue a
/// person works through, not a line that arrives pre-decided.
#[must_use]
pub fn contract(survey: &Survey, package: &str, root: &str, nested: &[String]) -> String {
    let facade = facade(survey);
    let zones = stack(survey, package, facade.as_deref());
    let mut out = format!(
        "// {package} — import topology, drafted by `zone draft` from the real\n\
         // @import graph. Every line below is TRUE of the tree as it stands: this is\n\
         // a description, not a wish. Read it once, then start tightening it —\n\
         // merging two zones or sealing a directory is where the value is.\n\
         \n\
         package {package} {{\n\
         \x20   root     {root}\n\
         \x20   language {}\n",
        survey.dialect.name()
    );
    if let Some(face) = &facade {
        let _ = writeln!(out, "    facade   {face}");
    }
    if !nested.is_empty() {
        let _ = writeln!(
            out,
            "    exclude  {}\n\
             \x20   // ^ these declare their own package, so they are not this one's code",
            nested.join(" ")
        );
    }
    out.push_str("}\n\n// Zones, low to high: an import may not point up the page. The order is a\n\
                  // topological sort of what imports what today, so nothing points up yet.\nzones {\n");
    let width = zones.iter().map(|(name, _)| name.len()).max().unwrap_or(4);
    for (name, globs) in &zones {
        if globs.len() > 1 {
            let _ = writeln!(
                out,
                "    // these {} directories import each other, so no order separates them:\n\
                 \x20   // one zone until somebody untangles them, and then it splits.",
                globs.len()
            );
        }
        let _ = writeln!(out, "    {name:<width$}  {}", globs.join(" "));
    }
    out.push_str("}\n");

    let grants = grants(survey, &zones);
    if !grants.is_empty() {
        out.push_str("\n// Outside modules, and who carries them.\n");
        for (module, scopes) in &grants {
            let _ = writeln!(out, "use {module} by {}", scopes.join(" "));
        }
    }

    let hops = survey.edges.iter().map(|e| e.hops).max().unwrap_or(0);
    let _ = write!(
        out,
        "\n// Structural laws. The reach ceiling is what the tree needs today — lower it\n\
         // when a file moves nearer what it depends on; never raise it to go green.\n\
         limit  reach to {hops} hop{}\n\
         forbid cycles across directories\n",
        if hops == 1 { "" } else { "s" }
    );

    let facade_globs = Globs::new(facade.as_deref());
    let tangled = tangles(survey, &facade_globs);
    if tangled.is_empty() {
        out.push_str(
            "\n// Nothing else to declare: this graph is already a stack. `zone status`\n\
             // lists the directories that could be sealed next.\n",
        );
        return out;
    }

    // The one thing a draft cannot derive. A cycle is not fixed by zoning it — the two
    // directories genuinely depend on each other — so it needs either the untangling or
    // a person's sentence about why it stays. The empty reason does not parse, which is
    // how a draft with real tangles refuses to be adopted silently.
    let _ = write!(
        out,
        "\n// {} import cycle(s) cross a directory boundary. Zoning cannot describe these\n\
         // away — a cycle means the directories are one unit. Untangle them, or write the\n\
         // reason each one stays: these stanzas do NOT parse until you do.\n",
        tangled.len()
    );
    for component in &tangled {
        out.push_str("variance cycle {\n");
        for member in component {
            let _ = writeln!(out, "    {member}");
        }
        out.push_str("} because \"\"\n");
    }
    out
}

/// The module's public face, if one file obviously is it.
///
/// A facade may reach anywhere, so guessing wrong grants too much. The test is
/// deliberately narrow: a file at the module root, named for the package or by one of
/// the conventional entry names, that nothing inside the module imports.
fn facade(survey: &Survey) -> Option<String> {
    let extension = survey.dialect.extensions().first().copied().unwrap_or_default();
    let package = survey
        .module_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let imported: BTreeSet<&str> = survey.edges.iter().map(|e| e.dst.as_str()).collect();
    ["root", "lib", "main", "mod", package.as_str()]
        .into_iter()
        .map(|stem| format!("{stem}.{extension}"))
        .find(|name| survey.files.iter().any(|f| f == name) && !imported.contains(name.as_str()))
}

/// Directories in dependency order, bottom first, as (zone name, globs).
///
/// The unit is a directory because that is the unit a person already reorganises by,
/// and because a zone per file would be a graph rather than a stack. Directories that
/// depend on each other in a cycle are one zone: they cannot be ordered, and pretending
/// otherwise would emit a contract its own tree violates.
fn stack(survey: &Survey, package: &str, facade: Option<&str>) -> Vec<(String, Vec<String>)> {
    let mine = |file: &String| Some(file.as_str()) != facade;
    let mut homes: BTreeSet<&str> =
        survey.files.iter().filter(|f| mine(f)).map(|f| dir_of(f)).collect();
    homes.insert(".");
    let mut linked: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in survey.edges.iter().filter(|e| mine(&e.src)) {
        let (from, to) = (dir_of(&edge.src), dir_of(&edge.dst));
        if from != to {
            linked.entry(from).or_default().insert(to);
        }
    }
    // Tarjan wants owned adjacency lists; sorted, so the answer does not depend on the
    // order the survey happened to read the tree in.
    let adjacency: HashMap<&str, Vec<&str>> = homes
        .iter()
        .map(|dir| {
            let next = linked.get(dir).map(|to| to.iter().copied().collect()).unwrap_or_default();
            (*dir, next)
        })
        .collect();

    let groups: Vec<Vec<String>> = condensation(&adjacency)
        .into_iter()
        .filter(|group| group.iter().any(|dir| holds(survey, dir, facade)))
        .collect();

    // The package's own name belongs to the loose files at the module root and to nothing
    // else. Reserving it either way is what keeps `use outliner by outliner` — a module
    // name beside a zone name that happen to be the same word — from ever being written:
    // a `surface/face/outliner` directory becomes `face_outliner` instead, and if there
    // *are* root files, they get the word that was always right for them rather than
    // losing a race to whichever zone the topological order put first.
    let mut names: Vec<String> = vec![String::new(); groups.len()];
    let mut taken: BTreeSet<String> = BTreeSet::from([package.replace('-', "_")]);
    if let Some(at) = groups.iter().position(|g| g.as_slice() == ["."]) {
        names[at] = package.replace('-', "_");
    }
    for (name, group) in names.iter_mut().zip(&groups) {
        if name.is_empty() {
            *name = distinct(group, package, &mut taken);
        }
    }

    let extension = survey.dialect.extensions().first().copied().unwrap_or_default();
    names
        .into_iter()
        .zip(groups)
        .map(|(name, group)| {
            let globs = group
                .iter()
                .map(|dir| if dir == "." { format!("*.{extension}") } else { format!("{dir}/**") })
                .collect();
            (name, globs)
        })
        .collect()
}

/// Does this directory hold any file the contract would have to claim?
///
/// The module root is always a candidate zone — a package with loose files there needs
/// one — but a root holding nothing but the facade would otherwise get a zone whose glob
/// claims no file, and a zone claiming nothing is precisely what the staleness check
/// exists to delete.
fn holds(survey: &Survey, dir: &str, facade: Option<&str>) -> bool {
    survey.files.iter().any(|f| dir_of(f) == dir && Some(f.as_str()) != facade)
}

/// A name nothing else in this stack already answers to.
///
/// Several directories in one zone means they import each other, and a knot has no
/// honest single word: `folio_lex_quire_walk_press` is five names wearing one, and it
/// grows with the tangle. So it is called what it is. The comment above the row lists
/// the members, and a zone nobody enjoys reading is a zone somebody eventually splits —
/// which is the only outcome that helps.
///
/// A lone directory takes the word its author already chose. Two can share a last
/// segment — `surface/face/outliner` beside a package root also called `outliner` — and
/// two zones with one name is a contract that cannot be read, so the loser widens to its
/// parent segment (`face_outliner`) before anything as meaningless as a number appears.
fn distinct(group: &[String], package: &str, taken: &mut BTreeSet<String>) -> String {
    let name = match group {
        [one] => (1..=3)
            .map(|depth| name_for(one, package, depth))
            .find(|candidate| !taken.contains(candidate))
            .unwrap_or_else(|| free(&name_for(one, package, 3), taken)),
        _ => free("tangle", taken),
    };
    taken.insert(name.clone());
    name
}

/// `stem`, or the first `stem2`, `stem3`, … nothing answers to yet.
fn free(stem: &str, taken: &BTreeSet<String>) -> String {
    if !taken.contains(stem) {
        return stem.to_owned();
    }
    // Bounded by construction: there are at most `taken.len()` names to collide with, so
    // one of the first that many suffixes is free.
    (2..=taken.len() + 2)
        .map(|n| format!("{stem}{n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or_else(|| stem.to_owned())
}

/// A zone name for one directory, from its last `depth` path segments.
///
/// One segment is almost always right: it is the word the author already chose for the
/// region. Files loose at the module root have no such word, so they take the package's
/// own name — a zone called `core` would be a name this tool invented, and an invented
/// name in somebody's architecture document is a small lie.
fn name_for(dir: &str, package: &str, depth: usize) -> String {
    if dir == "." {
        return package.replace('-', "_");
    }
    let mut parts: Vec<&str> = dir.rsplit('/').take(depth).collect();
    parts.reverse();
    parts.join("_").replace(['.', '-'], "_")
}

/// Each non-ambient module, and the zones importing it.
fn grants(survey: &Survey, zones: &[(String, Vec<String>)]) -> Vec<(String, Vec<String>)> {
    let ambient = survey.dialect.ambient();
    let claims: Vec<(&str, Globs)> =
        zones.iter().map(|(name, globs)| (name.as_str(), Globs::new(globs))).collect();
    let mut wanted: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for outside in &survey.outside {
        if ambient.contains(&outside.spec.as_str()) {
            continue;
        }
        let zone =
            claims.iter().find(|(_, globs)| globs.matches(&outside.src)).map(|(name, _)| *name);
        wanted.entry(&outside.spec).or_default().extend(zone);
    }
    wanted
        .into_iter()
        .map(|(module, zones)| (module.to_owned(), zones.into_iter().map(str::to_owned).collect()))
        .collect()
}
