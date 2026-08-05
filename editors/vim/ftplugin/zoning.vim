if exists('b:did_ftplugin')
  finish
endif
let b:did_ftplugin = 1

setlocal comments=b://
setlocal commentstring=//\ %s
setlocal foldmethod=expr
setlocal foldexpr=zoning#fold(v:lnum)
setlocal foldtext=foldtext()
setlocal formatoptions-=t
setlocal formatoptions+=croql
setlocal iskeyword+=-,/,.,*
setlocal suffixesadd=.zone

let b:undo_ftplugin = 'setlocal comments< commentstring< foldmethod< foldexpr< '
      \ . 'foldtext< formatoptions< iskeyword< suffixesadd<'
