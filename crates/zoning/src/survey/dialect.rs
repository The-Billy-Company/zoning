// Copyright (c) 2026 The Billy Company
//! Language-owned source parsing produces one shared, located import graph.
//!
//! Zig paths are file names; Rust paths name items in a target's module tree.
//! Each reader resolves that spelling at the boundary. The laws see only files,
//! outside modules and escapes, never language-specific resolution rules.

use std::path::Path;

use crate::Result;

/// One import spelling, before lexical resolution.
pub struct Import {
    /// Byte offset of the statement in its original source.
    pub offset: usize,
    /// Its path or external module name.
    pub spec: String,
}

/// One admitted source file, read once for the entire survey.
pub struct Source {
    /// File name relative to the module root, with forward slashes.
    pub path: String,
    /// Original UTF-8 source, including comments and literals.
    pub text: String,
}

/// All the source context a language needs to resolve its import spellings.
pub struct Context<'a> {
    /// Inputs cached for this invocation, never across source edits.
    pub session: &'a super::Session,
    /// The absolute root the source paths hang off.
    pub module_root: &'a Path,
    /// The contract's package identity.
    pub package: &'a str,
    /// The sorted, admitted sources of this survey.
    pub sources: &'a [Source],
}

/// The resolved meaning of a source reference.
#[derive(Clone)]
pub enum Destination {
    /// A file inside the governed module.
    Local {
        /// Its module-relative name, with forward slashes.
        path: String,
        /// Parent steps in the written path, not the resolved file name.
        hops: u32,
    },
    /// A dependency named outside the governed module.
    Outside(String),
    /// A source path that crosses the governed module's boundary.
    Escape,
}

/// A language's located reference, ready for the shared graph.
pub struct Reference {
    /// The importing source's module-relative name.
    pub source: String,
    /// Byte offset in the original source.
    pub offset: usize,
    /// The language reader's reference spelling, retained for diagnostics.
    pub spec: String,
    /// Its resolved target and explicit reach.
    pub destination: Destination,
}

/// What varies between source languages; graph policy remains shared.
pub trait Dialect: Sync {
    /// The language selected by a contract and named in reports.
    fn name(&self) -> &'static str;

    /// Governed source extensions, without a leading dot.
    fn extensions(&self) -> &'static [&'static str];

    /// Package-declaring manifest names.
    fn manifests(&self) -> &'static [&'static str];

    /// Conventional source root for an otherwise undeclared package scope.
    fn source_root(&self, package: &Path) -> &'static str {
        if package.join("src").is_dir() { "src" } else { "." }
    }

    /// Dependency directories a manifest declares as vendored, relative to itself.
    fn vendored(&self, _manifest: &str) -> Vec<String> {
        Vec::new()
    }

    /// The identity declared by a package manifest.
    fn declared(&self, _manifest: &str) -> Option<String> {
        None
    }

    /// Modules supplied by the language rather than chosen by the package.
    fn ambient(&self) -> &'static [&'static str];

    /// Resolve references with complete source and target context available.
    ///
    /// # Errors
    /// Returns a located error when source parsing or resolution cannot establish
    /// a complete graph. Errors must not become a clean partial verdict.
    fn references(&self, context: &Context<'_>) -> Result<Vec<Reference>>;

    /// The language's remedy for a path that crosses the module root.
    fn escape_remedy(&self) -> &'static str;
}

/// Every dialect this build knows about.
#[must_use]
pub fn all() -> &'static [&'static dyn Dialect] {
    &[&super::zig::Zig, &super::python::Python, &super::rust::Rust]
}

/// The dialect named `name`, if there is one.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static dyn Dialect> {
    all().iter().copied().find(|d| d.name() == name)
}
