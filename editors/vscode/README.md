# Zoning for Cursor and VS Code

Language support for `.zone` architecture contracts:

- TextMate highlighting and editor-aware comments, braces, and indentation
- diagnostics, completions, hover, symbols, references, formatting, rename,
  folding, and code actions from `zoning lsp --stdio`
- **Zoning: Set Up Editor Integrations** and **Zoning: Show Status** commands

## Requirement

Install `zoning` separately and make it available on the editor's `PATH`.
The VSIX deliberately does not contain the executable. If the editor uses a
different environment, set `zoning.executablePath` to its absolute path.

## Build and package

```sh
npm ci
npm test
npm run package
```

`npm run package` creates `artifacts/zoning-<version>.vsix`. The bundle is
minified without source maps or timestamps, and packaging excludes source,
tests, build tools, dependencies, and the zoning executable.

`npm test` runs three suites over the contributions themselves, none of which
needs VS Code running:

| Suite | What it holds |
| --- | --- |
| `tests/tokens.test.mjs` | the real TextMate scopes, tokenized through `vscode-textmate` and `vscode-oniguruma` the way the editor does - so a keyword painted inside a comment, or a law scoped as a statement, is a failed assertion rather than something you notice in a screenshot |
| `tests/language.test.mjs` | `language-configuration.json`'s comment, bracket, word, and indentation rules, each exercised as the regex the editor compiles |
| `tests/manifest.test.mjs` | every path `package.json` contributes exists, the grammar's scope agrees with the language id, and the activation events and settings are the ones the extension reads |
