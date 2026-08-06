/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
  name: "zoning",

  extras: ($) => [/[ \t\r]/, $.comment],

  word: ($) => $.word,

  rules: {
    source_file: ($) =>
      repeat(
        choice(
          $.package_declaration,
          $.workspace_declaration,
          $.zones_declaration,
          $.seal_declaration,
          $.keep_declaration,
          $.use_declaration,
          $.limit_declaration,
          $.forbid_declaration,
          $.variance_declaration,
          $._newline,
        ),
      ),

    // The block is optional: a member of a workspace can inherit every setting, and
    // then there is nothing left to put between the braces.
    package_declaration: ($) =>
      seq(
        "package",
        field("name", $.word),
        optional(
          seq(
            "{",
            $._newline,
            repeat(choice(seq($.package_setting, $._newline), $._newline)),
            "}",
          ),
        ),
        $._newline,
      ),

    package_setting: ($) =>
      choice(
        seq("root", field("value", $.word)),
        seq("language", field("value", $.word)),
        seq("facade", field("value", $.paths)),
        seq("exclude", field("value", $.paths)),
      ),

    // `use` and `limit` are whole statements and eat their own line end; the settings
    // are one value each and leave that to the block.
    workspace_declaration: ($) =>
      seq(
        "workspace",
        "{",
        $._newline,
        repeat(
          choice(
            seq($.workspace_setting, $._newline),
            $.use_declaration,
            $.limit_declaration,
            $._newline,
          ),
        ),
        "}",
        $._newline,
      ),

    workspace_setting: ($) =>
      choice(
        seq("member", field("value", $.paths)),
        seq("root", field("value", $.word)),
        seq("language", field("value", $.word)),
        seq("facade", field("value", $.paths)),
      ),

    zones_declaration: ($) =>
      seq(
        "zones",
        "{",
        $._newline,
        repeat(choice(seq($.zone_definition, $._newline), $._newline)),
        "}",
        $._newline,
      ),

    zone_definition: ($) =>
      seq(field("name", $.word), field("paths", $.paths)),

    seal_declaration: ($) =>
      seq(
        "seal",
        field("subject", $.word),
        "through",
        field("facade", $.word),
        optional(seq("open", "to", field("guests", $.paths))),
        $._newline,
      ),

    keep_declaration: ($) =>
      seq(
        "keep",
        field("subject", $.word),
        "to",
        choice("nobody", field("guests", $.paths)),
        $._newline,
      ),

    use_declaration: ($) =>
      seq(
        "use",
        field("modules", $.paths),
        optional(seq("by", field("scope", $.paths))),
        $._newline,
      ),

    limit_declaration: ($) =>
      seq(
        "limit",
        "reach",
        "to",
        field("count", $.number),
        choice("hop", "hops"),
        $._newline,
      ),

    forbid_declaration: ($) =>
      seq("forbid", "cycles", "across", "directories", $._newline),

    variance_declaration: ($) =>
      seq(
        "variance",
        field("law", $.law),
        field("subject", choice($.edge, $.cycle)),
        repeat($._newline),
        "because",
        repeat($._newline),
        field(
          "reason",
          choice(seq($.string, $._newline), $.folded_reason),
        ),
      ),

    edge: ($) =>
      seq(
        field("from", $.word),
        "->",
        field("to", $.word),
      ),

    cycle: ($) =>
      seq(
        "{",
        $._newline,
        repeat1(choice(seq(field("member", $.word), $._newline), $._newline)),
        "}",
      ),

    paths: ($) => choice(repeat1($.word), $.path_block),

    path_block: ($) =>
      seq(
        "{",
        $._newline,
        repeat1(choice(seq(repeat1($.word), $._newline), $._newline)),
        "}",
      ),

    law: (_) => choice("zone", "seal", "keep", "cycle", "reach", "use", "escape"),

    folded_reason: ($) =>
      repeat1(seq("\\\\", optional($.reason_text), $._newline)),

    string: ($) =>
      seq(
        '"',
        repeat(choice(token.immediate(/[^"\\\n]+/), $.escape_sequence)),
        '"',
      ),

    escape_sequence: (_) => token.immediate(seq("\\", /./)),
    reason_text: (_) => token.immediate(/[^\n]+/),
    number: (_) => /\d+/,
    word: (_) => /[^\s{}"]+/,
    comment: (_) => token(seq("//", /[^\n]*/)),
    _newline: (_) => "\n",
  },
});
