if exists('b:current_syntax')
  finish
endif

syntax match zoningComment '//.*$' contains=@Spell
syntax region zoningString start=+"+ skip=+\\.+ end=+"+ oneline
syntax match zoningFoldedReason '^\s*\\\\.*$' contains=zoningReasonMarker
syntax match zoningReasonMarker '\\\\' contained
syntax match zoningNumber '\<\d\+\>'
syntax match zoningArrow '->'
syntax match zoningBrace '[{}]'
syntax match zoningPath '\S*\%(\*\*\|\*\|/\)\S*'
syntax keyword zoningKeyword package root language facade exclude zones
syntax keyword zoningKeyword seal through open to keep nobody use by
syntax keyword zoningKeyword limit reach hop hops forbid cycles across
syntax keyword zoningKeyword directories variance because
syntax keyword zoningLaw zone seal keep cycle reach use escape

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
