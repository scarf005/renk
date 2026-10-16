# Rusk

![](./thumbnail.webp)

`rusk` is an alternative syntax for [rust](https://rust-lang.org) inspired by the [2023 post rust's ugly syntax ](https://matklad.github.io/2023/01/26/rusts-ugly-syntax.html). `ruk` (`.rk`) is Rust syntax with optional semicolons inferred from function signatures.

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
rusk input.rsk -o output.rs
rusk input.rk -o output.rs
rusk transpile input.rsk --source-map output.map.json
rusk from-rust input.rs -o output.rsk
rusk from-rust input.rs -o output.rsk --aggressive
rusk to-ruk input.rs -o output.rk
rusk fmt input.rsk -o input.rsk --line-width 100
cat input.rsk | rusk
rusk run
rusk cargo test
rusk script tool.rsk arg
rusk-lsp
just web # open web editor
```

## Supported MVP syntax

- `:`-marked indentation blocks for `struct`, `enum`, `trait`, `impl`, `mod`, `macro_rules!`, `fn`, `if`, `else`, `match`, loops, `unsafe`, and `async`
- optional opening braces on `:`-marked indented blocks, plus Rust `{` blocks as an escape hatch
- single-expression function shorthand with `=`, e.g. `fn id(value: i32) -> i32 = value`
- `if condition then expr else expr` expressions
- Rust-style inline struct literals, e.g. `Self{ id, name }`
- trailing `;` for explicit semicolon/discard statements when inference is not enough
- Rust-style `#[...]` / `#![...]` attributes; attribute payloads use Rust token syntax unchanged
- generic parameter declarations with `[T]`, e.g. `struct Boxed[T]` -> `struct Boxed<T>`
- generic calls with `name[...](`, e.g. `iter.collect[Vec[i32]]()` -> `iter.collect::<Vec<i32>>()`, `size_of[T]()` -> `size_of::<T>()`, and `serde_json::from_str[T](s)`; to index and call the result, parenthesize it as in Rust's `(x.f)()`: `(handlers[i])()`
- generic paths with an uppercase head before `::` or `.name`, e.g. `Vec[String].new()` and `Vec[String]::new()` -> `Vec::<String>::new()`; to index an uppercase value, parenthesize it: `(TABLE)[i].len()`
- any other expression `[...]` is indexing, e.g. `values[index]` and `values[index].len()`; it is never inferred from its contents
- other generic application uses Rust syntax, e.g. `.map(str::parse::<i32>)`
- Unicode identifiers follow Rust's XID rules, e.g. `목록.변환[Vec[i32]]()`
- type positions inside statements keep `[T]`: `let`/`static`/`const` annotations and closure parameter/return types, e.g. `let found: Option[i32] = None`
- multi-line Rust groups: a line ending with `(` or `[`, a struct literal or `use` tree ending with `{`, an unterminated string, or a trailing `=` continues on the next lines until the group closes; group contents stay Rust syntax, and same-indent `.method()` / `?` lines after the group extend it
- a `})?` / `}).method()` closing line keeps its suffix after a `{` block, a trailing `match` head, or the last `else` of a chain whose head ends with `{`
- `match` at the end of a larger expression, e.g. `let x = match y:` or `.filter(|v| match v:`, and `else` branches after `let ... = if c {` or `let pattern = value` (`let ... else`)
- dotted path lowering for obvious item paths, e.g. `std.io.Read` -> `std::io::Read`, `Foo.new()` -> `Foo::new()`
- escape hatches: Rust `::` / `::<...>` force path/generic syntax, while parentheses force a value receiver, e.g. `(TEXT).len()`
- macro definitions with indentation-based `macro_rules!` arms; macro patterns, bodies, and arbitrary macro invocation token trees use Rust syntax unchanged, and a multi-line `pattern =>:` arm keeps every indented body line, including closing delimiters (`println!` / `format!` keep their existing inline-format-expression sugar)
- source formatter that preserves existing line break style; only line width is configurable
- hierarchical JSON source map generation
- Rust-to-Rusk, Rust-to-Ruk, Ruk-to-Rust, Ruk-to-Rusk, and Rusk-to-Ruk conversion
- `from-rust` writes idiomatic Rusk (dotted paths, `[T]` generics, `fn f() -> T = expr`, inferred `;`, `if c then a else b`) only where the result transpiles back to the same Rust; `--aggressive` also accepts rewrites that keep the Rust tokens but not their line layout, such as multi-line `if`/`else` value chains

## Formatter

`rusk fmt` trims trailing whitespace and validates indentation without reflowing expressions, so both compact chains and already-broken chains keep their line break style. The only formatting option is `--line-width`.

```sh
rusk fmt src/main.rsk -o src/main.rsk
rusk fmt src/main.rsk --line-width 120
```

## Cargo wrapper

`rusk` can wrap common Cargo commands. It transpiles `.rsk` and `.rk` files under Cargo source roots (`src`, `examples`, `tests`, `benches`, plus `build.rsk`/`build.rk`) into generated `.rs` files, then runs Cargo.

```sh
rusk run        # cargo run
rusk check      # cargo check
rusk cargo test # cargo test
```

Existing non-generated `.rs` files are never overwritten.

## Scripts

`rusk script FILE.rsk [ARGS...]` runs a single-file script with `cargo +nightly -Zscript`. It keeps the cargo-script `---` frontmatter, transpiles the rest into `$XDG_CACHE_HOME/rusk/scripts/` (rewritten only when it changes), sets `RUSK_SCRIPT` to the script's absolute path, and exits with the script's exit code.

```rsk
#!/usr/bin/env -S rusk script
---
[dependencies]
regex = "1"
---

fn main():
    println!("{}", regex::Regex::new("a+").unwrap().is_match("caab"))
```

## Self-hosted web runner

The Podman Compose setup builds the web UI at image build time, bundles the `rusk` CLI and a small `rusk-web` HTTP server, and uses the runtime image's `rustc` for `/api/run`. Deno is only used in the build stage, not at runtime. The runner bundles `trpl` 0.3.0, so examples can use helpers such as `trpl::block_on` without a Cargo manifest.

```sh
podman compose up --build
# open http://localhost:8080
```

The service binds to `127.0.0.1:8080` on the host for local use or a local Cloudflare Tunnel origin. The container runtime is read-only except for `/tmp`, where Rust snippets are compiled and run with `RUSK_RUN_TIMEOUT_MS`. The `trpl` support does not relax the existing child-process sandbox; network availability remains governed by that sandbox. Run requests are limited to 64 KiB of Rust source, 256 KiB of captured output, two concurrent runs, and ten runs per minute by default. If exposed through Cloudflare Tunnel, put Cloudflare Access and a `/api/run` rate limit in front of it.

## Language Server

`rusk-lsp` is a standard stdio Language Server Protocol server. It reports diagnostics for `.rsk` and `.rk` files, document symbols for `.rsk`, and proxies Rust features through generated Rust when `rust-analyzer` is available.

```sh
cargo install --path crates/rusk --bin rusk-lsp
```

## Current limits

This is a compiler-front-end MVP, not a full Rust replacement yet. It intentionally avoids proc-macro DSL parsing and full type-aware dot disambiguation.

Generic application and indexing are syntactically distinct: `name[...](` and an uppercase `Name[...]` before `::` or `.name` are generic applications, while every other expression `[...]`, including `(values[index])()` and `(TABLE)[i].len()`, is indexing; any other generic application uses Rust `::<...>`. Only dotted path lowering remains heuristic in expression positions. Obvious Rust-style item paths such as `Vec.from(pair)` are lowered to `Vec::from(pair)`, while value receivers keep `.`, and explicit `::` is never reinterpreted. When the heuristic would choose the wrong side, use `foo::new()` to force a path or `(FOO).method()` to force a value receiver. Arbitrary macro and attribute token trees are opaque Rust syntax and are not path/generic-rewritten; the existing `println!` / `format!` inline-format-expression transform is a dedicated exception. Rust-to-Rusk conversion preserves explicit `::` paths, emits `name[...](` and `Name[...]::` for turbofish calls and uppercase paths, keeps other turbofish as Rust, and parenthesizes index-then-call receivers and uppercase value receivers so conversion does not depend on path heuristics.
