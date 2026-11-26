(item_header
  (keyword) @context
  name: (_) @name) @item

(impl_header
  .
  (keyword) @context
  trait: (_) @name
  (keyword)? @context
  type: (_)? @name) @item

(macro_header
  (macro_identifier) @context
  name: (_) @name) @item
