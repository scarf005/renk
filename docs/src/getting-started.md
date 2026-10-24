# Getting started

## Install

Install the CLI and the language server from a checkout:

```sh
cargo install --path crates/rusk --bin rusk --bin rusk-lsp
```

[Scripts](scripts.md) also need a nightly toolchain (`rustup toolchain install nightly`). The
[language server](editors.md) uses `rust-analyzer` from `PATH` when it is installed.

## A first program

Save this as `hello.rsk`:

```rsk
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

Then run `rusk transpile hello.rsk -o hello.rs && rustc --edition 2024 hello.rs && ./hello`, which prints
`3`.

The [examples](https://github.com/scarf005/rusk/tree/main/examples) directory has more programs.

## Commands

```sh
rusk transpile input.rsk -o output.rs           # Rusk to Rust
rusk transpile input.rsk --source-map out.json  # also write a JSON source map
cat input.rsk | rusk                            # stdin to stdout
rusk from-rust input.rs -o output.rsk           # Rust to Rusk
rusk fmt input.rsk -o input.rsk                 # format
rusk run                                        # transpile a crate, then cargo run
rusk cargo test                                 # any cargo command
rusk script tool.rsk arg                        # run a single-file script
rusk add tool.rsk regex                         # add a script dependency
rusk-lsp                                        # language server on stdio
```

Without `-o`, output goes to stdout. Run without an input while stdin is a terminal, `rusk`
prints its help.
