# Syntax

Rusk changes three things about Rust's surface syntax:

- [Blocks](blocks.md) open with `:` and end by indentation, and statements end at line ends.
- [Paths and generics](paths.md) use `.` and `[T]` where Rust uses `::` and `<T>`.
- [Format strings](macros.md) take inline expressions such as `{user.name()}`.

The rest is Rust. A Rust `{ ... }` block, a `::` path, or a `::<T>` turbofish is always
accepted. The `::` and `::<T>` are kept as written.
