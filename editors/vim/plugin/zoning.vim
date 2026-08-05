if exists('g:loaded_zoning')
  finish
endif
let g:loaded_zoning = 1

augroup zoning_lsp_clients
  autocmd!
  autocmd VimEnter * call zoning#lsp#register()
augroup END

if v:vim_did_enter
  call zoning#lsp#register()
endif
