const ITEM_KEYWORDS = [
  "const",
  "enum",
  "fn",
  "mod",
  "static",
  "struct",
  "trait",
  "type",
]

const KEYWORDS = [
  "as",
  "async",
  "await",
  "break",
  "continue",
  "crate",
  "else",
  "extern",
  "false",
  "for",
  "if",
  "impl",
  "in",
  "let",
  "loop",
  "match",
  "move",
  "mut",
  "pub",
  "ref",
  "return",
  "Self",
  "self",
  "then",
  "true",
  "unsafe",
  "use",
  "where",
  "while",
  ...ITEM_KEYWORDS,
]

const BUILTIN_TYPES = [
  "bool",
  "char",
  "f32",
  "f64",
  "i8",
  "i16",
  "i32",
  "i64",
  "i128",
  "isize",
  "str",
  "u8",
  "u16",
  "u32",
  "u64",
  "u128",
  "usize",
]

const NUMBER_SUFFIX = /([iu](8|16|32|64|128|size)|f32|f64)/

module.exports = grammar({
  name: "renk",

  externals: ($) => [$.block_comment, $.raw_string],

  extras: ($) => [/\s/, $.line_comment, $.block_comment],

  word: ($) => $.identifier,

  conflicts: ($) => [
    [$.keyword, $.item_header],
    [$.keyword, $.impl_header],
    [$._identifier, $.item_header],
    [$.operator, $.return_impl],
  ],

  rules: {
    source_file: ($) => repeat($._item_token),

    // Headers exist at the top level and in braces, so `&impl Trait` in a parameter list is not an item.
    // An unclosed `{` ends where its enclosing group ends, because Renk closes some blocks by indentation.
    _item_token: ($) => choice($.item_header, $.impl_header, $.macro_header, $._token),

    _token: ($) => choice(
      $.paren_group,
      $.bracket_group,
      $.brace_group,
      $.attribute,
      $.raw_string,
      $.string,
      $.char,
      $.number,
      $.lifetime,
      $.macro_variable,
      $.return_impl,
      $.keyword,
      $.builtin_type,
      $.macro_identifier,
      $.constant,
      $.type_identifier,
      $._identifier,
      $.operator,
      $.punctuation,
    ),

    paren_group: ($) => seq("(", repeat($._token), ")"),
    bracket_group: ($) => seq("[", repeat($._token), "]"),
    brace_group: ($) => prec.right(seq("{", repeat($._item_token), optional("}"))),

    line_comment: (_) => token(seq("//", /.*/)),

    attribute: (_) => token(/#!?\[[^\]\n]*\]/),

    string: (_) => token(seq(
      optional(/[bc]/),
      '"',
      repeat(choice(/[^"\\]+/, /\\(.|\n)/)),
      '"',
    )),

    char: (_) => token(seq(
      optional("b"),
      "'",
      choice(/[^'\\\n]/, /\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|[^\n])/),
      "'",
    )),

    lifetime: (_) => token(/'[\p{L}_][\p{L}\p{N}_]*/),

    macro_variable: (_) => token(/\$[\p{L}_][\p{L}\p{N}_]*/),

    number: (_) => token(choice(
      seq(/0[xX]/, /[0-9a-fA-F_]+/, optional(NUMBER_SUFFIX)),
      seq(/0[bB]/, /[01_]+/, optional(NUMBER_SUFFIX)),
      seq(/0[oO]/, /[0-7_]+/, optional(NUMBER_SUFFIX)),
      seq(
        /\d[\d_]*/,
        optional(seq(".", /\d[\d_]*/)),
        optional(seq(/[eE]/, optional(/[+-]/), /[\d_]+/)),
        optional(NUMBER_SUFFIX),
      ),
    )),

    keyword: (_) => choice(...KEYWORDS),

    // `-> impl Trait` names a type, not an `impl` block.
    return_impl: ($) => prec.dynamic(2, seq(alias("->", $.operator), alias("impl", $.keyword))),

    item_header: ($) => prec.dynamic(1, seq(
      alias(choice(...ITEM_KEYWORDS, "union"), $.keyword),
      field("name", $._name),
    )),

    impl_header: ($) => prec.right(prec.dynamic(1, seq(
      alias("impl", $.keyword),
      optional($.bracket_group),
      field("trait", $.type_identifier),
      optional(seq(alias("for", $.keyword), field("type", $.type_identifier))),
    ))),

    macro_header: ($) => prec.dynamic(1, seq(
      alias("macro_rules!", $.macro_identifier),
      field("name", $._name),
    )),

    _name: ($) => choice($.identifier, $.constant, $.type_identifier),

    builtin_type: (_) => token(choice(...BUILTIN_TYPES)),

    macro_identifier: (_) => token(/[\p{L}_][\p{L}\p{N}_]*!/),

    constant: (_) => token(prec(1, /[A-Z][A-Z0-9_]+/)),

    type_identifier: (_) => token(/[A-Z][A-Za-z0-9_]*/),

    _identifier: ($) => choice($.identifier, alias("union", $.identifier)),

    identifier: (_) => token(/[\p{Ll}\p{Lo}_][\p{L}\p{N}_]*/),

    operator: (_) => choice(
      "=>",
      "->",
      "::",
      /[+\-*%=<>!&|.:?^@~#\\]+/,
      "$",
      "/",
      "/=",
    ),

    punctuation: (_) => token(/[,;]/),
  },
})
