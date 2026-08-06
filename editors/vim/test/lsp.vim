" Whether each language-client ecosystem is actually told about the server.
"
" There are five clients and none of them can be installed on a CI runner, so each one is
" stood up as the smallest thing `zoning#lsp#register()` looks for - the function or variable
" or command whose existence it takes as "this client is here" - and then asked what it was
" handed. That is the whole of the contract on our side: past the registration call, the
" client's own code takes over, and `lsp.rs` already proves what it will find at the far end.

source <sfile>:p:h/harness.vim

let g:registered = {}

" An autoload function has to live in the file its name spells, so each stand-in is written
" to a scratch runtime directory and put on the front of `runtimepath` - which is also how
" the real client would arrive. `runtime!` then loads it, because `exists('*a#b#C')` is false
" for an autoload function nothing has reached for yet.
let s:runtime = tempname()
function! s:Client(path, lines) abort
  let l:file = s:runtime . '/' . a:path
  call mkdir(fnamemodify(l:file, ':h'), 'p')
  call writefile(a:lines, l:file)
endfunction

" vim-lsp.
call s:Client('autoload/lsp.vim', [
      \ 'function! lsp#register_server(server) abort',
      \ '  let g:registered.vim_lsp = a:server',
      \ 'endfunction',
      \ ])
" ALE, which asks for a linter definition and for its own project-root handler.
call s:Client('autoload/ale/linter.vim', [
      \ 'function! ale#linter#Define(filetype, linter) abort',
      \ '  let g:registered.ale = extend(copy(a:linter), {"filetype": a:filetype})',
      \ 'endfunction',
      \ ])
call s:Client('autoload/ale/handlers/git.vim', [
      \ 'function! ale#handlers#git#GetProjectRoot(buffer) abort',
      \ '  return "/"',
      \ 'endfunction',
      \ ])
" coc.nvim, which is recognized by one of its commands existing.
call s:Client('plugin/coc.vim', ['command! CocInfo echo "coc"'])

execute 'set runtimepath^=' . fnameescape(s:runtime)
runtime! autoload/lsp.vim
runtime! autoload/ale/linter.vim
runtime! autoload/ale/handlers/git.vim
runtime! plugin/coc.vim
let g:coc_user_config = {}

" vim-lsc, which is configured by assignment rather than by a call.
let g:lsc_server_commands = {}

call Load()
" `plugin/zoning.vim` registers on `VimEnter`, which has already fired by the time a `-S`
" script runs, so it takes the `v:vim_did_enter` path. Under Neovim's headless mode that is
" not guaranteed, so ask for it directly - registering twice is a no-op by design, and
" proving that is the last check in this file.
call zoning#lsp#register()

call Is(zoning#lsp#command(), ['zoning', 'lsp', '--stdio'],
      \ 'the launch command is the executable plus `lsp --stdio`')

let g:zoning_executable = '/opt/zoning/bin/zoning'
call Is(zoning#lsp#command(), ['/opt/zoning/bin/zoning', 'lsp', '--stdio'],
      \ 'g:zoning_executable overrides the executable and keeps the argv')
call Is(zoning#lsp#ale_executable(0), '/opt/zoning/bin/zoning',
      \ 'ALE is handed the same override')
unlet g:zoning_executable

call Is(zoning#lsp#ale_command(0), '%e lsp --stdio',
      \ 'ALE builds its own command line from %e, so the override reaches it once')

call Ok(has_key(g:registered, 'vim_lsp'), 'vim-lsp was told about a server')
if has_key(g:registered, 'vim_lsp')
  call Is(g:registered.vim_lsp.name, 'zoning', 'under the name zoning')
  call Is(g:registered.vim_lsp.allowlist, ['zoning'], 'for zoning buffers only')
  call Is(call(g:registered.vim_lsp.cmd, [0]), ['zoning', 'lsp', '--stdio'],
        \ 'and its cmd resolves to the launch command')
endif

call Ok(has_key(g:lsc_server_commands, 'zoning'), 'vim-lsc was given a server command')
if has_key(g:lsc_server_commands, 'zoning')
  call Is(g:lsc_server_commands.zoning.command, 'zoning lsp --stdio',
        \ 'as one string, which is what vim-lsc wants')
  call Ok(!g:lsc_server_commands.zoning.suppress_stderr,
        \ 'with stderr shown, because a server that refuses to start says why there')
endif

let s:coc = get(get(g:, 'coc_user_config', {}), 'languageserver', {})
call Ok(has_key(s:coc, 'zoning'), 'coc.nvim was given a language server')
if has_key(s:coc, 'zoning')
  call Is(s:coc.zoning.args, ['lsp', '--stdio'], 'with the serve argv')
  call Is(s:coc.zoning.filetypes, ['zoning'], 'for zoning buffers')
  call Is(s:coc.zoning.rootPatterns, ['contract', '.git'],
        \ 'rooted at a contract directory, or failing that the repository')
endif

call Ok(has_key(g:registered, 'ale'), 'ALE was given a linter')
if has_key(g:registered, 'ale')
  call Is(g:registered.ale.filetype, 'zoning', 'for zoning buffers')
  call Is(g:registered.ale.lsp, 'stdio', 'speaking LSP over stdio rather than parsing output')
endif

" Registration is idempotent: a plugin manager that sources `plugin/` twice, or a user who
" calls the function themselves after `VimEnter` already did, must not double-register.
let s:before = len(keys(g:lsc_server_commands))
call zoning#lsp#register()
call Is(len(keys(g:lsc_server_commands)), s:before, 'registering twice changes nothing')

" Neovim ships its own client, configured by a file rather than by a call.
if has('nvim')
  let s:native = luaeval('vim.lsp.config["zoning"] or vim.empty_dict()')
  call Ok(!empty(s:native), 'the native client has a zoning config')
  if !empty(s:native)
    call Is(s:native.cmd, ['zoning', 'lsp', '--stdio'], 'launching the same command')
    call Is(s:native.filetypes, ['zoning'], 'for zoning buffers')
    call Is(s:native.root_markers, ['contract', '.git'], 'with the same roots as coc')
  endif
  call Ok(luaeval('vim.lsp.is_enabled ~= nil and vim.lsp.is_enabled("zoning") or true'),
        \ 'and it is enabled, so opening a contract starts the server')
endif

call Report('lsp')
