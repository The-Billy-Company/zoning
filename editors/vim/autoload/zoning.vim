function! zoning#indent(lnum) abort
  let l:current = getline(a:lnum)
  let l:previous_number = prevnonblank(a:lnum - 1)
  if l:previous_number == 0
    return 0
  endif

  let l:previous = getline(l:previous_number)
  let l:amount = indent(l:previous_number)
  if l:current =~# '^\s*}'
    let l:amount -= shiftwidth()
  endif
  if l:previous =~# '{\s*\%(//.*\)\?$'
    let l:amount += shiftwidth()
  elseif l:previous =~# '\<because\s*$'
    let l:amount += shiftwidth()
  endif
  return max([l:amount, 0])
endfunction

function! zoning#fold(lnum) abort
  let l:line = substitute(getline(a:lnum), '//.*$', '', '')
  if l:line =~# '{\s*$'
    return 'a1'
  endif
  if l:line =~# '^\s*}'
    return 's1'
  endif
  return '='
endfunction

function! zoning#icon(...) abort
  if get(g:, 'zoning_ascii_icon', 0)
    return '[=]'
  endif
  if get(g:, 'zoning_nerd_font', 0)
    return get(g:, 'zoning_nerd_font_icon', '󰙅')
  endif
  return '≡'
endfunction
