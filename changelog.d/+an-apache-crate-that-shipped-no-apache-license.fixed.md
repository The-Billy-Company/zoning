The published crate declared Apache-2.0 and carried none of it. `readme` reaches up
to the repository's own `README.md` and Cargo relocates that one file into the
tarball, which made the omission easy to miss: nothing else above `crates/zoning/`
travels, so the license text and the NOTICE did not. Section 4 of that license asks
a redistributor for exactly those two files.

Both are now committed in `crates/zoning/`, byte-identical to the root pair, and the
tarball carries them.
