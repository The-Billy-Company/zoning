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
  "seal"
  "through"
  "open"
  "to"
  "keep"
  "nobody"
  "use"
  "by"
  "limit"
  "reach"
  "hop"
  "hops"
  "forbid"
  "cycles"
  "across"
  "directories"
  "variance"
  "because"
] @keyword

(law) @type
(package_declaration name: (word) @title)
(zone_definition name: (word) @type)
(seal_declaration subject: (word) @type)
(keep_declaration subject: (word) @type)

"->" @operator
["{" "}"] @punctuation.bracket
