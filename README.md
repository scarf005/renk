# Renk

![](./thumbnail.webp)

Renk is an indentation-based syntax for [Rust](https://rust-lang.org), inspired by [Rust's Ugly Syntax](https://matklad.github.io/2023/01/26/rusts-ugly-syntax.html).

Documentation: <https://scarf005.github.io/rusk/> ([한국어](https://scarf005.github.io/rusk/ko/))

## Example

```rk
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

## Usage

```sh
cargo install --path crates/renk --bin renk --bin renk-lsp
renk transpile input.rk -o output.rs
renk from-rust input.rs -o output.rk
cat input.rk | renk
```

See [Getting started](https://scarf005.github.io/rusk/getting-started.html) and [Syntax](https://scarf005.github.io/rusk/syntax/index.html).

## Formatter

`renk fmt` formats a file with rustfmt's layout; `--check` prints a diff and exits with 1 for unformatted files. [Details](https://scarf005.github.io/rusk/tools.html).

```sh
renk fmt src/main.rk -o src/main.rk
renk fmt --check src/*.rk
```

## Cargo wrapper

`renk build`, `renk run`, `renk test`, and `renk cargo ...` transpile `.rk` files under Cargo source roots into `.rs` files, then run Cargo. [Details](https://scarf005.github.io/rusk/cargo.html).

```sh
renk cargo test
```

## Scripts

`renk script` runs a single-file script with `cargo +nightly -Zscript`. Dependencies go in `---` frontmatter; `renk add` edits it. [Details](https://scarf005.github.io/rusk/scripts.html).

```rk
#!/usr/bin/env -S renk script
---
[dependencies]
regex = "1"
---

fn main():
    println!("{}", regex::Regex::new("a+").unwrap().is_match("caab"))
```

```sh
renk script tool.rk arg
renk add tool.rk regex
```

## Language server

`renk-lsp` is a stdio language server. It reports syntax errors and symbols, and forwards hover, definition, and completion to `rust-analyzer`. [Editor setup](https://scarf005.github.io/rusk/editors.html).

## Web runner

`podman compose up --build` serves the web editor at <http://localhost:8080>. `just web` starts it from a checkout. [Details](https://scarf005.github.io/rusk/tools.html#web-runner).

## Limits

Renk reads text, not types, so some names are misread. See [Limits](https://scarf005.github.io/rusk/limits.html).
