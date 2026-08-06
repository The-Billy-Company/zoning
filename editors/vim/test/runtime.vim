set nomore
" An installed copy of this plugin lives under `packpath` once `zone setup` has
" run, and it would shadow the tree under test — so the suite would grade the
" last install rather than the working copy.
set packpath=
execute 'set runtimepath^=' . fnameescape(fnamemodify(expand('<sfile>'), ':p:h:h'))
filetype plugin indent on
syntax enable
runtime plugin/zoning.vim
if has('nvim')
  runtime plugin/zoning.lua
endif

execute 'edit ' . fnameescape(tempname() . '.zone')
call setline(1, ['package demo {', 'root src', '}', '', 'zones {', 'core core/**', '}'])
doautocmd BufRead

if &filetype !=# 'zoning'
  cquit 1
endif
if &commentstring !=# '// %s'
  cquit 2
endif
if &indentexpr !=# 'zoning#indent(v:lnum)'
  cquit 3
endif
if zoning#indent(2) != shiftwidth()
  cquit 4
endif
if zoning#icon() !=# '≡'
  cquit 5
endif

" The extension is shared with BIND, so the first declaration decides. These go
" through the disk rather than `doautocmd`, because what is under test is the
" detection a real `:edit` performs and not a buffer we already labelled.
function! s:FiletypeOf(lines) abort
  let l:path = tempname() . '.zone'
  call writefile(a:lines, l:path)
  execute 'edit! ' . fnameescape(l:path)
  let l:filetype = &filetype
  bwipeout!
  call delete(l:path)
  return l:filetype
endfunction

if s:FiletypeOf(['// a comment first', '', 'workspace {', 'member */', '}']) !=# 'zoning'
  cquit 6
endif
if s:FiletypeOf(['$TTL 3600', '@ IN SOA ns1.example.com. root.example.com. (1 1 1 1 1)']) ==# 'zoning'
  cquit 7
endif

quitall!
