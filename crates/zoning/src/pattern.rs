//! Path globs, with the semantics every other language's linter already agreed on.
//!
//! One primitive, no dependency: `*` stops at a separator, `?` is one non-separator
//! character, `[…]` is a class, and `**` is the only token that may cross a `/`.
//! A trailing `**` means *strictly underneath* — `assay/**` claims `assay/span.zig`
//! and not the directory itself — because a zone is a set of files, and a directory
//! is not a file.
//!
//! Matching is anchored at both ends. A glob is a claim about a whole path, so
//! `*.zig` is `b.zig` and never `a/b.zig`; the alternative silently widens every
//! zone in every contract, which is the one failure mode a boundary tool cannot have.

use std::fmt;

/// One compiled glob.
#[derive(Clone, Debug)]
pub struct Pattern {
    raw: Box<str>,
    parts: Box<[Piece]>,
}

/// A set of globs matched as one alternation.
#[derive(Clone, Debug, Default)]
pub struct Globs(Box<[Pattern]>);

#[derive(Clone, Debug)]
enum Piece {
    /// Exact bytes, separators included.
    Text(Box<[u8]>),
    /// `*` — zero or more non-separator bytes.
    Run,
    /// `?` — exactly one non-separator byte.
    One,
    /// `[…]` — one byte from a class.
    Class(Class),
    /// `**/` in the middle — zero or more whole segments.
    Segments,
    /// `**` at the end — everything left, separators included.
    Rest,
}

#[derive(Clone, Debug)]
struct Class {
    negated: bool,
    singles: Box<[u8]>,
    ranges: Box<[(u8, u8)]>,
}

impl Class {
    fn admits(&self, byte: u8) -> bool {
        let hit = self.singles.contains(&byte)
            || self.ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&byte));
        hit != self.negated
    }
}

impl Pattern {
    /// Compile one glob. Malformed classes degrade to literals, as every shell does.
    #[must_use]
    pub fn new(glob: &str) -> Self {
        let segments: Vec<&str> = glob.split('/').collect();
        let last = segments.len() - 1;
        let mut parts = Vec::new();
        for (i, segment) in segments.iter().enumerate() {
            if *segment == "**" {
                parts.push(if i == last { Piece::Rest } else { Piece::Segments });
                continue;
            }
            compile_segment(segment, &mut parts);
            if i != last {
                push_text(&mut parts, b'/');
            }
        }
        Self { raw: glob.into(), parts: parts.into() }
    }

    /// The glob exactly as it was written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Does this glob claim `path`? `path` is slash-separated and relative.
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        walk(&self.parts, path.as_bytes())
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl Globs {
    /// Compile a set of globs. An empty set claims nothing.
    #[must_use]
    pub fn new<I: IntoIterator<Item = S>, S: AsRef<str>>(globs: I) -> Self {
        Self(globs.into_iter().map(|g| Pattern::new(g.as_ref())).collect())
    }

    /// Does any glob in the set claim `path`?
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        self.0.iter().any(|p| p.matches(path))
    }

    /// The globs as written, for echoing a contract back to its author.
    pub fn raw(&self) -> impl ExactSizeIterator<Item = &str> {
        self.0.iter().map(Pattern::as_str)
    }

    /// Is the set empty — claiming nothing at all?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Globs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, pattern) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            fmt::Display::fmt(pattern, f)?;
        }
        Ok(())
    }
}

fn push_text(parts: &mut Vec<Piece>, byte: u8) {
    if let Some(Piece::Text(text)) = parts.last_mut() {
        let mut grown = text.to_vec();
        grown.push(byte);
        *text = grown.into();
    } else {
        parts.push(Piece::Text(vec![byte].into()));
    }
}

fn compile_segment(segment: &str, parts: &mut Vec<Piece>) {
    let bytes = segment.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'*' => parts.push(Piece::Run),
            b'?' => parts.push(Piece::One),
            b'[' => match read_class(bytes, i) {
                Some((class, next)) => {
                    parts.push(Piece::Class(class));
                    i = next;
                    continue;
                }
                None => push_text(parts, b'['),
            },
            byte => push_text(parts, byte),
        }
        i += 1;
    }
}

