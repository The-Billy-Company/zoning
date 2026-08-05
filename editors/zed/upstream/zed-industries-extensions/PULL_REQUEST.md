# Zoning language support

Adds `.zone` architecture-contract support from
`https://github.com/The-Billy-Company/zoning-zed`.

- Tree-sitter grammar with highlighting, outline, bracket, and indentation queries
- `zoning lsp --stdio` through the user's `PATH`
- no bundled or downloaded language-server executable
- language-only package; the file icon is a separate default-icon-theme proposal

## Before submitting

- [ ] Mirror `editors/zed` at the root of `zoning-zed`.
- [ ] Pin `[grammars.zoning].rev` to the mirror's immutable commit SHA.
- [ ] Point the mirrored manifest's `repository` field at `zoning-zed`.
- [ ] Run `npm ci && npm test` in `grammar/`.
- [ ] Run `cargo check --target wasm32-wasip1`.
- [ ] Install the mirror as a Zed dev extension and open a `.zone` fixture.
- [ ] Add the submodule at `extensions/zoning`.
- [ ] Add `extension-entry.toml` to the registry's `extensions.toml`.
- [ ] Run the registry's sort, test, and package commands.
- [ ] Sign the Zed CLA.
