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

    /// Comment and literal conventions, so imports are read from code alone.
    fn prose(&self) -> &Prose;

    /// Every import written in `source`, in the order they appear.
    ///
    /// `code` is `source` with prose blanked and byte offsets preserved; find the
    /// statement there and read its argument out of `source` at the same offset.
    fn imports(&self, source: &str, code: &[u8]) -> Vec<Import>;

    /// Does this spec name a file inside the module, rather than an external
    /// dependency the build system resolves?
    fn is_local(&self, spec: &str) -> bool;

    /// What to tell an author whose import climbed out of the module root.
    fn escape_remedy(&self) -> &'static str;
}

/// Every dialect this build knows about.
#[must_use]
pub fn all() -> &'static [&'static dyn Dialect] {
    &[&super::zig::Zig]
}

/// The dialect named `name`, if there is one.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static dyn Dialect> {
    all().iter().copied().find(|d| d.name() == name)
}
