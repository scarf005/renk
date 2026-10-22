# Syntax

Rusk differs from Rust in four places:

- [Blocks](blocks.md) open with `:` and end by indentation. Statements end at line ends.
- [Expressions](expressions.md) add `if c then a else b`, and `if`, `match`, and closure blocks
  can stand where a value goes.
- [Paths and generics](paths.md) use `.` and `[T]` where Rust uses `::` and `<T>`.
- [Format strings](macros.md) take inline expressions such as `{user.name()}`.

The rest is Rust. A Rust `{ ... }` block, a `::` path, or a `::<T>` turbofish is always
accepted and kept as written.
