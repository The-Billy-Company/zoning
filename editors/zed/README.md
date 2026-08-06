# Zoning for Zed

This extension recognizes `.zone`, compiles the bundled Tree-sitter grammar,
provides highlighting, outline, bracket, and indentation queries, and launches
the separately installed language server as:

```text
zoning lsp --stdio
```

`zoning` must resolve through Zed's worktree `PATH`. The extension never ships
or downloads the executable.

## Registry-install constraint

Zed's registry installs an extension from the root of its own Git repository;
it cannot publish `editors/zed` directly from this monorepo subdirectory. The
release process must mirror this directory to a dedicated repository (the
upstream templates use `The-Billy-Company/zoning-zed`) and replace the grammar
`rev = "main"` development reference with that mirror's immutable commit SHA.
Do not submit the language extension until both conditions hold.

Zed also forbids combining a language extension with a theme or icon theme.
The `.zone` icon proposal therefore has a separate submission template under
`upstream/default-icon-theme/`; it is not part of this extension package.

## Local development

1. Install `zoning` and confirm `zoning lsp --stdio` starts.
2. Run `npm ci` in `grammar/`, then `./test/run.sh` from this directory.
3. Run `cargo check --target wasm32-wasip1`.
4. In Zed, use **Install Dev Extension** and select this directory.

`test/run.sh` regenerates the parser, runs the corpus and the highlight
annotations under `grammar/test/`, and then runs each of the four `.scm`
queries against `test/fixture.zone`. That last pass is the one worth having:
a query that no longer matches anything still compiles, and Zed's only symptom
is a buffer that stops being painted where the rule used to apply. The runner
requires every capture Zed reads to actually appear, and rejects a capture name
outside the closed set Zed's themes key off - a typo there is invisible by eye.

Zed downloads the WASI SDK when it compiles the grammar. The generated
Tree-sitter C parser is committed because it is the deterministic source Zed
actually compiles; regenerate it with ABI 14 after changing `grammar.js`.
