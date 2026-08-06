" `.zone` is shared with BIND, so the extension cannot decide the filetype on
" its own. A contract leads with `package` or `workspace` — the same first
" declaration the tool itself identifies one by — and anything else is left
" unclaimed, which hands it to Vim's own content detection rather than to a
" guess of ours: a DNS zone file comes out `bindzone` without this plugin
" naming that filetype anywhere.
function! s:Detect() abort
  if line('$') ==# 1 && getline(1) ==# ''
    setlocal filetype=zoning
    return
  endif
  for l:lnum in range(1, min([line('$'), 64]))
    let l:line = getline(l:lnum)
    if l:line =~# '^\s*\%(//.*\)\?$'
      continue
    endif
    if l:line =~# '^\s*\%(package\|workspace\)\>'
      setlocal filetype=zoning
    endif
    return
  endfor
endfunction

augroup zoning_filetype
  autocmd!
  autocmd BufNewFile,BufRead *.zone call s:Detect()
augroup END
