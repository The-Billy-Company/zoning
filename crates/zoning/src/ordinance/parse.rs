//! Tokens → parse tree. Recursive descent, one declaration per call.
//!
//! The grammar, in full:
//!
//! ```text
//! file     = ( package | workspace ) { decl } END
//! decl     = package | workspace | zones | seal | keep | use | limit | forbid | variance
//! package  = "package" WORD [ "{" BREAK { setting BREAK } "}" ] BREAK
//! setting  = "root" WORD | "language" WORD | "facade" paths | "exclude" paths
//! workspace = "workspace" "{" BREAK { shared BREAK } "}" BREAK
//! shared   = "member" paths | "root" WORD | "language" WORD | "facade" paths
//!          | use | limit
//! zones    = "zones" "{" BREAK { WORD paths BREAK } "}" BREAK
//! paths    = WORD... | "{" BREAK { WORD... BREAK } "}"
//! seal     = "seal" WORD "through" WORD [ "open" "to" paths ] BREAK
//! keep     = "keep" WORD "to" ( paths | "nobody" ) BREAK
//! use      = "use" WORD... [ "by" paths ] BREAK
//! limit    = "limit" "reach" "to" INT "hops" BREAK
//! forbid   = "forbid" "cycles" "across" "directories" BREAK
//! variance = "variance" ( edge | cycle )
//! edge     = LAW WORD "->" WORD "because" TEXT BREAK
//! cycle    = "cycle" "{" BREAK { WORD BREAK } "}" "because" TEXT BREAK
//! LAW      = "zone" | "seal" | "keep" | "cycle" | "reach" | "use" | "escape"
//! ```
//!
//! `keep` takes a single subject rather than a `paths` list so the inline form
//! stays unambiguous against its own `to` — and because one claim per line is what
//! lets each keep carry the comment that justifies it.
//!
//! `use` is the one declaration whose subjects are not paths: a module name the
//! build system resolves, not a file this package owns. Its `by` list is scopes —
//! zone names, or path globs where a grant is narrower than a whole zone.
//!
//! Zones are listed low to high, and that vertical order *is* the stack: reading
//! down the block is reading up the architecture. `because` sits in the grammar
//! rather than in a validation pass because an unexplained exception is the thing
//! this language most wants to make unsayable.
//!
//! A file leads with `package` or `workspace` — what it governs, before anything it
//! says about it. That reads better than the alternative, and it is also how a sweep
//! recognises a contract at a glance now that one may sit anywhere in a tree rather
//! than in a `contract/` drawer: `.zone` is an extension BIND has used for DNS since
//! long before this tool, and identity has to be cheaper than parsing.
//!
//! Everything a member inherits lives *inside* the `workspace` block. A file may
//! declare a package and a workspace at once — a root package with members below it —
//! and then the two blocks must not be able to leak into each other: what the members
//! share is indented under `workspace`, and what the file's own package is stays
//! outside it.

use std::path::Path;

use super::fault::{Fault, Span};
use super::law::Law;
use super::lex::{Kind, Token, tokenize};

/// The `package { … }` header: what this file governs.
pub(super) struct Package {
    pub name: Token,
    pub root: Option<Token>,
    pub language: Option<Token>,
    pub facade: Vec<Token>,
    pub exclude: Vec<Token>,
}

