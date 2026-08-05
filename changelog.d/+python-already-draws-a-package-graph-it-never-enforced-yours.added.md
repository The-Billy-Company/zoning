zoning could only read Zig, and Zig's total absence of a module system was the
whole reason the tool exists. Python is a different animal: `import a.b.c` and
`from a.b import c` already resolve dotted names against real files, and
`sys.path` already draws a boundary. What that graph has never said is which
of *your* packages may reach which — the same question a zone stack answers,
at a grain the language's own boundary is too coarse to draw.

The new `python` dialect reads that graph on its own terms rather than
borrowing Zig's: dotted names resolve to a leaf module or a package's
`__init__.py`, a relative import's leading dots climb exactly as many
directories as they count regardless of how deep the `from` nests, and
`pyproject.toml` supplies both the package's declared name and its
`path = "…"` vendored dependencies, the same authority `build.zig.zon` carries
for a vendored Zig one. The standard-library grant is drawn from
`sys.stdlib_module_names` unioned across every CPython release still in
support rather than one interpreter's local answer, so a contract written
against 3.12 keeps meaning the same thing under 3.13 or 3.14 without an
edit.
