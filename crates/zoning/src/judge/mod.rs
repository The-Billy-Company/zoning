//! The bench: six laws, judged against a resolved survey.
//!
//! Each law catches something a compiler structurally cannot:
//!
//! * **zone** — no visibility rules inside a package means any file may name any
//!   other file's path, so stack direction is convention with nothing behind it.
//! * **seal** — a directory is the unit of "deep module", but the language gives it
//!   no boundary: an outsider reaches past `rank.zig` into `rank/signals.zig` and
//!   the build is just as happy.
//! * **keep** — zones order the stack but say nothing about peers, so two siblings
//!   at the same height may quietly grow a dependency on each other.
//! * **cycle** — lazy analysis means a genuine import cycle *compiles*. Nothing to
//!   notice until the module is unsplittable.
//! * **reach** — a five-hop `../../../../../` resolves fine and says, quietly, that
//!   the file's physical home disagrees with its logical one.
//! * **escape** — a path climbing out of the module root is a dependency the build
//!   cannot follow.
//!
//! Anything a contract still claims but the tree no longer contains — a stale
//! variance, a zone matching no file, an `exclude` holding nothing back — lands in
//! [`Verdict::stale`], which no variance can silence. That is what makes the
//! exception list shrink as debt is paid, instead of accreting as folklore.

mod census;
mod cycle;
mod law;

use std::collections::HashSet;

pub use census::Census;
// Path predicates the laws reason with. The map draws the same distinctions, and
// two answers to "is this file inside that directory" is one answer too many.
pub(crate) use law::inside;

use crate::ordinance::{Law, Ordinance};
use crate::survey::Survey;

/// One violation, located.
#[derive(Clone)]
pub struct Finding {
    /// Which law was broken.
    pub law: Law,
    /// Repo-relative path, for a report line an editor can open.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// What went wrong, and why it matters.
    pub message: String,
    /// The exact string a `variance` must name to ratify this.
    pub subject: String,
}

/// What the bench found.
pub struct Verdict {
    /// The package that was judged.
    pub package: String,
    /// Violations nobody has ratified.
    pub findings: Vec<Finding>,
    /// Violations a variance excuses, with the reason it gave.
    pub ratified: Vec<(Finding, String)>,
    /// Declarations the tree no longer supports. No variance can silence these.
    pub stale: Vec<String>,
    /// Advisory cartography: what the contract could govern but does not yet.
    pub census: Census,
}

impl Verdict {
    /// Did the package pass?
    #[must_use]
    pub fn ok(&self) -> bool {
        self.findings.is_empty() && self.stale.is_empty()
    }
}

/// Judge one surveyed module against its ordinance.
#[must_use]
pub fn judge(survey: &Survey, ordinance: &Ordinance) -> Verdict {
    let mut bench = Bench {
        survey,
        ordinance,
        findings: Vec::new(),
        ratified: Vec::new(),
        stale: Vec::new(),
        used: HashSet::new(),
    };

    bench.zones();
    bench.seals();
    bench.keeps();
    bench.cycles();
    bench.reach();
    bench.escapes();

    // A variance nobody needed is permission outliving the code it was written for.
    let unused: Vec<String> = ordinance
        .variances
        .iter()
        .filter(|v| !bench.used.contains(&(v.law, v.subject.clone())))
        .map(|v| format!("variance {} {}  ({}: {})", v.law, v.subject, v.source, v.reason))
        .collect();
    bench.stale.extend(unused);

    // An exclude is the widest exception in the language — the file leaves the
    // judged set entirely — so it is the one that must not be allowed to linger.
    let file =
        ordinance.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let idle: Vec<String> = ordinance
        .exclude
        .iter()
        .filter(|p| !survey.spent.contains(p.as_str()))
        .map(|p| format!("exclude {p}  ({file}: holds back no file)"))
        .collect();
    bench.stale.extend(idle);

    Verdict {
        package: ordinance.package.clone(),
        census: census::take(survey, ordinance),
        findings: bench.findings,
        ratified: bench.ratified,
        stale: bench.stale,
    }
}

/// The judging in progress: everything a law needs, and the record it writes to.
struct Bench<'a> {
    survey: &'a Survey,
    ordinance: &'a Ordinance,
    findings: Vec<Finding>,
    ratified: Vec<(Finding, String)>,
    stale: Vec<String>,
    used: HashSet<(Law, String)>,
}

impl Bench<'_> {
    /// Record a violation, routing it through any variance that ratifies it.
    fn record(&mut self, law: Law, path: &str, line: usize, message: String, subject: String) {
        let finding =
            Finding { law, path: self.survey.rel(path), line, message, subject: subject.clone() };
        match self.ordinance.variance(law, &subject) {
            Some(variance) => {
                self.used.insert((law, subject));
                self.ratified.push((finding, variance.reason.clone()));
            }
            None => self.findings.push(finding),
        }
    }

    /// Record a violation no variance can excuse.
    ///
    /// A file no zone claims, or one that two zones claim, is not an exception to
    /// the law — it is the law having nothing to say. Ratifying that would ratify
    /// the hole rather than the crossing.
    fn unwaivable(&mut self, law: Law, path: &str, message: String, subject: String) {
        self.findings.push(Finding { law, path: self.survey.rel(path), line: 1, message, subject });
    }
}
