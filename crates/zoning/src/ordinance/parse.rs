//! Tokens → parse tree. Recursive descent, one declaration per call.
//!
//! The grammar, in full:
//!
//! ```text
//! file     = { decl } END
//! decl     = package | zones | seal | keep | limit | forbid | variance
//! package  = "package" WORD "{" BREAK { setting BREAK } "}" BREAK
//! setting  = "root" WORD | "facade" paths | "exclude" paths
//! zones    = "zones" "{" BREAK { WORD paths BREAK } "}" BREAK
//! paths    = WORD... | "{" BREAK { WORD... BREAK } "}"
//! seal     = "seal" WORD "through" WORD [ "open" "to" paths ] BREAK
//! keep     = "keep" WORD "to" paths BREAK
//! limit    = "limit" "reach" "to" INT "hops" BREAK
//! forbid   = "forbid" "cycles" "across" "directories" BREAK
//! variance = "variance" ( edge | cycle )
//! edge     = LAW WORD "->" WORD "because" TEXT BREAK
//! cycle    = "cycle" "{" BREAK { WORD BREAK } "}" "because" TEXT BREAK
//! LAW      = "zone" | "seal" | "keep" | "cycle" | "reach" | "escape"
//! ```
//!
//! `keep` takes a single subject rather than a `paths` list so the inline form
//! stays unambiguous against its own `to` — and because one claim per line is what
//! lets each keep carry the comment that justifies it.
//!
//! Zones are listed low to high, and that vertical order *is* the stack: reading
//! down the block is reading up the architecture. `because` sits in the grammar
//! rather than in a validation pass because an unexplained exception is the thing
//! this language most wants to make unsayable.

use std::path::Path;

use super::fault::{Fault, Span};
use super::law::Law;
use super::lex::{Kind, Token, tokenize};

/// The `package { … }` header: what this file governs.
pub(super) struct Package {
    pub name: Token,
    pub root: Option<Token>,
    pub facade: Vec<Token>,
    pub exclude: Vec<Token>,
}

/// One `name  paths…` row of the zones block.
pub(super) struct Zone {
    pub name: Token,
    pub globs: Vec<Token>,
}

/// One `seal DIR through FILE [open to …]`.
pub(super) struct Seal {
    pub path: Token,
    pub entry: Token,
    pub open: Vec<Token>,
}

/// One `keep REGION to IMPORTERS…`.
pub(super) struct Keep {
    pub subject: Token,
    pub importers: Vec<Token>,
}

/// One ratified exception. `because` is grammar, so `reason` is never empty.
pub(super) struct Variance {
    pub law: Token,
    /// Two words for an edge, N for a cycle.
    pub subject: Vec<Token>,
    pub reason: Token,
}

/// One parsed `.zone` file, exactly as written, before it means anything.
pub(super) struct Tree {
    pub package: Package,
    pub zones: Vec<Zone>,
    pub seals: Vec<Seal>,
    pub keeps: Vec<Keep>,
    pub variances: Vec<Variance>,
    pub reach: Option<(u32, Span)>,
}

/// Spellings that changed when `ward` became `zoning`, and what they became.
/// A contract written against the old tool should say so, not fail as gibberish.
const RENAMED: [(&str, &str); 3] = [("tiers", "zones"), ("tier", "zone"), ("allow", "variance")];

struct Reader<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    at: usize,
}

