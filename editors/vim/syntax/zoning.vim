if exists('b:current_syntax')
  finish
endif

" Code first, prose last. Vim gives a later `:syntax match` priority over an earlier one at
" the same position, and a glob is any run of non-space containing a slash - which `//` is.
" So a comment declared before the glob rule loses its own opening to it, and the whole line
" reads as a path: keywords, globs, and numbers written inside a sentence come out painted
" as code. Declaring the spans after the atoms lets each span own its opening, and because
" none of them list the atoms in `contains`, nothing inside a comment or a string is code.
syntax match zoningNumber '\<\d\+\>'
syntax match zoningArrow '->'
syntax match zoningBrace '[{}]'
syntax match zoningPath '\S*\%(\*\*\|\*\|/\)\S*'
syntax keyword zoningKeyword package workspace member root language facade exclude zones
syntax keyword zoningKeyword seal through open to keep nobody use by
syntax keyword zoningKeyword limit reach hop hops forbid cycles across
syntax keyword zoningKeyword directories because

" `seal`, `keep`, `use`, and `reach` name both a statement and a law, and a flat pair of
" keyword lists cannot tell them apart - whichever list is declared second simply wins
" everywhere, which is why `seal engine through …` used to paint its own statement as a
" type. A law only ever appears in one place, immediately after `variance`, so that is
" where it is looked for: `nextgroup` tries the law first at the word after the keyword,
" and `contained` keeps it from matching anywhere else.
syntax keyword zoningKeyword variance nextgroup=zoningLaw skipwhite
syntax match zoningLaw '\<\%(zone\|seal\|keep\|cycle\|reach\|use\|escape\)\>' contained

syntax match zoningComment '//.*$' contains=@Spell
syntax region zoningString start=+"+ skip=+\\.+ end=+"+ oneline
syntax match zoningFoldedReason '^\s*\\\\.*$' contains=zoningReasonMarker
syntax match zoningReasonMarker '\\\\' contained

highlight default link zoningComment Comment
highlight default link zoningString String
highlight default link zoningFoldedReason String
highlight default link zoningReasonMarker SpecialChar
highlight default link zoningNumber Number
highlight default link zoningArrow Operator
highlight default link zoningBrace Delimiter
highlight default link zoningPath Directory
highlight default link zoningKeyword Keyword
highlight default link zoningLaw Type

let b:current_syntax = 'zoning'
