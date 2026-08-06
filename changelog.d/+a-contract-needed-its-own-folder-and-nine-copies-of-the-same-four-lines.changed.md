A contract had to live in a `contract/` drawer, so a boundary tool asked every
package for a directory to hold one page — while the manifest, the formatter
config, and the CI config all sat at the root. And it had to say everything
itself: ten Zig kernels in one tree were ten copies of the same `root`,
`language`, and `facade`, which is not a cosmetic cost. Four lines nobody reads
are four lines nobody notices are wrong, and a repository-wide fact spelled
per-package will eventually disagree with itself in exactly one package and
pass.

A `.zone` file now governs the directory it sits in, so `acme/acme.zone`
is the whole layout. The drawer still resolves to the same anchor, because
contracts are checked in and a tool does not get to invalidate a repository's
on-disk shape to tidy its own rules; nothing downstream can tell the two apart,
and only `draft --write` picks a side. Since `.zone` is also BIND's extension
for DNS data and a contract now sits where a nameserver file might, identity
moved off the extension and onto the first declaration: a contract opens with
`package` or `workspace`, and a sweep walks past any `*.zone` that opens with
something else — a claim of authorship rather than a guess about content, and
only for sweeping, since a file named on the command line is still parsed and
still faults.

The shared part is now sayable once. A `workspace { member … }` block claims
packages below it and hands down `root`, `language`, `facade`, `use` grants, and
the reach ceiling; a member's own word always wins, and a member that agrees
with all of it writes `package hearth` with no block at all. The link points
down, the way `[workspace] members` and `[tool.uv.workspace]` do — a package is
a member because something above it said so, never because it declared a parent,
so nothing can quietly attach itself to a policy nobody granted it. What a
workspace deliberately cannot share is anything naming a file: zones, seals,
keeps, and variances are claims about one graph, and a blanket exception written
once for a whole monorepo is the accretion this language exists to prevent.

An inherited grant is judged against the whole membership rather than each
member, or sharing a line would be strictly worse than repeating it — one grant
of nine members would fail eight times. It is stale only when no member
exercises it, the report names the workspace that wrote it, and a run that saw
only part of the membership (`--under`, or a sibling that would not parse) says
nothing at all: absence of evidence across an unknown remainder is not evidence.
