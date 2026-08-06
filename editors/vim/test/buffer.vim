" What editing a claimed buffer feels like: the options, the indent, and the folds.
"
" The indent and fold expressions are checked twice over - once by calling them, which says
" what the function decided, and once by letting Vim run them through `=` and `foldlevel()`,
" which says what the user gets. They can disagree: an expression can be right while
" `indentkeys` never fires it, and that difference is exactly what a user reports as
" "indenting is broken".

source <sfile>:p:h/harness.vim
call Load()

call Open(['package demo {', 'root src', '}', '', 'zones {', 'core core/**', '}'])

call Is(&filetype, 'zoning', 'the buffer is zoning')
call Is(&commentstring, '// %s', 'gc and friends comment with //')
call Is(&comments, 'b://', '// needs the space after it that `b:` requires')
call Is(&indentexpr, 'zoning#indent(v:lnum)', 'the indent expression is wired')
call Is(&foldexpr, 'zoning#fold(v:lnum)', 'the fold expression is wired')
call Is(&foldmethod, 'expr', 'folding is by expression, not by indent')
call Is(&suffixesadd, '.zone', 'gf on a bare package name finds its contract')
call Ok(&autoindent, 'autoindent carries the previous line while typing')
for s:char in ['-', '/', '.', '*']
  call Ok(stridx(&iskeyword, s:char) >= 0,
        \ 'a glob character `' . s:char . '` is part of a word, so `*` selects a whole path')
endfor
call Ok(!empty(get(b:, 'undo_ftplugin', '')),
      \ 'the ftplugin can be undone when the filetype changes')
call Ok(!empty(get(b:, 'undo_indent', '')), 'the indent plugin can be undone too')

" The expression, asked directly.
call Is(zoning#indent(2), shiftwidth(), 'a line after `package demo {` indents one level')
call Is(zoning#indent(3), 0, 'the closing brace returns to the margin')
call Is(zoning#indent(4), 0, 'a blank line after `}` stays at the margin')
call Is(zoning#indent(6), shiftwidth(), 'a line inside `zones {` indents one level')

" The fixtures below are written with four-space indents, so pin `shiftwidth` to match:
" otherwise the expected level and the fixture's own indentation are two different rulers.
function! s:IndentOf(lines, lnum) abort
  call Open(a:lines)
  setlocal shiftwidth=4 expandtab
  return zoning#indent(a:lnum)
endfunction

call Is(s:IndentOf(['workspace {', 'member */'], 2), shiftwidth(),
      \ '`workspace {` opens a block like any other brace')
call Is(s:IndentOf(['zones {', '    core core/**', '    keep core {', 'nobody'], 4),
      \ shiftwidth() * 2, 'a nested `keep` block indents from its own line, not the margin')
call Is(s:IndentOf(['variance cycle a -> b because', 'the reason'], 2), shiftwidth(),
      \ 'a reason continues indented under `because`')
call Is(s:IndentOf(['zones { // a trailing comment', 'core core/**'], 2), shiftwidth(),
      \ 'a comment after the brace does not hide the brace')

" The same expressions, run by Vim.
call Open(['package demo {', 'root src', '}'])
setlocal shiftwidth=4 expandtab
normal! gg=G
call Is(getline(2), '    root src', '`=G` reindents the body')
call Is(getline(3), '}', '`=G` leaves the closing brace at the margin')

call Open(['package demo {', 'root src', '}', '', 'zones {', 'core core/**', '}'])
call Is(foldlevel(1), 1, '`package demo {` opens a fold')
call Is(foldlevel(2), 1, 'the body is inside it')
call Is(foldlevel(3), 1, 'the closing brace ends it')
call Is(foldlevel(4), 0, 'the blank line between blocks is outside every fold')
call Is(foldlevel(6), 1, 'the second block folds too')

" A trailing comment is stripped before the brace is looked for, so the fold still opens.
" The fixture has to lead with `package`, because otherwise the detector - correctly -
" never claims the buffer and there is no fold expression to grade.
call Open(['package demo', '', 'zones { // fold me', 'core core/**', '}'])
call Is(foldlevel(3), 1, 'a commented brace still opens a fold')
call Is(foldlevel(2), 0, 'and the blank line before it is still outside')

call Report('buffer')
