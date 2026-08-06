# Zoning for Vim and Neovim

A runtime-path plugin for `.zone` files:

- filetype detection, syntax, comments, indentation, and expression folding

`.zone` is BIND's extension too, so detection claims only the files that lead
with `package` or `workspace` — the same first declaration the tool identifies a
contract by. Anything else is left for the editor's own content detection, which
already calls a DNS zone file `bindzone` without this plugin naming it.

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

## Tests

```sh
./test/run.sh          # every suite below, in every Vim on this machine, as TAP
```

| Suite | What it holds |
| --- | --- |
| `test/detect.vim` | `ftdetect` claims `.zone` by name, extension, and shebang, and the terminal icon honors each of the three settings |
| `test/buffer.vim` | the `ftplugin` options, `indentexpr` on every block and continuation form, and `foldexpr` over nested braces |
| `test/syntax.vim` | the group at a given line and column, and what it links to - which is how a keyword painted inside a comment, or a law repainted as a statement, gets caught |
| `test/lsp.vim` | registration against each supported client - vim-lsp, vim-lsc, coc.nvim, ALE, and Neovim's native `vim.lsp` - through an autoload shim standing in for the plugin |

`run.sh` runs each suite under both `vim` and `nvim` when both are installed,
because the two disagree about `l:` scope at script level, where `:echo` goes
in silent mode, and whether an autoload function may live in the wrong file -
each of which this suite has already caught. A machine with neither installed
is a failure rather than a silent pass.
