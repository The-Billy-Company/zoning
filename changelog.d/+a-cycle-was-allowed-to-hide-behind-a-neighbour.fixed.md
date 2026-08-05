The cycle law was blind to most real cycles, and a property test found it in about a
second. `forbid cycles across directories` withheld same-directory imports from the graph
*before* searching it, which severs any tangle whose trip home goes through a neighbour:
`a/one.zig -> a/two.zig -> b/three.zig -> a/one.zig` binds `a` and `b` into one
indivisible unit exactly as tightly as a two-file cycle does, and zoning reported nothing.
Crossing a boundary is a property of the cycle, not of the individual imports in it, so
the filter moved off the edges and onto the component: search the whole graph, then keep
the components that bind more than one module. A cycle wholly inside a directory - or
inside a directory and the door file named for it - is still that module's own business.

`draft` had been contradicting itself in one breath because of this. It would merge two
directories into a single zone with the note "these 2 directories import each other, so no
order separates them", then close with "Nothing else to declare: this graph is already a
stack". The zone stack reads the directory graph and saw the tangle; the tangle detector
dropped the edge that proved it. Now a draft over a graph with a real cycle emits the
variance stanza with an empty reason, which does not parse, which is the whole point.

Adopting this is not free, and it should not be: it turns previously-silent tangles into
findings. In our own trees it surfaced five in a 310-file package - two nobody knew about,
and three that existing variances had described with too few members - and one in another.
Every one of them was real before this release; the tool simply could not see it. A
variance whose member list is now short fails as stale rather than passing quietly, so the
contract gets corrected rather than left subtly wrong.
