//! Telling code from prose.
//!
//! Any textual judgment about source is only as trustworthy as its ability to tell
//! the two apart. `@import("sweep.zig")` written inside a multiline string is test
//! input, not an edge; a path quoted in a header comment is documentation, not a
//! dependency. So a scan runs against a *blanked* copy in which every comment,
//! string literal, and character literal has become spaces of the same length.
//!
//! Offset-preserving is the load-bearing property: a match found in the blanked
//! copy can be read back out of the original bytes at the same index, which is how
//! a dialect recovers an import's path without a second lexer. Blanking is done
//! byte for byte, so a multi-byte character inside a comment costs its own width in
//! spaces and every later offset still lands where it did.

/// The comment and literal conventions of one language family.
pub struct Prose {
    /// What opens a comment that runs to end of line.
    pub line_comment: &'static str,
    /// What opens and closes a block comment, if the language has them.
    pub block_comment: Option<(&'static str, &'static str)>,
    /// A prefix that makes the rest of the line a string literal, as Zig's `\\` does.
    pub line_string: Option<&'static str>,
    /// Quote characters that open a single-line literal, with backslash escapes.
    pub quotes: &'static [u8],
    /// Multi-character literal delimiters that are allowed to hold a raw newline, as
    /// Python's `"""`/`'''` do. Checked before `quotes`, so a triple quote is never
    /// misread as an empty single-line literal followed by a stray one. A single-byte
    /// `quotes` table cannot spell a three-byte delimiter, and unlike `quotes`, one of
    /// these is expected to span lines — so it is blanked to its matching close (or to
    /// the end of the file, unterminated) rather than cut off at the first `\n`.
    pub triple_quotes: &'static [&'static str],
    /// Whether a backslash at end of line joins it to the next, as Python's does.
    ///
    /// A joined line is one statement wearing two rows, so the blanked copy erases the
    /// backslash *and* its newline to spaces: every scan downstream reads a logical
    /// line without knowing the rule, and a dialect that stops at `\n` stops in the
    /// right place. Line numbers are taken from the original text, so nothing moves.
    pub line_join: bool,
}

impl Prose {
    /// `text` with comments and literals blanked to spaces, byte offsets preserved.
    ///
    /// One left-to-right scan: whichever token starts first wins, so a `//` inside a
    /// string never opens a comment and a quote inside a comment never opens a string.
    #[must_use]
    pub fn code_only(&self, text: &str) -> Vec<u8> {
        let mut out = text.as_bytes().to_vec();
        if let Some(prefix) = self.line_string {
            blank_line_strings(&mut out, prefix.as_bytes());
        }
        let mut i = 0;
        while i < out.len() {
            if out[i] == b'\n' {
                i += 1;
            } else if out[i..].starts_with(self.line_comment.as_bytes()) {
                i = blank_to(&mut out, i, |b| b == b'\n');
            } else if let Some(end) = self.block_at(&out, i) {
                blank(&mut out, i, end);
                i = end;
            } else if let Some(delim) = self.triple_at(&out, i) {
                i = blank_triple(&mut out, i, delim);
            } else if self.quotes.contains(&out[i]) {
                i = blank_literal(&mut out, i);
            } else if let Some(end) = self.join_at(&out, i) {
                blank(&mut out, i, end);
                i = end;
            } else {
                i += 1;
            }
        }
        out
    }

    fn block_at(&self, buf: &[u8], i: usize) -> Option<usize> {
        let (open, close) = self.block_comment?;
        if !buf[i..].starts_with(open.as_bytes()) {
            return None;
        }
        let from = i + open.len();
        let end = buf[from..]
            .windows(close.len())
            .position(|w| w == close.as_bytes())
            .map_or(buf.len(), |k| from + k + close.len());
        Some(end)
    }

    fn triple_at(&self, buf: &[u8], i: usize) -> Option<&'static [u8]> {
        self.triple_quotes.iter().map(|d| d.as_bytes()).find(|d| buf[i..].starts_with(d))
    }

    /// The index just past a line join opening at `i`, if one does.
    ///
    /// Only ever consulted from code position — a comment or a literal is consumed
    /// whole by an earlier branch — so a backslash inside either can never join
    /// anything, which is what keeps a trailing `\` in a comment from swallowing the
    /// import on the line below it.
    fn join_at(&self, buf: &[u8], i: usize) -> Option<usize> {
        if !self.line_join || buf[i] != b'\\' {
            return None;
        }
        let end = i + 1 + usize::from(buf.get(i + 1) == Some(&b'\r'));
        (buf.get(end) == Some(&b'\n')).then_some(end + 1)
    }
}

/// Blank a delimiter-quoted literal that is allowed to span lines; returns the index
/// just past it. Unterminated blanks to end of file, matching every language that
/// requires this delimiter to eventually close.
fn blank_triple(buf: &mut [u8], i: usize, delim: &[u8]) -> usize {
    let mut j = i + delim.len();
    while j < buf.len() {
        if buf[j] == b'\\' && j + 1 < buf.len() {
            j += 2;
        } else if buf[j..].starts_with(delim) {
            let end = j + delim.len();
            blank(buf, i, end);
            return end;
        } else {
            j += 1;
        }
    }
    blank(buf, i, buf.len());
    buf.len()
}

/// Blank every line whose first non-blank content is the line-string prefix.
fn blank_line_strings(buf: &mut [u8], prefix: &[u8]) {
    let mut start = 0;
    while start <= buf.len() {
        let end = buf[start..].iter().position(|&b| b == b'\n').map_or(buf.len(), |k| start + k);
        let head = buf[start..end].iter().position(|&b| !matches!(b, b' ' | b'\t'));
        if let Some(head) = head
            && buf[start + head..end].starts_with(prefix)
        {
            blank(buf, start, end);
        }
        start = end + 1;
    }
}

