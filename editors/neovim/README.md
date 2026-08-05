# Neovim compatibility

The canonical shipped runtime is `../vim`; it is intentionally shared without
symlinks or duplicated generated files. Install that directory as a Neovim
package. It contains the Neovim 0.11 `lsp/zoning.lua` configuration and
auto-enables `zoning lsp --stdio`, while retaining the same syntax, indent,
folding, and terminal-icon behavior in Vim.
