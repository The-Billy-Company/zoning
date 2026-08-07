`zone list` names the package a contract declares rather than the file it was
written in.

The stem was a fine stand-in while every contract was named after its package,
because then the two were the same word. They are not the same word once a tree
adopts a convention: a fleet that calls every root contract `charter.zone` got a
column reading `charter  charter.zone`, which is the listing telling you what
you just read. The `package` block was always the authoritative name - it is
what a verdict header, a `--package` filter, and a workspace lookup all read -
so the listing now reads it too, and falls back to the stem only for a contract
too malformed to parse, where a name is the one thing left to salvage.

The README also writes the naming convention down, under `What To Call It`: one
name at a repository root, a role name for a package nested inside a bigger tree
(`kernel.zone`, `service.zone`), and nothing enforced either way. A tool that
dictated filenames would be back to demanding a directory.
