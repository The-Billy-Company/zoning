zoning shipped without the governance and hygiene layer its sibling repos (`gist`, `relate`,
`blast`, `irregex`) already carry: no `CODE_OF_CONDUCT.md`, `SECURITY.md`, or
`CONTRIBUTING.md`, no issue or pull-request templates or `CODEOWNERS`, no
`labels.json`/`triage.py` triage automation, and no `.typos.toml` / `.taplo.toml` /
`.yamllint` / `.editorconfig-checker.json` / `.mise.toml` to hold the parts of the tree that
aren't Rust to the same bar as the parts that are. Filing against this repo meant a
different experience than filing against a sibling for no reason but that nobody had
written the second one down yet.

All of it is written now, specific to what zoning actually is rather than copied verbatim.
`SECURITY.md`'s threat model covers a false verdict, glob semantics silently diverging from
the CPython contract they're specified against, and the in-process LSP server this crate now
ships (`zoning lsp --stdio`) - not a generic supply-chain section a zero-dependency crate has
no surface to justify. `labels.json` carries zoning's own `area/*` taxonomy (`cli`, `lsp`,
`contract`, `editors`, `ci`, `docs`, `build`) mapped onto its real module tree, alongside the
`size/*`/`status/*`/`type/*` rows kept byte-identical with every sibling so `triage.py peers`
still holds across all of them. A new `discipline` CI job runs markdownlint, typos,
yamllint, taplo, editorconfig-checker, ruff, and shellcheck over everything `cargo test`
never touches, and it is a `release-ready` dependency exactly like `check` or `dogfood` - a
tag can no longer ship with a broken paper trail any more than it can ship with a failing
test.
