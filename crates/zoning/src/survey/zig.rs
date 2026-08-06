//! Zig — the language that needs this tool most, and the first dialect.
//!
//! Zig has no module system *inside* a package. Every intra-package import is a
//! filesystem path spelled relative to the importing file, any file may name any
//! other file's path, there is no `internal/`, no export map, no visibility beyond
//! `pub` on a declaration — and because analysis is lazy, a genuine import cycle
//! compiles clean. Architecture in a Zig package is convention with nothing behind
//! it until something checks.
//!
//! The same property makes the graph recoverable exactly, with no toolchain: resolve
//! each `@import("…zig")` argument against its importer's directory.

use super::dialect::{Dialect, Import};
use super::prose::Prose;

/// The Zig dialect.
pub(super) struct Zig;

const PROSE: Prose = Prose {
    line_comment: "//",
    // Zig has no block comments, on purpose. Neither does this dialect.
    block_comment: None,
    line_string: Some("\\\\"),
    quotes: b"\"'",
    triple_quotes: &[],
};

impl Dialect for Zig {
    fn name(&self) -> &'static str {
        "zig"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }

    fn manifests(&self) -> &'static [&'static str] {
        &["build.zig", "build.zig.zon"]
    }

    // `build.zig.zon` spells a vendored dependency `.name = .{ .path = "dir" }`, and
    // `.path` appears nowhere else in the grammar — `.paths` is the publish manifest and
    // is plural. So the keys alone recover every in-tree dependency without a ZON
    // parser, and a stray match would only ever excuse a directory the manifest already
    // pointed at.
    fn vendored(&self, manifest: &str) -> Vec<String> {
        let code = PROSE.code_only(manifest);
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(k) = find(&code[i..], b".path") {
            let head = i + k;
            i = head + b".path".len();
            let mut j = i;
            while matches!(code.get(j), Some(b' ' | b'\t')) {
                j += 1;
            }
            if code.get(j) != Some(&b'=') {
                continue;
            }
            if let Some(dir) = literal(manifest.as_bytes(), j + 1) {
                out.push(dir.trim_end_matches('/').to_owned());
            }
        }
        out
    }

    // `.name = .acme` since Zig 0.14 made it an enum literal; older manifests spell
    // it `.name = "acme"`. Both are read, because a tree does not upgrade its
    // manifests the day the compiler changes and a contract named after the wrong thing
    // is worse than one named after the directory.
    fn declared(&self, manifest: &str) -> Option<String> {
        let code = PROSE.code_only(manifest);
        let head = find(&code, b".name")?;
        let mut j = head + b".name".len();
        while matches!(code.get(j), Some(b' ' | b'\t')) {
            j += 1;
        }
        if code.get(j) != Some(&b'=') {
            return None;
        }
        j += 1;
        // The value is read from the original bytes, because `code_only` blanks a literal
        // *to whitespace* — skipping whitespace in the blanked copy would step over the
        // quoted form's entire value and land on the next token. The key was located in
        // the blanked copy so a commented-out `.name` cannot win; the value is read where
        // it is actually written. Same division of labour as the import lexer.
        if let Some(quoted) = literal(manifest.as_bytes(), j) {
            return Some(quoted);
        }
        let raw = manifest.as_bytes();
        while raw.get(j).is_some_and(u8::is_ascii_whitespace) {
            j += 1;
        }
        if raw.get(j) != Some(&b'.') {
            return None;
        }
        let word = &manifest[j + 1..];
        let end = word.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(word.len());
        (end > 0).then(|| word[..end].to_owned())
    }

    // `std` is the standard library; `builtin` is the compiler's own view of the
    // build; `root` is whatever file the compilation began at. All three are
    // available to every file of every Zig package by construction, so none of them
    // is a dependency a contract could meaningfully refuse.
    fn ambient(&self) -> &'static [&'static str] {
        &["std", "builtin", "root"]
    }

    fn prose(&self) -> &Prose {
        &PROSE
    }

    fn imports(
        &self,
        _path: &str,
        _roots: &[&str],
        _own: &str,
        source: &str,
        code: &[u8],
    ) -> Vec<Import> {
        let raw = source.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        // The `@import(` head only. Prose blanking erased the literal *including its
        // quotes*, so the head is located in the blanked copy and the argument is
        // read forward from the original bytes. One lexer, no phantom edges.
        while let Some(k) = find(&code[i..], b"@import") {
            let head = i + k;
            let mut j = head + b"@import".len();
            while matches!(code.get(j), Some(b' ' | b'\t')) {
                j += 1;
            }
            i = head + 1;
            if code.get(j) != Some(&b'(') {
                continue;
            }
            if let Some(spec) = literal(raw, j + 1) {
                out.push(Import { offset: head, spec });
                i = j;
            }
        }
        out
    }

    // Case-sensitive on purpose: an import spec is a literal path, and `.ZIG` is a
    // different file from `.zig` on every filesystem that distinguishes them.
    #[allow(clippy::case_sensitive_file_extension_comparisons, reason = "import specs are literal")]
    fn is_local(&self, spec: &str) -> bool {
        spec.ends_with(".zig")
    }

    fn escape_remedy(&self) -> &'static str {
        "Zig cannot follow a path across a module boundary; declare the dependency \
         as a named module in build.zig"
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Read the string literal opening at or after `from`.
///
/// `None` when the argument is not a plain string — `@import(foo)` over a comptime
/// value has no path to resolve.
fn literal(raw: &[u8], from: usize) -> Option<String> {
    let mut i = from;
    while raw.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    if raw.get(i) != Some(&b'"') {
        return None;
    }
    i += 1;
    let mut out = Vec::new();
    while let Some(&ch) = raw.get(i) {
        match ch {
            b'\\' if i + 1 < raw.len() => {
                out.push(raw[i + 1]);
                i += 2;
            }
            b'"' => return String::from_utf8(out).ok(),
            b'\n' => return None,
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs(source: &str) -> Vec<String> {
        let code = PROSE.code_only(source);
        Zig.imports("test.zig", &[], "", source, &code).into_iter().map(|i| i.spec).collect()
    }

    #[test]
    fn reads_real_imports_and_ignores_quoted_ones() {
        let src = "\
const std = @import(\"std\");
const a = @import(\"../a/b.zig\");
// const c = @import(\"comment.zig\");
const doc =
    \\\\@import(\"prose.zig\")
;
const d = @import( \"spaced.zig\" );
const e = @import(comptime_value);
";
        assert_eq!(specs(src), ["std", "../a/b.zig", "spaced.zig"]);
    }

    #[test]
    fn reads_in_tree_dependencies_and_not_the_publish_paths() {
        let zon = "\
.{
    .name = .acme,
    .dependencies = .{
        // .vendor = .{ .path = \"commented\" },
        .vendor = .{ .path = \"vendor\" },
        .fetched = .{ .url = \"https://example/x.tar.gz\", .hash = \"…\" },
    },
    .paths = .{ \"build.zig\", \"vendor\", \"README.md\" },
}
";
        assert_eq!(Zig.vendored(zon), ["vendor"]);
    }

    #[test]
    fn reads_the_declared_name_in_either_spelling() {
        // Zig 0.14 onward, and what every current manifest here spells.
        assert_eq!(
            Zig.declared(".{ .name = .acme, .version = \"0.1.0\" }").as_deref(),
            Some("acme")
        );
        // Before it, and still on disk in trees that have not moved.
        assert_eq!(Zig.declared(".{ .name = \"acme\" }").as_deref(), Some("acme"));
        // `.name` is the first key by convention but not by rule, and a commented-out one
        // must not win — the same prose discipline the import lexer uses.
        let zon = "\
.{
    // .name = .wrong,
    .version = \"0.1.0\",
    .name = .right,
}
";
        assert_eq!(Zig.declared(zon).as_deref(), Some("right"));
        // A manifest that names nothing leaves the directory to say it.
        assert_eq!(Zig.declared(".{ .version = \"0.1.0\" }"), None);
    }

    #[test]
    fn a_named_module_is_not_a_path() {
        assert!(Zig.is_local("a/b.zig"));
        assert!(!Zig.is_local("std"));
        assert!(!Zig.is_local("acme"));
    }
}
