# Rusk

![](./thumbnail.webp)

Rusk is an alternative syntax for [Rust](https://rust-lang.org) inspired by [Rust's Ugly Syntax](https://matklad.github.io/2023/01/26/rusts-ugly-syntax.html).

Documentation: <https://scarf005.github.io/rusk/> ([한국어](https://scarf005.github.io/rusk/ko/))

## Example

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

## Usage

```sh
cargo install --path crates/rusk --bin rusk --bin rusk-lsp
rusk input.rsk -o output.rs
rusk from-rust input.rs -o output.rsk
cat input.rsk | rusk
```

[Getting started](https://scarf005.github.io/rusk/getting-started.html) lists the commands. [Syntax](https://scarf005.github.io/rusk/syntax/index.html) describes the language.

## Formatter

`rusk fmt` checks that the file parses and trims trailing whitespace. It does not reflow expressions.

```sh
rusk fmt src/main.rsk -o src/main.rsk
```

## Cargo wrapper

`rusk build`, `rusk run`, `rusk test`, and `rusk cargo ...` transpile `.rsk` files under Cargo source roots into `.rs` files, then run Cargo. [Details](https://scarf005.github.io/rusk/cargo.html).

```sh
rusk cargo test
```

## Scripts

`rusk script` builds and runs a single-file script with `cargo +nightly -Zscript`. Dependencies go in `---` frontmatter, and `rusk add` edits it. [Details](https://scarf005.github.io/rusk/scripts.html).

```rsk
#!/usr/bin/env -S rusk script
---
[dependencies]
regex = "1"
---

fn main():
    println!("{}", regex::Regex::new("a+").unwrap().is_match("caab"))
```

```sh
rusk script tool.rsk arg
rusk add tool.rsk regex
```

## Language server

`rusk-lsp` is a stdio language server. It reports syntax errors and symbols for `.rsk` files and forwards hover, definition, and completion to `rust-analyzer` on the generated Rust. [Editor setup](https://scarf005.github.io/rusk/editors.html).

## Web runner

`podman compose up --build` serves the web editor at <http://localhost:8080>. `just web` starts it from a checkout. [Details](https://scarf005.github.io/rusk/tools.html#web-runner).

## Limits

Rusk reads text, not types. See [Limits](https://scarf005.github.io/rusk/limits.html).
