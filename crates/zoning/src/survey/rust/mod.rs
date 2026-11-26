// Copyright (c) 2026 The Billy Company
//! Rust's written dependency graph, with Cargo targets and module ownership kept separate.
//!
//! `mod` introduces a namespace; it is not an import edge. References through that
//! namespace, reexports, and extern-prelude names become the same graph the other
//! dialects judge. Parsing deliberately precedes name resolution so malformed Rust
//! cannot turn into an empty, apparently clean graph.

mod cargo;
mod resolve;

pub(crate) use cargo::Cache;

use crate::Result;
use crate::survey::{Context, Dialect, Reference};
use std::path::Path;

/// Rust source and Cargo packages.
pub(super) struct Rust;

impl Dialect for Rust {
    fn name(&self) -> &'static str {
        "rust"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }
    fn manifests(&self) -> &'static [&'static str] {
        &["Cargo.toml"]
    }
    fn source_root(&self, _package: &Path) -> &'static str {
        "."
    }
    fn ambient(&self) -> &'static [&'static str] {
        &["std", "core", "alloc", "proc_macro", "test"]
    }
    fn declared(&self, manifest: &str) -> Option<String> {
        cargo::Manifest::parse(manifest)?.name()
    }
    fn references(&self, context: &Context<'_>) -> Result<Vec<Reference>> {
        resolve::references(context)
    }
    fn escape_remedy(&self) -> &'static str {
        "keep Rust source inside the package; share another crate through a declared Cargo dependency"
    }
}

#[cfg(test)]
mod tests {
    use super::Rust;
    use crate::survey::{Context, Destination, Dialect, Session, Source};
    use std::path::Path;

    fn sources(files: &[(&str, &str)]) -> Vec<Source> {
        files
            .iter()
            .map(|(path, text)| Source { path: (*path).to_owned(), text: (*text).to_owned() })
            .collect()
    }

    #[test]
    fn raw_identifiers_and_attribute_macro_paths_keep_their_semantic_names() {
        let sources = sources(&[
            ("lib.rs", "mod r#type; mod api;"),
            ("type.rs", "pub struct Value;"),
            ("api.rs", "use crate::r#type::Value; #[derive(crate::r#type::Trait)] struct Example;"),
        ]);
        let session = Session::default();
        let context = Context {
            session: &session,
            module_root: Path::new("/__zoning_rust_unit__"),
            package: "sample",
            sources: &sources,
        };
        let references = Rust
            .references(&context)
            .unwrap_or_else(|error| panic!("raw identifier fixture failed: {error}"));
        assert!(references.iter().any(|reference| reference.source == "api.rs" && matches!(&reference.destination, Destination::Local { path, .. } if path == "type.rs")));
        assert!(references.iter().any(|reference| reference.spec == "crate::type::Trait" && matches!(&reference.destination, Destination::Local { path, .. } if path == "type.rs")));
    }

    #[test]
    fn associated_item_reexports_keep_their_doorway_and_enum_globs_do_not_recurse() {
        let sources = sources(&[
            ("lib.rs", "mod store; mod api;"),
            ("store/mod.rs", "mod internal; pub use internal::Thing as T;"),
            (
                "store/internal.rs",
                "pub enum Thing { Value } #[cfg(test)] mod tests { use super::*; use Thing::*; fn check() { let _ = Thing::Value; } }",
            ),
            ("api.rs", "use crate::store::T; fn check() { let _ = T::Value; }"),
        ]);
        let session = Session::default();
        let context = Context {
            session: &session,
            module_root: Path::new("/__zoning_rust_unit__"),
            package: "sample",
            sources: &sources,
        };
        let references = Rust
            .references(&context)
            .unwrap_or_else(|error| panic!("reexport fixture failed: {error}"));
        assert!(references.iter().any(|reference| reference.source == "api.rs" && matches!(&reference.destination, Destination::Local { path, .. } if path == "store/mod.rs")));
        assert!(!references.iter().any(|reference| reference.source == "api.rs" && matches!(&reference.destination, Destination::Local { path, .. } if path == "store/internal.rs")));
    }

    #[test]
    fn a_reexported_inline_namespace_keeps_its_doorway_for_direct_members() {
        let sources = sources(&[
            ("lib.rs", "mod store; mod api;"),
            ("store/mod.rs", "mod internal; pub use internal::flags;"),
            ("store/internal.rs", "pub mod flags { pub const ACTIVE: u8 = 1; }"),
            (
                "api.rs",
                "use crate::store::flags; fn check() -> u8 { flags::ACTIVE + crate::store::flags::ACTIVE }",
            ),
        ]);
        let session = Session::default();
        let context = Context {
            session: &session,
            module_root: Path::new("/__zoning_rust_unit__"),
            package: "sample",
            sources: &sources,
        };
        let references = Rust
            .references(&context)
            .unwrap_or_else(|error| panic!("namespace reexport fixture failed: {error}"));
        assert!(references.iter().any(|reference| reference.source == "api.rs" && matches!(&reference.destination, Destination::Local { path, .. } if path == "store/mod.rs")));
        assert!(!references.iter().any(|reference| reference.source == "api.rs" && matches!(&reference.destination, Destination::Local { path, .. } if path == "store/internal.rs")));
    }

    #[test]
    fn an_include_preserves_written_parent_steps_before_path_normalization() {
        let sources = sources(&[
            ("lib.rs", "mod api;"),
            ("api/mod.rs", "include!(\"../body.rs\");"),
            ("body.rs", "pub fn read() {}"),
        ]);
        let session = Session::default();
        let context = Context {
            session: &session,
            module_root: Path::new("/__zoning_rust_unit__"),
            package: "sample",
            sources: &sources,
        };
        let references = Rust
            .references(&context)
            .unwrap_or_else(|error| panic!("include reach fixture failed: {error}"));
        assert!(references.iter().any(|reference| reference.source == "api/mod.rs" && matches!(&reference.destination, Destination::Local { path, hops: 1 } if path == "body.rs")));
    }

    #[test]
    fn a_missing_or_ungoverned_include_is_a_technical_failure() {
        for (files, reason) in [
            (vec![("lib.rs", "include!(\"missing.rs\");")], "missing"),
            (
                vec![("lib.rs", "include!(\"body.inc\");"), ("body.inc", "pub fn read() {}")],
                "requires a .rs",
            ),
        ] {
            let sources = sources(&files);
            let session = Session::default();
            let context = Context {
                session: &session,
                module_root: Path::new("/__zoning_rust_unit__"),
                package: "sample",
                sources: &sources,
            };
            match Rust.references(&context) {
                Ok(_) => panic!("an incomplete source include must not pass"),
                Err(error) => assert!(error.to_string().contains(reason), "{error}"),
            }
        }
    }
}
