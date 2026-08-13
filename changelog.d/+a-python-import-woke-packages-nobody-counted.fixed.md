`import a.mid.deep.leaf` binds one name and initializes three packages: Python runs
`a/mid/__init__.py`, then `a/mid/deep/__init__.py`, then the leaf. The Python dialect
read only the leaf, so every one of those ancestors was a dependency the importer
genuinely had and the contract could not see - and because zones are globs over paths
rather than module names, a contract could put `mid/__init__.py` in a zone above the
importer and pass clean. A stack no interpreter could honour, judged green.

An absolute import now also names each ancestor package it wakes. Only the ones the
importer does not already live under, which is the same reason a relative import
contributes none: nothing inside `mid/` runs until `mid/__init__.py` already has, so
reaching `mid.deep.leaf` from within `mid/` cannot be a new dependency on the root.
Counting those would have made the ordinary absolute self-import - `from mypkg.sub
import x`, written inside `mypkg`, with any non-empty `__init__.py` - a cycle through
the package root in every Python package alive.
