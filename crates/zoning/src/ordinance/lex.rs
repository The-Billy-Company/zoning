//! Source text → tokens, each carrying the span it came from.
//!
//! The token set is deliberately tiny. A zone file is paths, prose, and a handful
//! of keywords, so there is no operator table, no precedence, and no escaping
//! rules beyond a plain double-quoted string.
//!
//! Line breaks are real tokens. The language is line-oriented — one zone per line,
//! one law per line — which is what lets a zone read as `math  kernel/math/**`
//! instead of needing a separator, and what makes every diagnostic land on the line
//! the author actually typed. Runs of blank and comment-only lines collapse into a
//! single break, so the parser never counts whitespace.

use std::fmt;
use std::path::Path;

use super::fault::{Fault, Span};

/// What a token is. A bare word covers identifiers and globs alike, because a
/// source path never contains a space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// An identifier or a path glob.
    Word,
    /// Quoted prose: a reason.
    Text,
    /// `->`
    Arrow,
    /// `{`
    Open,
    /// `}`
    Close,
    /// End of a line.
    Break,
    /// End of the file.
    End,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Word => "a word",
            Self::Text => "quoted text",
            Self::Arrow => "`->`",
            Self::Open => "`{`",
            Self::Close => "`}`",
            Self::Break => "end of line",
            Self::End => "end of file",
        })
    }
}

/// One lexed token.
#[derive(Clone, Debug)]
pub(super) struct Token {
    /// Which kind of token.
    pub kind: Kind,
    /// Words and text carry their value; punctuation carries its lexeme.
    pub text: String,
    /// Where it came from.
    pub span: Span,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Kind::Word | Kind::Text => write!(f, "`{}`", self.text),
            kind => fmt::Display::fmt(&kind, f),
        }
    }
}

const BREAKS: [char; 6] = ['{', '}', '"', ' ', '\t', '\r'];

struct Scan<'a> {
    src: &'a [char],
    file: &'a Path,
    text: &'a str,
    out: Vec<Token>,
    i: usize,
    line: usize,
    col: usize,
}

impl Scan<'_> {
    fn at(&self, width: usize) -> Span {
        Span { file: self.file.to_path_buf(), line: self.line, col: self.col, width }
    }

    fn push(&mut self, kind: Kind, text: impl Into<String>, span: Span) {
        self.out.push(Token { kind, text: text.into(), span });
    }

    /// Emit a token spanning `width` characters at the current position.
    fn emit(&mut self, kind: Kind, text: impl Into<String>, width: usize) {
        let where_it_is = self.at(width);
        self.push(kind, text, where_it_is);
    }

    fn starts(&self, at: usize, needle: &str) -> bool {
        needle.chars().enumerate().all(|(k, c)| self.src.get(at + k) == Some(&c))
    }

    /// A `\\` block: prose with quotes and backslashes in it, delimited per line so
    /// a missing terminator cannot swallow the rest of the file. Lines fold into one
    /// paragraph, because a reason is read as a sentence.
    fn folded(&mut self) {
        let (start_line, start_col) = (self.line, self.col);
        let mut parts: Vec<String> = Vec::new();
        loop {
            let end =
                (self.i..self.src.len()).find(|&k| self.src[k] == '\n').unwrap_or(self.src.len());
            parts.push(
                self.src[(self.i + 2).min(end)..end].iter().collect::<String>().trim().into(),
            );
            self.i = end + 1;
            self.line += 1;
            self.col = 1;
            let mut probe = self.i;
            while matches!(self.src.get(probe), Some(' ' | '\t')) {
                probe += 1;
            }
            if !self.starts(probe, "\\\\") {
                break;
            }
            self.col = probe - end;
            self.i = probe;
        }
        let folded = parts.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        let span =
            Span { file: self.file.to_path_buf(), line: start_line, col: start_col, width: 2 };
        self.push(Kind::Text, folded, span);
        // The block consumed its own trailing newline; the parser still needs to
        // see that the line ended.
        let after = Span { file: self.file.to_path_buf(), line: self.line - 1, col: 1, width: 1 };
        self.push(Kind::Break, "\n", after);
    }

    fn quoted(&mut self) -> Result<(), Fault> {
        let start_col = self.col;
        let unterminated = |scan: &Self| {
            Fault::at(
                "unterminated text — a `\"` is missing",
                Span { file: scan.file.to_path_buf(), line: scan.line, col: start_col, width: 1 },
                scan.text,
            )
        };
        let mut j = self.i + 1;
        let mut buf = String::new();
        while let Some(&ch) = self.src.get(j) {
            match ch {
                '\n' => return Err(unterminated(self)),
                '\\' if j + 1 < self.src.len() => {
                    buf.push(self.src[j + 1]);
                    j += 2;
                }
                '"' => {
                    let width = j + 1 - self.i;
                    let span = Span {
                        file: self.file.to_path_buf(),
                        line: self.line,
                        col: start_col,
                        width,
                    };
                    self.push(Kind::Text, buf, span);
                    self.i = j + 1;
                    self.col += width;
                    return Ok(());
                }
                _ => {
                    buf.push(ch);
                    j += 1;
                }
            }
        }
        Err(unterminated(self))
    }
}

/// Scan `source` into tokens, ending with exactly one break then end-of-file.
pub(super) fn tokenize(source: &str, file: &Path) -> Result<Vec<Token>, Fault> {
    let chars: Vec<char> = source.chars().collect();
    let mut scan = Scan { src: &chars, file, text: source, out: Vec::new(), i: 0, line: 1, col: 1 };

    while let Some(&ch) = scan.src.get(scan.i) {
        if ch == '\n' {
            // Collapse blank runs: the parser wants "a line ended", not how many.
            if scan.out.last().is_some_and(|t| t.kind != Kind::Break) {
                scan.emit(Kind::Break, "\n", 1);
            }
            scan.i += 1;
            scan.line += 1;
            scan.col = 1;
        } else if matches!(ch, ' ' | '\t' | '\r') {
            scan.i += 1;
            scan.col += 1;
        } else if scan.starts(scan.i, "//") {
            while scan.src.get(scan.i).is_some_and(|&c| c != '\n') {
                scan.i += 1;
            }
        } else if scan.starts(scan.i, "->") {
            scan.emit(Kind::Arrow, "->", 2);
            scan.i += 2;
            scan.col += 2;
        } else if matches!(ch, '{' | '}') {
            let kind = if ch == '{' { Kind::Open } else { Kind::Close };
            scan.emit(kind, ch, 1);
            scan.i += 1;
            scan.col += 1;
        } else if scan.starts(scan.i, "\\\\") {
            scan.folded();
        } else if ch == '"' {
            scan.quoted()?;
        } else {
            let start = scan.i;
            while scan.src.get(scan.i).is_some_and(|c| !BREAKS.contains(c) && *c != '\n')
                && !scan.starts(scan.i, "//")
            {
                scan.i += 1;
            }
            let word: String = scan.src[start..scan.i].iter().collect();
            let width = scan.i - start;
            scan.emit(Kind::Word, word, width);
            scan.col += width;
        }
    }

    if scan.out.last().is_none_or(|t| t.kind != Kind::Break) {
        scan.emit(Kind::Break, "\n", 1);
    }
    scan.emit(Kind::End, "", 1);
    Ok(scan.out)
}
