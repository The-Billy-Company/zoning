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
};

impl Dialect for Zig {
    fn name(&self) -> &'static str {
        "zig"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }

    fn prose(&self) -> &Prose {
        &PROSE
    }

    fn imports(&self, source: &str, code: &[u8]) -> Vec<Import> {
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
        Zig.imports(source, &code).into_iter().map(|i| i.spec).collect()
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
    fn a_named_module_is_not_a_path() {
        assert!(Zig.is_local("a/b.zig"));
        assert!(!Zig.is_local("std"));
        assert!(!Zig.is_local("irregex"));
    }
}
