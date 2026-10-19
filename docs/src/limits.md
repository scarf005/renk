# Limits

Rusk is a syntax front end, not a Rust replacement. It reads text, not types, so:

- `a.b` is decided by name binding, not by type. A misread name usually fails to compile; use
  `foo::new()` to force a path, or `(Unit).method()` to force a value receiver.
- `[...]` after an expression is indexing unless it is `name[...](` or a CamelCase
  `Name[...]` before `::` or `.name`. Any other generic application uses Rust `::<...>`.
- Macro and attribute token trees are opaque Rust tokens. Only the std format and expression
  macros listed in [Macros](syntax/macros.md) are rewritten.
- Proc-macro DSLs are not parsed.

## Conversion gaps

- An attribute on the same line as a function with a body, as in `#[inline] pub fn f() {`,
  converts to Rusk that `rusk` rejects with "attributes and comments cannot own indented blocks".
  Move the attribute to its own line first.
- Ruk keeps the `;` of items: `pub struct U;` and `mod a;` need it.
