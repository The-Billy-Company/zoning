//! What varies between languages, and nothing else.
//!
//! The interesting part of this tool — zones, seals, keeps, cycles, reach — is
//! about a graph of files, and a graph of files is the same shape in every
//! language. What actually differs is narrow: which files are source, how an import
//! is spelled, and whether a given import names a path inside the module or a
//! dependency outside it. That is the whole trait.
//!
//! Resolution is deliberately *not* here. Turning `../a/b` into a module-relative
//! path is arithmetic on a path, and a dialect that could do it its own way is a
//! dialect that could disagree with the others about what a cycle is.

use super::prose::Prose;

/// One import statement, located.
pub struct Import {
    /// Byte offset of the statement in the original source, for the line number.
    pub offset: usize,
    /// The path or module name exactly as written.
    pub spec: String,
}

/// How one language spells the things this tool needs to read.
pub trait Dialect: Sync {
    /// The name a report uses, and the name a contract selects it by.
    fn name(&self) -> &'static str;

    /// File extensions this dialect judges, without the leading dot.
    fn extensions(&self) -> &'static [&'static str];

    /// Filenames that mark a directory as a package root in this language.
    ///
    /// Only [`list`](crate::ordinance::packages) reads these, to answer which
    /// packages exist at all — the question a governed-only report cannot answer,
    /// and the one that says whether adoption is finished.
    fn manifests(&self) -> &'static [&'static str];

    /// Directories a manifest's own text declares as dependencies living in-tree.
    ///
    /// A vendored dependency is a package by every test this tool can run — it has a
    /// manifest, source, and an import graph — and it is nonetheless not yours: its
    /// architecture is decided in the repository it came from, which is where its
    /// contract lives. The manifest already knows the difference, because a build that
    /// did not declare the path dependency would not link, so reading it is how
    /// coverage tells "we have not written this contract yet" from "this is somebody
    /// else's package" without anybody maintaining a list of exceptions.
    ///
    /// Paths are relative to the manifest's own directory.
    fn vendored(&self, _manifest: &str) -> Vec<String> {
        Vec::new()
    }

    /// The name a manifest's own text gives the package it declares.
    ///
    /// A package usually already has a name, and it is not the directory's: the build
    /// system was told one, everything downstream depends on it by that name, and a
    /// directory can be renamed without the package being. So a drafted contract takes
    /// the declared name when there is one, which keeps `--package NAME` meaning the
    /// same thing in a contract as it does in a build file.
    fn declared(&self, _manifest: &str) -> Option<String> {
        None
    }

    /// Modules the language always provides, which no contract need grant.
    ///
    /// The standard library is not an architectural dependency: every zone has it,
    /// no zone chose it, and a contract forced to declare it would spend its `use`
    /// lines on noise instead of on the handful of dependencies that are decisions.
    fn ambient(&self) -> &'static [&'static str];

    /// Comment and literal conventions, so imports are read from code alone.
    fn prose(&self) -> &Prose;

    /// Every import written in `source`, in the order they appear.
    ///
    /// `path` is the importing file's own module-relative path, and `roots` is every
    /// top-level name this survey judges as local — the first path segment of a
    /// nested file, or the bare stem of one loose at the module root. `own` is the
    /// package's own declared name (empty when it has none). None of the three is
    /// needed by a dialect whose import spelling is already a path relative to
    /// itself; all exist for a dialect whose spelling is a dotted or absolute module
    /// name: `path` gives the importer's own depth, to turn a climb into `../`;
    /// `roots` tells its own package from an external one sharing the same leading
    /// word; and `own` catches the case `roots` cannot — a flat-laid package whose
    /// files address their own top level the way an installed consumer would
    /// (`import acme.contracts` from inside `acme` itself, when `acme` is not a
    /// subdirectory of anything but its own module root).
    ///
    /// `code` is `source` with prose blanked and byte offsets preserved; find the
    /// statement there and read its argument out of `source` at the same offset.
    fn imports(
        &self,
        path: &str,
        roots: &[&str],
        own: &str,
        source: &str,
        code: &[u8],
    ) -> Vec<Import>;

    /// Does this spec name a file inside the module, rather than an external
    /// dependency the build system resolves?
    fn is_local(&self, spec: &str) -> bool;

    /// What to tell an author whose import climbed out of the module root.
    fn escape_remedy(&self) -> &'static str;
}

/// Every dialect this build knows about.
#[must_use]
pub fn all() -> &'static [&'static dyn Dialect] {
    &[&super::zig::Zig, &super::python::Python]
}

/// The dialect named `name`, if there is one.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static dyn Dialect> {
    all().iter().copied().find(|d| d.name() == name)
}
