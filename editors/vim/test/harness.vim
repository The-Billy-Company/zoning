" A named-assertion harness for the Vim runtime, and the runtime the suites load.
"
" The suites used to signal with `cquit <n>`, which tells you a number and makes you count
" `if` blocks to learn what broke. These report TAP: a failure names itself, and the whole
" suite runs rather than stopping at the first one, so a CI log shows every problem at once
" instead of one per push.

set nomore
set nocompatible
" A test that reindents or deletes a range earns Vim's own "3 lines indented" chatter,
" which Neovim writes to the same stream the report goes to - so the TAP stream would
" carry a line no parser can read. Nothing here is interactive enough to want the count.
set report=9999
" An installed copy of this plugin lives under `packpath` once `zone setup` has run, and it
" would shadow the tree under test - so the suite would grade the last install rather than
" the working copy.
set packpath=
let g:zoning_test_root = fnamemodify(expand('<sfile>'), ':p:h:h')
execute 'set runtimepath^=' . fnameescape(g:zoning_test_root)
filetype plugin indent on
syntax enable

let s:passed = 0
let s:failures = []

" `:echo` is swallowed by silent ex mode and lands on a different stream in Neovim's
" headless mode, so the report is written to stdout directly, where both agree.
function! s:Say(line) abort
  call writefile([a:line], '/dev/stdout', 'a')
endfunction

" Announce a passing check, or bank a failing one with what it expected.
function! Ok(condition, name) abort
  if a:condition
    let s:passed += 1
    call s:Say('ok ' . (s:passed + len(s:failures)) . ' - ' . a:name)
  else
    call add(s:failures, a:name)
    call s:Say('not ok ' . (s:passed + len(s:failures)) . ' - ' . a:name)
  endif
endfunction

" The same, for the common case of comparing two values, so the report can show both.
function! Is(actual, expected, name) abort
  call Ok(a:actual ==# a:expected, a:name)
  if a:actual !=# a:expected
    call s:Say('  # expected: ' . string(a:expected))
    call s:Say('  #   actual: ' . string(a:actual))
  endif
endfunction

" Load the plugin files a package manager would, in the order it would load them.
function! Load() abort
  runtime plugin/zoning.vim
  if has('nvim')
    runtime plugin/zoning.lua
  endif
endfunction

" A `.zone` file on disk holding `lines`, opened for real. Detection is what is usually
" under test, and `:doautocmd` on a buffer we already named would not exercise it.
function! Open(lines) abort
  let l:path = tempname() . '.zone'
  call writefile(a:lines, l:path)
  execute 'edit! ' . fnameescape(l:path)
  return l:path
endfunction

" Print the tally and leave with a status the shell can read.
function! Report(suite) abort
  let l:total = s:passed + len(s:failures)
  call s:Say('1..' . l:total)
  let l:editor = has('nvim') ? 'nvim' : 'vim'
  if empty(s:failures)
    call s:Say('# ' . l:editor . ' ' . a:suite . ': ' . l:total . ' passed')
    quitall!
  endif
  call s:Say('# ' . l:editor . ' ' . a:suite . ': ' . len(s:failures) . ' of ' . l:total
        \ . ' failed')
  for l:name in s:failures
    call s:Say('#   ' . l:name)
  endfor
  cquit 1
endfunction
