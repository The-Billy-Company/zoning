# Oracle cases

One tiny package per disagreement. Each box holds a `case.json` and a `tree/`,
and the `tree/` is a whole package: a `.zone` contract, plus `.importlinter` and
`tach.toml` wherever the rule is expressible in those languages too.

The rule for writing one: **the expectation comes from the language, not from a
tool.** `expect` is what Python does when the import runs, cited in `spec`. A
case captured from our own output could never catch us being wrong, and that is
the only thing this rung is for.

| Case | Expect | Asked of | Turns on |
|---|---|---|---|
| `ancestor-init` | violation | zoning | importing a leaf runs its package's `__init__` |
| `self-import` | clean | zoning | the same rule must not fire on a file's own package |
| `relative-climb` | violation | all three | `from ..edge.thing import upper` is still an upward edge |
| `type-checking` | violation | all three | a dependency that exists in the source and not at runtime |
| `dynamic` | violation | all three | `importlib.import_module` — the blind spot everybody shares |

Two fields shape how a case is scored:

- **`gate`** — `false` marks a case whose correct answer is contested, or one no
  static reader can be expected to catch. It is measured and reported and never
  fails the run. Gating on `dynamic` would only pressure someone into deleting
  the case, which is worse than knowing the limit is there.
- **`tools`** — who can be asked at all. Where a rival is missing from the list,
  `unexpressible` says why in that rival's own terms. Both remaining cases are
  file-granular: `import-linter` and `tach` judge modules, so a guest list that
  admits `acme.deep.leaf` while refusing `acme/deep/__init__.py` has no spelling
  in either config language. Reporting that as a rival declining to answer would
  be a lie in our favour, so it is reported as `n/a` and explained here.

The two zoning-only boxes are a matched pair, and that is deliberate:
`ancestor-init` fails if the resolver ignores parent `__init__` files, and
`self-import` fails if it counts the one the importing file already lives in.
Either alone is easy to pass by being wrong in the other direction.