/// Read `[…]` starting at `open`, returning the class and the index after `]`.
fn read_class(bytes: &[u8], open: usize) -> Option<(Class, usize)> {
    let mut i = open + 1;
    let negated = matches!(bytes.get(i), Some(b'!' | b'^'));
    i += usize::from(negated);
    // A `]` in first position is a literal member, per POSIX.
    let first = i;
    let mut singles = Vec::new();
    let mut ranges = Vec::new();
    while i < bytes.len() {
        if bytes[i] == b']' && i > first {
            return Some((
                Class { negated, singles: singles.into(), ranges: ranges.into() },
                i + 1,
            ));
        }
        let is_range =
            bytes.get(i + 1) == Some(&b'-') && bytes.get(i + 2).is_some_and(|&b| b != b']');
        if is_range {
            ranges.push((bytes[i], bytes[i + 2]));
            i += 3;
        } else {
            singles.push(bytes[i]);
            i += 1;
        }
    }
    None // unterminated — the caller falls back to a literal `[`
}

/// Backtracking match. Depth is the piece count of one glob, which is a contract
/// author's line, not a property of the tree being judged.
fn walk(parts: &[Piece], path: &[u8]) -> bool {
    let Some((head, tail)) = parts.split_first() else {
        return path.is_empty();
    };
    match head {
        Piece::Text(text) => path.strip_prefix(&**text).is_some_and(|rest| walk(tail, rest)),
        Piece::One => {
            matches!(path.split_first(), Some((&b, rest)) if b != b'/' && walk(tail, rest))
        }
        Piece::Class(class) => {
            matches!(path.split_first(), Some((&b, rest)) if b != b'/' && class.admits(b) && walk(tail, rest))
        }
        Piece::Run => {
            let stop = path.iter().position(|&b| b == b'/').unwrap_or(path.len());
            (0..=stop).any(|k| walk(tail, &path[k..]))
        }
        Piece::Segments => {
            walk(tail, path)
                || path
                    .iter()
                    .enumerate()
                    .filter(|&(i, &b)| b == b'/' && i > 0)
                    .any(|(i, _)| walk(tail, &path[i + 1..]))
        }
        Piece::Rest => (0..=path.len()).any(|k| walk(tail, &path[k..])),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "a test that cannot construct its fixture has failed")]
mod tests {
    use super::*;

    /// Every case here was read off `CPython`'s `glob.translate(g, recursive=True,
    /// include_hidden=True)` + `re.match`, which is what the tool this one replaces
    /// used. A contract must not change meaning because its judge was rewritten.
    #[test]
    fn agrees_with_the_reference_implementation() {
        let cases: &[(&str, &str, bool)] = &[
            ("assay/**", "assay/span.zig", true),
            ("assay/**", "assay/a/b.zig", true),
            ("assay/**", "assay", false),
            ("assay/**", "assayed/x.zig", false),
            ("kernel/math/**", "kernel/math/glob.zig", true),
            ("kernel/math/**", "kernel/mathx/glob.zig", false),
            ("**/*_test.zig", "mark_test.zig", true),
            ("**/*_test.zig", "a/b/mark_test.zig", true),
            ("**/*_test.zig", "a/b/mark.zig", false),
            ("**", "anything/at/all.zig", true),
            ("**", "", true),
            ("a/**/b.zig", "a/b.zig", true),
            ("a/**/b.zig", "a/x/b.zig", true),
            ("a/**/b.zig", "a/x/y/b.zig", true),
            ("a/**/b.zig", "b.zig", false),
            ("*.zig", "b.zig", true),
            ("*.zig", "a/b.zig", false),
            ("portal.zig", "portal.zig", true),
            ("portal.zig", "portal.zig.bak", false),
            ("surface/api.zig", "surface/api.zig", true),
            ("zz_*.zig", "zz_probe.zig", true),
            ("zz_*.zig", "zz_a/b.zig", false),
        ];
        for &(glob, path, want) in cases {
            assert_eq!(Pattern::new(glob).matches(path), want, "{glob} vs {path}");
        }
    }

    #[test]
    fn question_marks_and_classes_stop_at_a_separator() {
        assert!(Pattern::new("a?c.zig").matches("abc.zig"));
        assert!(!Pattern::new("a?c.zig").matches("a/c.zig"));
        assert!(Pattern::new("[abc]x.zig").matches("bx.zig"));
        assert!(!Pattern::new("[abc]x.zig").matches("dx.zig"));
        assert!(Pattern::new("[a-c]x.zig").matches("cx.zig"));
        assert!(Pattern::new("[!a-c]x.zig").matches("dx.zig"));
        assert!(!Pattern::new("[!a-c]x.zig").matches("bx.zig"));
        assert!(Pattern::new("a[.zig").matches("a[.zig"), "unterminated class is a literal");
    }

    #[test]
    fn a_set_is_an_alternation_and_an_empty_set_claims_nothing() {
        let set = Globs::new(["a/**", "b.zig"]);
        assert!(set.matches("a/deep/x.zig"));
        assert!(set.matches("b.zig"));
        assert!(!set.matches("c.zig"));
        assert!(!Globs::default().matches("anything"));
    }
}
