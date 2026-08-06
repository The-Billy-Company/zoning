//! The roll: which members of a workspace showed up, and what they left unused.
//!
//! Every other claim zoning makes is about one package, and the bench is built for
//! exactly that — one contract, one graph, one verdict. A shared grant is the one claim
//! that cannot fit there. `use httpx` written in a workspace is dead permission only when
//! *no* member imports httpx, and a member reading its own contract has no way to know
//! what the others do. So the bench sets inherited grants aside
//! ([`Verdict::dormant`](super::Verdict::dormant)) and the roll takes the intersection
//! once the membership has been through.
//!
//! It fails open, deliberately. A run that judged four of five members has not earned
//! the claim, so it makes none: [`Roll::short`] retires the whole question rather than
//! blaming a shared grant for a package nobody read.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Verdict;

/// Shared grants no member of one workspace exercises.
pub struct Dormant {
    /// The workspace file that granted them.
    pub workspace: PathBuf,
    /// How many members were judged to reach this answer.
    pub members: usize,
    /// The grants, as the workspace wrote them, sorted.
    pub grants: Vec<String>,
}

/// The membership roll of every workspace a run touched.
#[derive(Default)]
pub struct Roll {
    /// Per workspace file: members judged, and how many left each shared grant idle.
    seen: HashMap<PathBuf, (usize, HashMap<String, usize>)>,
    /// A member somewhere never reached the bench, so no membership is whole.
    ///
    /// One flag for the run rather than one per workspace, because an unread contract is
    /// unread: which workspace claimed it is exactly the fact that is missing.
    short: bool,
}

impl Roll {
    /// A roll with nobody on it yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one member's verdict in, under the workspace that claimed it.
    pub fn attend(&mut self, workspace: &Path, verdict: &Verdict) {
        let (members, idle) = self.seen.entry(workspace.to_path_buf()).or_default();
        *members += 1;
        for grant in &verdict.dormant {
            *idle.entry(grant.clone()).or_default() += 1;
        }
    }

    /// Record that a member went unjudged, retiring the question for this run.
    pub fn short(&mut self) {
        self.short = true;
    }

    /// Per workspace, the shared grants every judged member left unexercised.
    ///
    /// Empty when any member was missed — see [`Roll::short`].
    #[must_use]
    pub fn dormant(&self) -> Vec<Dormant> {
        if self.short {
            return Vec::new();
        }
        let mut found: Vec<Dormant> = self
            .seen
            .iter()
            .filter_map(|(workspace, (members, idle))| {
                let mut grants: Vec<String> = idle
                    .iter()
                    .filter(|&(_, count)| count == members)
                    .map(|(grant, _)| grant.clone())
                    .collect();
                grants.sort();
                (!grants.is_empty()).then(|| Dormant {
                    workspace: workspace.clone(),
                    members: *members,
                    grants,
                })
            })
            .collect();
        found.sort_by(|a, b| a.workspace.cmp(&b.workspace));
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::Census;

    /// A verdict carrying nothing but the dormant grants this test is about.
    fn member(dormant: &[&str]) -> Verdict {
        Verdict {
            package: "member".to_owned(),
            findings: Vec::new(),
            ratified: Vec::new(),
            stale: Vec::new(),
            dormant: dormant.iter().map(|&g| g.to_owned()).collect(),
            census: Census::default(),
        }
    }

    #[test]
    fn a_grant_one_member_exercises_is_not_dormant_for_the_workspace() {
        let shared = Path::new("libs/kernels/kernels.zone");
        let mut roll = Roll::new();
        roll.attend(shared, &member(&["use ledger", "use bugle"]));
        roll.attend(shared, &member(&["use bugle"]));

        let found = roll.dormant();
        assert_eq!(found.len(), 1, "one workspace answered");
        assert_eq!(found[0].grants, ["use bugle"], "ledger was exercised by the second member");
        assert_eq!(found[0].members, 2);
    }

    #[test]
    fn every_member_leaving_it_idle_is_what_makes_it_dead() {
        let shared = Path::new("kernels.zone");
        let mut roll = Roll::new();
        roll.attend(shared, &member(&["use bugle"]));
        roll.attend(shared, &member(&["use bugle"]));
        assert_eq!(roll.dormant()[0].grants, ["use bugle"]);
    }

    #[test]
    fn a_short_roll_makes_no_claim_at_all() {
        let shared = Path::new("kernels.zone");
        let mut roll = Roll::new();
        roll.attend(shared, &member(&["use bugle"]));
        roll.short();
        assert!(roll.dormant().is_empty(), "four of five members cannot convict a grant");
    }

    #[test]
    fn two_workspaces_are_tallied_apart() {
        let mut roll = Roll::new();
        roll.attend(Path::new("a/a.zone"), &member(&["use one"]));
        roll.attend(Path::new("b/b.zone"), &member(&["use two"]));
        let found = roll.dormant();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].workspace, Path::new("a/a.zone"), "sorted by file");
        assert_eq!(found[1].grants, ["use two"]);
    }
}
