Every release here so far was tagged, released, and relabelled by a person,
because `release-please-config.json` named the package and that one line kept the
bot from ever cutting one.

With `include-component-in-tag` off, release-please writes a standalone release
PR's body with no component in it, and names the branch
`release-please--branches--main` with no component either. Then, on merge, before
it will tag anything, it compares that empty component against
`component || package-name` - so a `package-name` here makes the two halves of
its own bookkeeping disagree permanently. Every merge logged `PR component:
undefined does not match configured component: zoning` and returned without
creating the tag or the release. That is worse than a missed release, because it
wedges: an untagged merged release PR makes the *next* run abort before it opens
anything, so the queue stops until someone relabels the old PR by hand - which is
where 1.3.1 has been sitting since August, tagged but never released.

The three maintenance steps on the release PR - the changelog fold, the VS Code
payload refresh, and the `Cargo.lock` re-pin - only ran on the push where
release-please rewrote the PR. The action sets its `pr` output only when it wrote
something, so a `ci`/`docs` commit carrying a new fragment, which changes no
version and therefore no note, left that output empty and skipped all three with
nothing saying so. A PR could sit open with fragments unfolded, a stale VSIX, and
a lock a version behind, and the first complaint would come from preflight
refusing to publish it. The branch is now resolved from the
`autorelease: pending` label instead, which is release-please's own marker for
the PR it is holding open rather than a name guessed from a convention.

With `always-update` on, the branch is rebuilt on every push while the PR is
open, so the fold recomputes from main rather than appending to whatever the
branch already carries - towncrier treats a second write of the same version as a
hard error, not a no-op.
