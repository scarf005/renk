# Converting from Rust

`rusk from-rust` converts Rust to Rusk:

```sh
rusk from-rust src/lib.rs -o src/lib.rsk
rusk from-rust src/lib.rs -o src/lib.rsk --aggressive
```

The output uses dotted paths, `[T]` generics, `fn f() -> T = expr`, inferred `;`, and
`if c then a else b` only where the result transpiles back to the same Rust tokens. Format
arguments that each fill one placeholder in order are inlined: `format!("{}", cli.url)` becomes
`format!("{cli.url}")`. Explicit `::` paths and other turbofish calls are kept.

`--aggressive` also accepts rewrites that keep the Rust tokens but not their line layout, such
as multi-line `if`/`else` value chains.

Comments, string literals, and macro bodies are kept as written. Anything Rusk has no
indentation form for, such as a `{` block holding a `match` scrutinee, stays Rust syntax.

## Round-trip guarantee

The repository tests the round trip `Rust -> Rusk -> Rust` against a corpus of Rust files and
against pinned real crates. For each real crate, the round-tripped crate must compile, keep
every comment, and produce the same MIR as the original after normalizing source locations,
local and basic block numbering, and unit temporaries.

```sh
cargo test -p rusk --test real_crates -- --ignored --nocapture
```

The test downloads the crates from crates.io and needs a nightly toolchain for
`--emit=mir`.

The round trip does not keep blank lines or the exact `} else {` layout; run `rustfmt` on the
result to restore a canonical layout.
