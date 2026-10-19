# Getting started

Install the CLI and the language server from a checkout:

```sh
cargo install --path crates/rusk --bin rusk --bin rusk-lsp
```

## Commands

```sh
rusk input.rsk -o output.rs                     # Rusk to Rust
rusk input.rk -o output.rs                      # Ruk to Rust
rusk transpile input.rsk --source-map out.json  # with a JSON source map
rusk from-rust input.rs -o output.rsk           # Rust to Rusk
rusk from-rust input.rs -o output.rsk --aggressive
rusk to-ruk input.rs -o output.rk               # Rust or Rusk to Ruk
rusk fmt input.rsk -o input.rsk --line-width 100
cat input.rsk | rusk                            # stdin to stdout
rusk run                                        # cargo run on a Rusk crate
rusk script tool.rsk arg                        # single-file script
```

Without `-o`, output goes to stdout.

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

Then run `rusk hello.rsk -o hello.rs && rustc --edition 2024 hello.rs && ./hello`, which prints
`3`.

The [examples](https://github.com/scarf005/rusk/tree/main/examples) directory has a `.rsk` and a
`.rk` version of each program.
