(comment) @comment
(string) @string
(escape_sequence) @string.escape
(folded_reason) @string
(number) @number

[
  "package"
  "workspace"
  "member"
  "root"
  "language"
  "facade"
  "exclude"
  "zones"
  "through"
  "open"
  "to"
  "nobody"
  "by"
  "limit"
  "hop"
  "hops"
  "forbid"
  "cycles"
  "across"
  "directories"
  "variance"
  "because"
] @keyword

; `seal`, `keep`, `use`, and `reach` are each a statement keyword and also a law name. A law
; is one anonymous token inside a `(law)` node, so the two share a range exactly - and the
; innermost capture is the one an editor paints, which means a bare keyword list reaches
; inside `(law)` and repaints the law it should have left alone. Ordering cannot settle it,
; because the conflict is depth rather than sequence. Naming the statement each word belongs
; to keeps the list out of a variance, where the grammar already knows the difference.
(seal_declaration "seal" @keyword)
(keep_declaration "keep" @keyword)
(use_declaration "use" @keyword)
(limit_declaration "reach" @keyword)

(law) @type

(package_declaration name: (word) @title)
(zone_definition name: (word) @type)
(seal_declaration subject: (word) @type)
(keep_declaration subject: (word) @type)

"->" @operator
["{" "}"] @punctuation.bracket
