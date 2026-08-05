`draft` wrote `use module by ` — a `by` clause with nothing after it — whenever
an outside module was imported only from the facade and never from any file a
zone actually covers. The facade has no zone to scope a grant to, so the
scope list came out empty, and an empty scope is not a legal grant: `zoning
verify` on the file `draft --write` had just produced refused to parse it.

The grant is now unscoped whenever any of its imports come from the facade —
`use module`, no `by` — the same shape a person would write by hand for a
dependency that isn't any one zone's business.
