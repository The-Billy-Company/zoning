" What every piece of a contract is painted as.
"
" Asked of Vim's own syntax engine rather than of the syntax file: `synID()` at a position
" is what the user's colour scheme sees, so a rule that exists but is shadowed by a later
" one - the usual way a highlighter goes wrong - fails here.

source <sfile>:p:h/harness.vim
call Load()

" The syntax group Vim resolves at a position - the plugin's own name, before any link.
function! s:GroupAt(lnum, col) abort
  return synIDattr(synID(a:lnum, a:col, 1), 'name')
endfunction

" What a group is declared to link to, read back from `:highlight` rather than from the file,
" so a link a later `highlight clear` or a typo dropped shows up as missing.
function! s:LinksTo(group) abort
  redir => l:declared
  silent execute 'highlight ' . a:group
  redir END
  return matchstr(l:declared, 'links to \zs\S\+')
endfunction

" The column of the first occurrence of `needle` on line `lnum`, one-based for `synID`.
function! s:ColOf(lnum, needle) abort
  return stridx(getline(a:lnum), a:needle) + 1
endfunction

call Open([
      \ 'package demo {',
      \ '    root src',
      \ '    exclude "vendor/**"',
      \ '}',
      \ '',
      \ '// a comment about the layers',
      \ 'zones {',
      \ '    core core/**',
      \ '    engine engine/**',
      \ '}',
      \ '',
      \ 'seal engine through engine/mod.zig',
      \ 'keep core {',
      \ '    nobody',
      \ '}',
      \ 'limit reach 2 hops',
      \ 'forbid cycles across directories',
      \ 'variance cycle core -> engine because',
      \ '\\ the loader has to see the engine to load it',
      \ ])

call Is(&filetype, 'zoning', 'the buffer is zoning, so the syntax file loaded')
call Is(get(b:, 'current_syntax', ''), 'zoning', 'the syntax file claimed the buffer')

for [s:lnum, s:word] in [
      \ [1, 'package'], [2, 'root'], [3, 'exclude'], [7, 'zones'],
      \ [12, 'seal'], [12, 'through'], [13, 'keep'], [14, 'nobody'],
      \ [16, 'limit'], [16, 'reach'], [16, 'hops'],
      \ [17, 'forbid'], [17, 'cycles'], [17, 'across'], [17, 'directories'],
      \ [18, 'variance'], [18, 'because'],
      \ ]
  call Is(s:GroupAt(s:lnum, s:ColOf(s:lnum, s:word)), 'zoningKeyword',
        \ '`' . s:word . '` is a keyword')
endfor

call Is(s:GroupAt(6, 1), 'zoningComment', 'a `//` line is a comment and not a path')
call Is(s:GroupAt(6, s:ColOf(6, 'layers')), 'zoningComment', 'and stays one to the end of it')
call Is(s:GroupAt(3, s:ColOf(3, '"vendor')), 'zoningString', 'a quoted glob is a string')
call Is(s:GroupAt(8, s:ColOf(8, 'core/**')), 'zoningPath', 'a glob is a path')
call Is(s:GroupAt(16, s:ColOf(16, '2')), 'zoningNumber', 'a hop count is a number')
call Is(s:GroupAt(18, s:ColOf(18, '->')), 'zoningArrow', 'the arrow is an operator')
call Is(s:GroupAt(1, s:ColOf(1, '{')), 'zoningBrace', 'a brace is a delimiter')
call Is(s:GroupAt(19, s:ColOf(19, 'loader')), 'zoningFoldedReason',
      \ 'a folded reason reads as prose, not as code')
call Is(s:GroupAt(19, 1), 'zoningReasonMarker', 'the `\\` continuation marks itself')

" A law name is a variance subject and is painted as a type, not as a keyword, so the eye
" can tell `variance cycle` apart from `forbid cycles`.
call Is(s:GroupAt(18, s:ColOf(18, 'cycle ')), 'zoningLaw', 'the law a variance names is a type')

" Nothing inside a comment is anything else. This is the failure that looks like a working
" highlighter until you write the word `package` in a sentence.
call Open(['package demo', '', '// the package keyword and a core/** glob and 12 hops'])
for s:inside in ['package', 'core/**', '12']
  call Is(s:GroupAt(3, s:ColOf(3, s:inside)), 'zoningComment',
        \ '`' . s:inside . '` inside a comment is comment')
endfor

" A trailing comment does not swallow the code before it, which is the mirror of the same
" priority question and the way an over-eager fix to the above would go wrong.
call Open(['package demo', '', 'zones { // core/** is a keyword-looking glob', '}'])
call Is(s:GroupAt(3, s:ColOf(3, 'zones')), 'zoningKeyword', 'code before a trailing comment')
call Is(s:GroupAt(3, s:ColOf(3, '//')), 'zoningComment', 'and the comment after it')

" The colour contract: each group links to a standard group a scheme already styles, so the
" plugin ships no colors of its own.
for [s:group, s:standard] in [
      \ ['zoningComment', 'Comment'], ['zoningString', 'String'],
      \ ['zoningFoldedReason', 'String'], ['zoningReasonMarker', 'SpecialChar'],
      \ ['zoningNumber', 'Number'], ['zoningArrow', 'Operator'],
      \ ['zoningBrace', 'Delimiter'], ['zoningPath', 'Directory'],
      \ ['zoningKeyword', 'Keyword'], ['zoningLaw', 'Type'],
      \ ]
  call Is(s:LinksTo(s:group), s:standard, s:group . ' links to ' . s:standard)
endfor

call Report('syntax')