/// The `workspace { … }` block: which packages hang off this file, and what they all
/// are unless one of them says otherwise.
///
/// The settings here are exactly the ones that are a fact about a *set* of packages —
/// what language they are written in, where each keeps its source, what fronts it, what
/// they may reach outside themselves, how far an import may climb. What a member cannot
/// inherit is anything naming its own files: zones, seals, keeps, and variances are
/// claims about one graph, and a blanket exception written once for a whole monorepo is
/// the accretion this language exists to prevent.
pub(super) struct Workspace {
    pub lead: Token,
    pub members: Vec<Token>,
    pub root: Option<Token>,
    pub language: Option<Token>,
    pub facade: Vec<Token>,
    pub uses: Vec<Use>,
    pub reach: Option<(u32, Span)>,
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

/// One `keep REGION to IMPORTERS…`. An empty list is `to nobody`, written out.
pub(super) struct Keep {
    pub subject: Token,
    pub importers: Vec<Token>,
}

/// One `use MODULE… [by SCOPE…]`. An empty `by` list grants every zone.
pub(super) struct Use {
    pub modules: Vec<Token>,
    pub scope: Vec<Token>,
    /// The `use` keyword itself, for locating a fault or a stale grant.
    pub lead: Token,
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
    /// The package it governs, absent in a file that only holds a workspace together.
    pub package: Option<Package>,
    /// The members hanging off it, and what they share.
    pub workspace: Option<Workspace>,
    pub zones: Vec<Zone>,
    pub seals: Vec<Seal>,
    pub keeps: Vec<Keep>,
    pub uses: Vec<Use>,
    pub variances: Vec<Variance>,
    pub reach: Option<(u32, Span)>,
}

/// Spellings that changed when `ward` became `zoning`, and what they became.
/// A contract written against the old tool should say so, not fail as gibberish.
const RENAMED: [(&str, &str); 3] = [("tiers", "zones"), ("tier", "zone"), ("allow", "variance")];

/// What a file may open with: what it governs, before anything it says about it.
const OPENERS: [&str; 2] = ["package", "workspace"];

/// Every declaration, once the file has said what it governs.
const DECLARATIONS: [&str; 9] =
    ["package", "workspace", "zones", "seal", "keep", "use", "limit", "forbid", "variance"];

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
        let mut workspace = None;
        let mut zones = Vec::new();
        let mut seen_zones = false;
        let (mut seals, mut keeps, mut uses) = (Vec::new(), Vec::new(), Vec::new());
        let mut variances = Vec::new();
        let mut reach = None;

        self.skip_blank();
        let mut opened = false;
        while self.head().kind != Kind::End {
            let lead = self.keyword(if opened { &DECLARATIONS } else { &OPENERS })?;
            opened = true;
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
                "workspace" => {
                    if workspace.is_some() {
                        return Err(self.at(
                            &lead,
                            "a second `workspace` block — merge them so one list names \
                             every member",
                        ));
                    }
                    workspace = Some(self.workspace(&lead)?);
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
                "use" => uses.push(self.uses(lead)?),
                "limit" => reach = Some(self.limit()?),
                "forbid" => self.forbid()?,
                _ => variances.push(self.variance()?),
            }
            self.skip_blank();
        }

