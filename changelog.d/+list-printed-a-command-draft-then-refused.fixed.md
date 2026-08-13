`list` prints the exact `draft` invocation for each ungoverned package so the middle
line of adoption is a paste rather than a guess. For a Python package it printed a
command that then failed: `list` knew the dialect from the manifest it had just read,
`draft` did not read one at all, and under the default dialect it found no `build.zig`,
concluded the directory was not a package, and refused - naming the package it was
refusing as an empty path, so the message read with a word missing.

`draft` now takes the language from the manifest sitting in the directory it was
pointed at, which is what `--language` always meant: the language for packages that do
not name one. Nothing needs the flag to draft a package that declares itself. The
drafted header also no longer tells a Python package it was read from an `@import`
graph.