fn blank(buf: &mut [u8], from: usize, to: usize) {
    let end = to.min(buf.len());
    buf[from..end].fill(b' ');
}

fn blank_to(buf: &mut [u8], from: usize, stop: impl Fn(u8) -> bool) -> usize {
    let end = buf[from..].iter().position(|&b| stop(b)).map_or(buf.len(), |k| from + k);
    blank(buf, from, end);
    end
}

/// Blank a `"…"` or `'…'` literal opening at `i`; returns the index just past it.
/// An unterminated literal blanks to end of line, matching every language that
/// forbids a raw newline inside one.
fn blank_literal(buf: &mut [u8], i: usize) -> usize {
    let quote = buf[i];
    let mut j = i + 1;
    while j < buf.len() {
        match buf[j] {
            b'\n' => break,
            b'\\' if j + 1 < buf.len() => j += 2,
            b if b == quote => {
                blank(buf, i, j + 1);
                return j + 1;
            }
            _ => j += 1,
        }
    }
    blank(buf, i, j);
    j
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "a test that cannot construct its fixture has failed")]
mod tests {
    use super::*;

    const ZIG: Prose = Prose {
        line_comment: "//",
        block_comment: None,
        line_string: Some("\\\\"),
        quotes: b"\"'",
        triple_quotes: &[],
        line_join: false,
    };

    const TRIPLE: Prose = Prose {
        line_comment: "#",
        block_comment: None,
        line_string: None,
        quotes: b"\"'",
        triple_quotes: &["\"\"\"", "'''"],
        line_join: true,
    };

    fn blanked(text: &str) -> String {
        String::from_utf8(ZIG.code_only(text)).expect("blanking yields spaces, never bytes")
    }

    #[test]
    fn prose_is_erased_and_offsets_survive() {
        let src = "const a = @import(\"real.zig\");\n// @import(\"comment.zig\")\n\\\\ @import(\"doc.zig\")\n";
        let out = blanked(src);
        assert_eq!(out.len(), src.len(), "byte offsets must not move");
        assert_eq!(out.matches("@import").count(), 1, "only the real import survives");
        assert_eq!(out.lines().count(), src.lines().count());
    }

    #[test]
    fn a_slash_inside_a_string_does_not_open_a_comment() {
        let out = blanked("const p = \"http://x\"; const q = @import(\"a.zig\");");
        assert!(out.contains("@import"), "the trailing code must survive: {out}");
    }

    #[test]
    fn a_quote_inside_a_comment_does_not_open_a_string() {
        let out = blanked("// he said \"hi\n@import(\"a.zig\")");
        assert!(out.contains("@import"));
    }

    #[test]
    fn multibyte_prose_costs_its_own_width() {
        let src = "// … an ellipsis\nconst x = 1;\n";
        assert_eq!(blanked(src).len(), src.len());
    }

    #[test]
    fn a_triple_quote_spans_lines_and_preserves_offsets() {
        let src = "x = \"\"\"\nimport os\n\"\"\"\nimport sys\n";
        let out = String::from_utf8(TRIPLE.code_only(src)).expect("blanking yields spaces");
        assert_eq!(out.len(), src.len(), "byte offsets must not move");
        assert_eq!(out.matches("import").count(), 1, "only the real import survives");
    }

    #[test]
    fn a_lone_quote_inside_a_triple_quote_does_not_close_it() {
        let src = "x = '''a \" quote and a ' quote'''\nimport sys\n";
        let out = String::from_utf8(TRIPLE.code_only(src)).expect("blanking yields spaces");
        assert_eq!(out.matches("import").count(), 1);
    }

    fn joined(text: &str) -> String {
        String::from_utf8(TRIPLE.code_only(text)).expect("blanking yields spaces, never bytes")
    }

    #[test]
    fn a_line_join_erases_its_own_newline() {
        let src = "import pkg.a, \\\n    pkg.c\n";
        let out = joined(src);
        assert_eq!(out.len(), src.len(), "byte offsets must not move");
        assert!(!out.contains('\\'), "the backslash is not part of any name: {out}");
        assert_eq!(out.lines().count(), 1, "one statement, one logical line: {out}");
        assert_eq!(
            out.split_whitespace().collect::<Vec<_>>(),
            ["import", "pkg.a,", "pkg.c"],
            "the two rows read as one statement: {out}"
        );
    }

    #[test]
    fn a_carriage_return_still_joins() {
        assert_eq!(joined("import a, \\\r\n    b\n").lines().count(), 1);
    }

    #[test]
    fn a_backslash_in_a_comment_joins_nothing() {
        let out = joined("# a windows path C:\\\nimport sys\n");
        assert!(out.contains("import sys"), "the next line must survive whole: {out}");
        assert_eq!(out.lines().count(), 2, "a comment ends at its newline: {out}");
    }

    #[test]
    fn a_backslash_without_a_newline_is_left_alone() {
        assert!(joined("x = a \\ b\n").contains('\\'));
    }

    #[test]
    fn a_dialect_without_line_joins_keeps_its_newline() {
        assert_eq!(blanked("const a = 1; \\\nconst b = 2;\n").lines().count(), 2);
    }

    #[test]
    fn an_unterminated_triple_quote_blanks_to_end_of_file() {
        let out = String::from_utf8(TRIPLE.code_only("x = \"\"\"\nnever closed\n"))
            .expect("blanking yields spaces");
        assert!(!out.contains("closed"), "the open literal must swallow the rest: {out}");
    }
}
