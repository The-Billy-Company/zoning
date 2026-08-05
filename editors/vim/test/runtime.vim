set nomore
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

quitall!
