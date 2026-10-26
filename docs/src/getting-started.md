# Getting started

## Install

Install the CLI and the language server from a checkout:

```sh
cargo install --path crates/renk --bin renk --bin renk-lsp
```

[Scripts](scripts.md) also need a nightly toolchain (`rustup toolchain install nightly`). The
[language server](editors.md) uses `rust-analyzer` from `PATH` when it is installed.

## A first program

Save this as `hello.rk`:

```rk
pub fn sum_until(values: &[i32], limit: usize) -> i32:
    let mut index = 0
    let mut total = 0
    while index < limit:
        total += values[index]
        index += 1
    if total < 0 then 0 else total

pub fn main():
    println!("{sum_until(&[1, 2, 3], 2)}")
```

Then run `renk transpile hello.rk -o hello.rs && rustc --edition 2024 hello.rs && ./hello`, which prints
`3`.

The [examples](https://github.com/scarf005/rusk/tree/main/examples) directory has more programs.

## Commands

```sh
renk transpile input.rk -o output.rs           # Renk to Rust
renk transpile input.rk --source-map out.json  # also write a JSON source map
cat input.rk | renk                            # stdin to stdout
renk from-rust input.rs -o output.rk           # Rust to Renk
renk fmt input.rk -o input.rk                 # format
renk run                                        # transpile a crate, then cargo run
renk cargo test                                 # any cargo command
renk script tool.rk arg                        # run a single-file script
renk add tool.rk regex                         # add a script dependency
renk-lsp                                        # language server on stdio
```

Without `-o`, output goes to stdout. Run without an input while stdin is a terminal, `renk`
prints its help.
