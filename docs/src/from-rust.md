# Converting from Rust

`renk from-rust` converts Rust to Renk:

```sh
renk from-rust src/lib.rs -o src/lib.rk
renk from-rust src/lib.rs -o src/lib.rk --aggressive
```

The output uses dotted paths, `[T]` generics, `fn f() -> T = expr`, inferred `;`, and
`if c then a else b` only where the result transpiles back to the same Rust tokens. Format
arguments that each fill one placeholder in order are inlined: `format!("{}", cli.url)` becomes
`format!("{cli.url}")`. `::` paths become dotted paths where that reads back the same:
`std::env::args()` becomes `std.env.args()`, while `Config::DEFAULT.len()` keeps its `::`. A
turbofish call becomes `name[...](`, as in `s.parse[i32]()`, and a turbofish on a CamelCase path
becomes `Name[...]`, as in `Vec[String].new()`; other turbofish stays Rust.
Index-then-call receivers and path value receivers are parenthesized, as in `(handlers[i])()`
and `(std.cmp.Ordering.Less).reverse()`.

`--aggressive` also accepts rewrites that keep the Rust tokens but not their line layout, such
as multi-line `if`/`else` value chains.

Comments, string literals, and macro bodies are kept as written. Anything Renk has no
indentation form for, such as a `{` block holding a `match` scrutinee, stays Rust syntax.

## Round-trip tests

The repository tests the round trip `Rust -> Renk -> Rust` on a corpus of Rust files and on
pinned real crates. For each real crate, the round-tripped crate must compile, keep
every comment, and produce the same MIR as the original after normalizing source locations,
local and basic block numbering, and unit temporaries. MIR is compared with default features,
with every feature, and for the unit tests, wherever the original builds that way.

```sh
cargo test -p renk --test real_crates -- --ignored --nocapture
```

The test downloads the crates from crates.io and needs a nightly toolchain for
`--emit=mir`.