        let head = self.tokens[0].clone();
        if package.is_some() && zones.is_empty() {
            return Err(
                self.at(&head, "no `zones` block — a contract with no zones governs nothing")
            );
        }
        // A declaration naming files, in a file that governs no package of its own, has
        // nothing to name. Members do not inherit these, so quietly ignoring them would
        // let a whole monorepo look sealed while nothing was.
        if package.is_none() {
            let local = [
                ("zones", !zones.is_empty()),
                ("seal", !seals.is_empty()),
                ("keep", !keeps.is_empty()),
                ("use", !uses.is_empty()),
                ("limit", reach.is_some()),
                ("variance", !variances.is_empty()),
            ];
            if let Some((word, _)) = local.into_iter().find(|(_, present)| *present) {
                return Err(self.at(
                    &head,
                    format!(
                        "`{word}` outside a `package` block — this file holds a workspace \
                         together and governs no package of its own. What every member \
                         shares goes inside `workspace {{ … }}`; what names one package's \
                         own files belongs in that package's contract"
                    ),
                ));
            }
        }
        Ok(Tree { package, workspace, zones, seals, keeps, uses, variances, reach })
    }

    /// `workspace { member … }` — the greater document a member's contract hangs off.
    fn workspace(&mut self, lead: &Token) -> Result<Workspace, Fault> {
        self.block(lead)?;
        let mut out = Workspace {
            lead: lead.clone(),
            members: Vec::new(),
            root: None,
            language: None,
            facade: Vec::new(),
            uses: Vec::new(),
            reach: None,
        };
        while self.head().kind != Kind::Close {
            if self.head().kind == Kind::End {
                return Err(self.fail("`}` to close the `workspace` block"));
            }
            let word = self.keyword(&["member", "root", "language", "facade", "use", "limit"])?;
            // `use` and `limit` are whole statements and eat their own line end; the
            // settings are one value each and do not.
            match word.text.as_str() {
                "member" => out.members.extend(self.paths("a member directory or glob")?),
                "root" => out.root = Some(self.word("the source root directory")?),
                "language" => out.language = Some(self.word("the language name")?),
                "facade" => out.facade.extend(self.paths("a facade path")?),
                "use" => {
                    out.uses.push(self.uses(word)?);
                    self.skip_blank();
                    continue;
                }
                _ => {
                    out.reach = Some(self.limit()?);
                    self.skip_blank();
                    continue;
                }
            }
            self.endline()?;
            self.skip_blank();
        }
        let close = self.bump();
        self.endline()?;
        if out.members.is_empty() {
            return Err(self.at(
                &close,
                "a `workspace` block with no `member` — name the packages that hang off \
                 this file, or the block governs nothing",
            ));
        }
        Ok(out)
    }

    fn package(&mut self, lead: &Token) -> Result<Package, Fault> {
        let name = self.word("the package name")?;
        let (mut root, mut language) = (None, None);
        let (mut facade, mut exclude) = (Vec::new(), Vec::new());
        // The block is optional because a member of a workspace can have nothing to put
        // in it, and a mandatory empty `{ }` is precisely the boilerplate the workspace
        // was written to delete.
        if self.head().kind != Kind::Open {
            self.endline()?;
            return Ok(Package { name, root, language, facade, exclude });
        }
        self.block(lead)?;
        while self.head().kind != Kind::Close {
            if self.head().kind == Kind::End {
                return Err(self.fail("`}` to close the `package` block"));
            }
            match self.keyword(&["root", "language", "facade", "exclude"])?.text.as_str() {
                "root" => root = Some(self.word("the source root directory")?),
                "language" => language = Some(self.word("the language name")?),
                "facade" => facade.extend(self.paths("a facade path")?),
                _ => exclude.extend(self.paths("a path to exclude")?),
            }
            self.endline()?;
            self.skip_blank();
        }
        self.bump();
        self.endline()?;
        Ok(Package { name, root, language, facade, exclude })
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

    /// `keep REGION to IMPORTERS…`, or `to nobody` for a region with no way in.
    ///
    /// `nobody` is spelled rather than expressed as an empty list because a guest
    /// list nobody is on is a strong claim — the region is reachable only from
    /// inside itself — and it should read like one instead of looking like a line
    /// somebody forgot to finish.
    fn keep(&mut self) -> Result<Keep, Fault> {
        let subject = self.word("the region to keep")?;
        self.keyword(&["to"])?;
        if self.looking_at(&["nobody"]) {
            self.bump();
            self.endline()?;
            return Ok(Keep { subject, importers: Vec::new() });
        }
        let importers = self.paths("an importer allowed to reach it, or `nobody`")?;
        self.endline()?;
        Ok(Keep { subject, importers })
    }

    /// `use MODULE… [by SCOPE…]` — what this package may reach outside itself.
    ///
    /// Several modules may share one line because they usually share one reason
    /// (`use ledger hyper by face`), and an omitted `by` grants every zone, which is
    /// the common case and should cost one word.
    fn uses(&mut self, lead: Token) -> Result<Use, Fault> {
        let mut modules = vec![self.word("an outside module name")?];
        while self.head().kind == Kind::Word && self.head().text != "by" {
            modules.push(self.bump());
        }
        let mut scope = Vec::new();
        if self.looking_at(&["by"]) {
            self.bump();
            scope = self.paths("a zone name or path glob the grant covers")?;
        }
        self.endline()?;
        Ok(Use { modules, scope, lead })
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