impl Reader<'_> {
    fn head(&self) -> &Token {
        &self.tokens[self.at]
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.at].clone();
        self.at = (self.at + 1).min(self.tokens.len() - 1);
        token
    }

    fn fail(&self, want: &str) -> Fault {
        let token = self.head();
        let renamed = RENAMED.iter().find(|(old, _)| *old == token.text);
        let message = match renamed {
            Some((old, new)) => format!(
                "expected {want}, found `{old}` — this is a `.zone` file, where `{old}` \
                 is spelled `{new}`"
            ),
            None => format!("expected {want}, found {token}"),
        };
        Fault::at(message, token.span.clone(), self.source)
    }

    fn at(&self, token: &Token, message: impl Into<String>) -> Fault {
        Fault::at(message, token.span.clone(), self.source)
    }

    fn take(&mut self, kind: Kind, want: &str) -> Result<Token, Fault> {
        if self.head().kind == kind { Ok(self.bump()) } else { Err(self.fail(want)) }
    }

    fn word(&mut self, want: &str) -> Result<Token, Fault> {
        self.take(Kind::Word, want)
    }

    fn keyword(&mut self, allowed: &[&str]) -> Result<Token, Fault> {
        if self.looking_at(allowed) {
            return Ok(self.bump());
        }
        let spelled = allowed.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(" or ");
        Err(self.fail(&spelled))
    }

    fn looking_at(&self, allowed: &[&str]) -> bool {
        self.head().kind == Kind::Word && allowed.contains(&self.head().text.as_str())
    }

    /// One or more paths: inline to end of line, or a `{ … }` block.
    ///
    /// The block form exists because a zone is often a hand-listed partition too
    /// wide for one line, and wrapping it is worth more than the brevity.
    fn paths(&mut self, want: &str) -> Result<Vec<Token>, Fault> {
        if self.head().kind != Kind::Open {
            let mut out = vec![self.word(want)?];
            while self.head().kind == Kind::Word {
                out.push(self.bump());
            }
            return Ok(out);
        }
        self.bump();
        self.endline()?;
        let mut out = Vec::new();
        while self.head().kind != Kind::Close {
            if self.head().kind == Kind::End {
                return Err(self.fail("`}` to close the list"));
            }
            out.push(self.word(want)?);
            if self.head().kind == Kind::Break {
                self.bump();
            }
        }
        let close = self.bump();
        if out.is_empty() {
            return Err(self.at(&close, format!("expected {want}, found {close}")));
        }
        Ok(out)
    }

    fn endline(&mut self) -> Result<(), Fault> {
        self.take(Kind::Break, "end of line").map(drop)
    }

    fn skip_blank(&mut self) {
        while self.head().kind == Kind::Break {
            self.bump();
        }
    }

    /// Consume `{` and the break after it, reporting the opener if absent.
    fn block(&mut self, opener: &Token) -> Result<(), Fault> {
        if self.head().kind != Kind::Open {
            return Err(self.fail(&format!("`{{` to open the `{}` block", opener.text)));
        }
        self.bump();
        self.endline()
    }

    fn parse(mut self) -> Result<Tree, Fault> {
        let mut package = None;
        let mut zones = Vec::new();
        let mut seen_zones = false;
        let (mut seals, mut keeps, mut variances) = (Vec::new(), Vec::new(), Vec::new());
        let mut reach = None;

        self.skip_blank();
        while self.head().kind != Kind::End {
            let lead =
                self.keyword(&["package", "zones", "seal", "keep", "limit", "forbid", "variance"])?;
            match lead.text.as_str() {
                "package" => {
                    if package.is_some() {
                        return Err(self.at(
                            &lead,
                            "a second `package` block — a zone file governs exactly one \
                             package",
                        ));
                    }
                    package = Some(self.package(&lead)?);
                }
                "zones" => {
                    if seen_zones {
                        return Err(self.at(
                            &lead,
                            "a second `zones` block — merge them so the stack has one \
                             readable order",
                        ));
                    }
                    zones = self.zones(&lead)?;
                    seen_zones = true;
                }
                "seal" => seals.push(self.seal()?),
                "keep" => keeps.push(self.keep()?),
                "limit" => reach = Some(self.limit()?),
                "forbid" => self.forbid()?,
                _ => variances.push(self.variance()?),
            }
            self.skip_blank();
        }

        let head = self.tokens[0].clone();
        let Some(package) = package else {
            return Err(self.at(&head, "no `package` block — a zone file must say what it governs"));
        };
        if zones.is_empty() {
            return Err(
                self.at(&head, "no `zones` block — a contract with no zones governs nothing")
            );
        }
        Ok(Tree { package, zones, seals, keeps, variances, reach })
    }

    fn package(&mut self, lead: &Token) -> Result<Package, Fault> {
        let name = self.word("the package name")?;
        self.block(lead)?;
        let (mut root, mut facade, mut exclude) = (None, Vec::new(), Vec::new());
        while self.head().kind != Kind::Close {
            if self.head().kind == Kind::End {
                return Err(self.fail("`}` to close the `package` block"));
            }
            match self.keyword(&["root", "facade", "exclude"])?.text.as_str() {
                "root" => root = Some(self.word("the source root directory")?),
                "facade" => facade.extend(self.paths("a facade path")?),
                _ => exclude.extend(self.paths("a path to exclude")?),
            }
            self.endline()?;
            self.skip_blank();
        }
        self.bump();
        self.endline()?;
        Ok(Package { name, root, facade, exclude })
    }

    fn zones(&mut self, lead: &Token) -> Result<Vec<Zone>, Fault> {
        self.block(lead)?;
        let mut out = Vec::new();
        while self.head().kind != Kind::Close {
            if self.head().kind == Kind::End {
                return Err(self.fail("`}` to close the `zones` block"));
            }
            let name = self.word("a zone name")?;
            let want = format!("a path for zone `{}`", name.text);
            out.push(Zone { name, globs: self.paths(&want)? });
            self.endline()?;
            self.skip_blank();
        }
        self.bump();
        self.endline()?;
        Ok(out)
    }

    fn seal(&mut self) -> Result<Seal, Fault> {
        let path = self.word("the directory to seal")?;
        self.keyword(&["through"])?;
        let entry = self.word("the entry filename")?;
        let mut open = Vec::new();
        if self.looking_at(&["open"]) {
            self.bump();
            self.keyword(&["to"])?;
            open = self.paths("a path allowed past the seal")?;
        }
        self.endline()?;
        Ok(Seal { path, entry, open })
    }

    fn keep(&mut self) -> Result<Keep, Fault> {
        let subject = self.word("the region to keep")?;
        self.keyword(&["to"])?;
        let importers = self.paths("an importer allowed to reach it")?;
        self.endline()?;
        Ok(Keep { subject, importers })
    }

    fn limit(&mut self) -> Result<(u32, Span), Fault> {
        self.keyword(&["reach"])?;
        self.keyword(&["to"])?;
        let hops = self.word("a hop count")?;
        let Ok(count) = hops.text.parse::<u32>() else {
            return Err(self.at(&hops, format!("`{}` is not a hop count", hops.text)));
        };
        self.keyword(&["hops", "hop"])?;
        self.endline()?;
        Ok((count, hops.span))
    }

    /// `forbid cycles across directories` — documentary, and the only phrasing.
    ///
    /// Cycles have no off switch: a contract cannot opt out of the law, only ratify
    /// individual cycles with `variance cycle … because`. The statement exists so a
    /// reader sees the full set of laws in force without having to know which ones
    /// are implicit.
    fn forbid(&mut self) -> Result<(), Fault> {
        self.keyword(&["cycles"])?;
        self.keyword(&["across"])?;
        self.keyword(&["directories"])?;
        self.endline()
    }

    fn variance(&mut self) -> Result<Variance, Fault> {
        let law = self.keyword(&Law::NAMES)?;
        let subject = if law.text == "cycle" {
            self.cycle_members(&law)?
        } else {
            let src = self.word("the importing file")?;
            self.take(Kind::Arrow, "`->`")?;
            vec![src, self.word("the imported file")?]
        };
        self.skip_blank(); // `because` may wrap onto its own line
        self.keyword(&["because"])?;
        self.skip_blank(); // and a `\\` folded reason onto the line after that
        let reason = self.take(
            Kind::Text,
            "a reason — every exception must say how it gets retired (\"…\" or a `\\\\` block)",
        )?;
        if reason.text.trim().is_empty() {
            return Err(self.at(&reason, "an empty reason ratifies nothing"));
        }
        self.endline()?;
        Ok(Variance { law, subject, reason })
    }

    fn cycle_members(&mut self, lead: &Token) -> Result<Vec<Token>, Fault> {
        self.block(lead)?;
        let mut out = Vec::new();
        while self.head().kind != Kind::Close {
            if self.head().kind == Kind::End {
                return Err(self.fail("`}` to close the cycle members"));
            }
            out.push(self.word("a cycle member path")?);
            self.endline()?;
            self.skip_blank();
        }
        let close = self.bump();
        if out.len() < 2 {
            return Err(
                self.at(&close, format!("a cycle needs at least two members, found {}", out.len()))
            );
        }
        Ok(out)
    }
}

/// Parse one `.zone` file, faulting with a caret on the first problem.
pub(super) fn parse(source: &str, file: &Path) -> Result<Tree, Fault> {
    let tokens = tokenize(source, file)?;
    Reader { source, tokens, at: 0 }.parse()
}
