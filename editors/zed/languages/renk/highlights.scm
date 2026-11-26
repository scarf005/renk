(line_comment) @comment
(block_comment) @comment
(attribute) @attribute
(raw_string) @string
(string) @string
(char) @string
(number) @number
(lifetime) @label
(macro_variable) @variable.special
(keyword) @keyword
(builtin_type) @type.builtin
(type_identifier) @type
(constant) @constant
(macro_identifier) @function
(identifier) @variable
(item_header
  (keyword) @_item
  name: (identifier) @function
  (#eq? @_item "fn"))
(operator) @operator
(punctuation) @punctuation.delimiter
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
