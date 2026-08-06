" Who claims a `.zone` file.
"
" The extension is shared with BIND, so the plugin reads the first declaration instead of
" trusting the suffix, and hands anything else back to Vim's own detection rather than
" guessing. Both halves matter: claiming too little breaks the plugin, and claiming too much
" breaks somebody's DNS zone file, which is the worse of the two.

source <sfile>:p:h/harness.vim
call Load()

function! s:FiletypeOf(lines) abort
  let l:path = Open(a:lines)
  let l:filetype = &filetype
  bwipeout!
  call delete(l:path)
  return l:filetype
endfunction

call Is(s:FiletypeOf(['package demo {', 'root src', '}']), 'zoning',
      \ 'a contract leading with `package` is zoning')
call Is(s:FiletypeOf(['workspace {', 'member */', '}']), 'zoning',
      \ 'a contract leading with `workspace` is zoning')
call Is(s:FiletypeOf(['// a comment first', '', 'workspace {', 'member */', '}']), 'zoning',
      \ 'comments and blank lines are skipped before the first declaration')
call Is(s:FiletypeOf(['package demo']), 'zoning',
      \ 'a one-line package declaration with no block is zoning')
call Is(s:FiletypeOf([]), 'zoning',
      \ 'a new, empty buffer is zoning, because that is what you are about to write')

call Ok(s:FiletypeOf(['$TTL 3600',
      \ '@ IN SOA ns1.example.com. root.example.com. (1 1 1 1 1)']) !=# 'zoning',
      \ 'a BIND zone file is left for Vim to recognize')
call Ok(s:FiletypeOf(['; a DNS comment', 'example.com. IN A 10.0.0.1']) !=# 'zoning',
      \ 'a leading `;` comment is not a zoning comment, so the file is not claimed')
call Ok(s:FiletypeOf(['packages are not a declaration']) !=# 'zoning',
      \ 'a word that merely starts with `package` does not claim the file')

" The other half of recognizing a contract is the mark next to it in a file tree. Three
" glyphs, because a terminal without a Nerd Font renders the patched one as a box, and some
" terminals cannot draw even the plain one.
call Is(zoning#icon(), '≡', 'the default icon needs no patched font')
let g:zoning_nerd_font = 1
call Is(zoning#icon(), '󰙅', 'a Nerd Font gets the patched glyph')
let g:zoning_nerd_font_icon = ''
call Is(zoning#icon(), '', 'which the user can choose for themselves')
let g:zoning_ascii_icon = 1
call Is(zoning#icon(), '[=]', 'and ASCII wins over both, for a terminal that draws neither')
unlet g:zoning_ascii_icon g:zoning_nerd_font g:zoning_nerd_font_icon

call Report('detect')
