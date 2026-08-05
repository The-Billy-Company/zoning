if exists('b:did_indent')
  finish
endif
let b:did_indent = 1

setlocal autoindent
setlocal indentexpr=zoning#indent(v:lnum)
setlocal indentkeys=0{,0},o,O,!^F

let b:undo_indent = 'setlocal autoindent< indentexpr< indentkeys<'
