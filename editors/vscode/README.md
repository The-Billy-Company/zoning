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
