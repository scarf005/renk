# Limits

Renk is a syntax front end. It reads text, not types, so:

- `a.b` is decided by name binding, not by type. A misread name usually fails to compile. Use
  `foo::new()` to force a path, or `(Unit).method()` to force a value receiver.
- `[...]` after an expression is indexing unless it is `name[...](` or a CamelCase
  `Name[...]` before `::` or `.name`. Any other generic application uses Rust `::<...>`.
- Macro and attribute token trees are opaque Rust tokens. Only the std format and expression
  macros listed in [Macros](syntax/macros.md) are rewritten.
- Proc-macro DSLs are not parsed.

## Conversion gaps

`renk from-rust` keeps the compiled meaning but not always the exact text:

- A unit or diverging expression that ends a block can gain a `;`, as after `panic!()` or a
  `match` statement whose arms are expressions such as `None => v.clear()`.
- A `macro_rules!` arm that starts on the item's line, as in `macro_rules! m {() => {`, moves
  to its own lines with a `;` after it.
