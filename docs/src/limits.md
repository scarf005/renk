# Limits

Rusk is a syntax front end, not a Rust replacement. It reads text, not types, so:

- `a.b` is decided by name binding, not by type. A misread name usually fails to compile; use
  `foo::new()` to force a path, or `(Unit).method()` to force a value receiver.
- `[...]` after an expression is indexing unless it is `name[...](` or a CamelCase
  `Name[...]` before `::` or `.name`. Any other generic application uses Rust `::<...>`.
- Macro and attribute token trees are opaque Rust tokens. Only the std format and expression
  macros listed in [Macros](syntax/macros.md) are rewritten.
- Proc-macro DSLs are not parsed.
