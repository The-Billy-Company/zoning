function! zoning#lsp#command(...) abort
  return [get(g:, 'zoning_executable', 'zoning'), 'lsp', '--stdio']
endfunction

function! zoning#lsp#ale_executable(buffer) abort
  return get(g:, 'zoning_executable', 'zoning')
endfunction

function! zoning#lsp#ale_command(buffer) abort
  return '%e lsp --stdio'
endfunction

function! zoning#lsp#register() abort
  if get(g:, 'zoning_lsp_registered', 0)
    return
  endif
  let g:zoning_lsp_registered = 1

  if exists('*lsp#register_server')
    call lsp#register_server({
          \ 'name': 'zoning',
          \ 'cmd': function('zoning#lsp#command'),
          \ 'allowlist': ['zoning'],
          \ })
  endif

  if exists('g:lsc_server_commands')
    let g:lsc_server_commands.zoning = {
          \ 'command': join(zoning#lsp#command(), ' '),
          \ 'suppress_stderr': v:false,
          \ }
  endif

  if exists(':CocInfo')
    let l:config = get(g:, 'coc_user_config', {})
    let l:servers = get(l:config, 'languageserver', {})
    if !has_key(l:servers, 'zoning')
      let l:servers.zoning = {
            \ 'command': get(g:, 'zoning_executable', 'zoning'),
            \ 'args': ['lsp', '--stdio'],
            \ 'filetypes': ['zoning'],
            \ 'rootPatterns': ['contract', '.git'],
            \ }
      let l:config.languageserver = l:servers
      let g:coc_user_config = l:config
    endif
  endif

  if exists('*ale#linter#Define')
    call ale#linter#Define('zoning', {
          \ 'name': 'zoning',
          \ 'lsp': 'stdio',
          \ 'executable': function('zoning#lsp#ale_executable'),
          \ 'command': function('zoning#lsp#ale_command'),
          \ 'project_root': function('ale#handlers#git#GetProjectRoot'),
          \ })
  endif
endfunction
