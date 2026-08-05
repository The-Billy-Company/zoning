# Zoning editor adapters

These adapters teach editors the `.zone` language and connect them to the
`zoning lsp --stdio` server. They never bundle the `zoning` executable; install
it separately and keep it on the editor's `PATH`.

- `vscode/` - Cursor and Visual Studio Code extension
- `zed/` - Zed language extension and upstream submission material
- `vim/` - Vim runtime package, including Neovim 0.11 LSP support
- `neovim/` - symlink-free Neovim compatibility notes
