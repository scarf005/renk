# Introduction

Rusk is an indentation-based syntax for [Rust](https://rust-lang.org), inspired by
[Rust's Ugly Syntax](https://matklad.github.io/2023/01/26/rusts-ugly-syntax.html). `rusk`
transpiles `.rsk` files to Rust and converts Rust back to Rusk.

```rsk
#[derive(Debug, Clone)]
pub struct User:
    pub id: u64
    pub name: String

impl User:
    pub fn new(id: u64, name: String) -> Self = Self{ id, name }
    pub fn display_name(&self) -> &str = &self.name

pub fn main():
    let user = User.new(1, "Ada".to_string())
    println!("{user.display_name()}")
```

Generated Rust:

```rust
#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
}
impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self{ id, name }
    }
    pub fn display_name(&self) -> &str {
        &self.name
    }
}
pub fn main() {
    let user = User::new(1, "Ada".to_string());
    println!("{}", user.display_name());
}
```

Rusk changes syntax only: types, borrowing, macros, and crates are Rust's. Whatever the
transpiler does not rewrite passes through as Rust tokens. A Rust `{ ... }` block, a `::` path,
or a `::<T>` turbofish is always accepted.
