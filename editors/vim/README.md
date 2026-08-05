# Zoning for Vim and Neovim

A runtime-path plugin for `.zone` files:

- filetype detection, syntax, comments, indentation, and expression folding
- Neovim 0.11 native LSP auto-enable through `lsp/zoning.lua`
- adapters for vim-lsp, vim-lsc, ALE, and coc.nvim when detected
- optional `nvim-web-devicons` registration

Install `zoning` separately and keep it on the editor's `PATH`; this package
does not contain the executable. Override the command before startup with:

```vim
let g:zoning_executable = '/absolute/path/to/zoning'
```

Install this directory with any runtime-path package manager, or copy it to:

- Vim: `~/.vim/pack/zoning/start/zoning`
- Neovim: `~/.local/share/nvim/site/pack/zoning/start/zoning`

Neovim 0.11 automatically runs `zoning lsp --stdio`. With older Neovim or Vim,
an installed supported LSP client is registered at `VimEnter`.

## Terminal icon

Terminal Vim does not render SVG. `zoning#icon()` and the optional
`nvim-web-devicons` adapter use the selected terminal mapping:

```vim
" Default one-cell Unicode fallback: ≡
let g:zoning_nerd_font = 1       " Nerd Font layers glyph: 󰙅
let g:zoning_ascii_icon = 1      " Width-safe ASCII fallback: [=]
```

`g:zoning_nerd_font_icon` can replace the Nerd Font glyph without patching the
runtime.

## Smoke test

```sh
vim -Nu NONE -n -es -S test/runtime.vim
nvim --clean --headless -S test/runtime.vim
```
