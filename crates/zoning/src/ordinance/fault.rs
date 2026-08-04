//! Where a complaint happened, and how it reads.
//!
//! Every fault carries the span of the word that caused it, not just the file.
//! A contract whose errors nobody can locate is a contract nobody will fix, so
//! the rendered form quotes the offending line and underlines the token.

use std::fmt;
use std::path::{Path, PathBuf};

/// A token's position, in the terms an editor uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    /// The file the token was read from.
    pub file: PathBuf,
    /// 1-based line.
    pub line: usize,
    /// 1-based column, counted in characters.
    pub col: usize,
    /// How many characters to underline.
    pub width: usize,
}

impl Span {
    /// A span pointing at the first character of `file`, for whole-file faults.
    #[must_use]
    pub fn head(file: &Path) -> Self {
        Self { file: file.to_path_buf(), line: 1, col: 1, width: 1 }
    }
}

/// A `.zone` file is malformed, or claims something the tree contradicts.
#[derive(Clone, Debug)]
pub struct Fault {
    /// What went wrong, in one lowercase sentence.
    pub message: String,
    /// Where it went wrong.
    pub span: Span,
    /// The full source, so the faulting line can be quoted back.
    pub source: String,
}

impl Fault {
    /// Build a fault at `span`.
    pub fn at(message: impl Into<String>, span: Span, source: &str) -> Self {
        Self { message: message.into(), span, source: source.to_owned() }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Span { file, line, col, width } = &self.span;
        write!(f, "{}:{line}:{col}: {}", file.display(), self.message)?;
        // A fault at end-of-file points one line past the last one there is. Quote
        // that last line anyway and point off its end: "the file stopped here" is
        // the most useful thing an unterminated block can say.
        let mut line = *line;
        let mut col = *col;
        let text = self.source.lines().nth(line.saturating_sub(1)).unwrap_or_else(|| {
            let last = self.source.lines().next_back().unwrap_or_default();
            line = self.source.lines().count().max(1);
            col = last.chars().count() + 1;
            last
        });
        let gutter = format!("{line:>5} | ");
        write!(f, "\n{gutter}{text}\n")?;
        for _ in 0..gutter.chars().count() + col - 1 {
            f.write_str(" ")?;
        }
        for _ in 0..(*width).max(1) {
            f.write_str("^")?;
        }
        Ok(())
    }
}

impl std::error::Error for Fault {}
