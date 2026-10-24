---
name: rusk-script
description: Write, run, and debug single-file Rusk (.rsk) scripts. Use when creating or editing a .rsk script with a `#!/usr/bin/env -S rusk script` shebang, adding script dependencies, or fixing a script build error.
---

# Rusk scripts

Full reference: `docs/src/scripts.md` and `docs/src/syntax/`.

## Skeleton

```rsk
#!/usr/bin/env -S rusk script
---
[dependencies]
anyhow = "1"
---

fn main():
    let args = std.env.args().skip(1).collect[Vec[String]]()
    println!("{args.len()} args")
```

- Run: `rusk tool.rsk ARGS` (same as `rusk script tool.rsk ARGS`), or `chmod +x tool.rsk && ./tool.rsk ARGS`. Needs nightly.
- Show the generated Rust: `rusk transpile tool.rsk`.
- Add a dependency: `rusk add tool.rsk regex [--features f]`. Do not hand-write versions.
- Without `fn main`, top-level lines run in a `main` that returns `Result`, so `?` works.
  With `anyhow`, `eyre`, or `color-eyre` in the frontmatter it returns that crate's `Result`.
- `--cargo` rebuilds through cargo; use it after dependency or toolchain updates.

## Syntax differences from Rust

- Blocks open with `:` and end by indentation; no braces or trailing `;`.
- `fn f() -> T = expr` for one-line bodies; `if c then a else b` for one-line `if`.
- `a.b` is a path (`std.fs.read_to_string`, `Foo.new()`) unless `a` is a bound value, `self`, or a
  `SCREAMING_CASE` constant. Force a path with `::`, a value receiver with `(a).b`.
- Generics use `[T]`: `collect[Vec[String]]()`, `Vec[u8].new()`. Other `[...]` is indexing.
- `println!("{user.name()}")` takes expressions inside `{}`; `\"` escapes inside the placeholder.
- Struct literals stay Rust-style: `Self{ id, name }`.
- Plain Rust syntax is always accepted when the shorthand is ambiguous.

## Checks

- A missing-crate error suggests the `rusk add` command; run it.
- Diagnostics point at `.rsk` lines. Fix the script, not the cached Rust in `$XDG_CACHE_HOME/rusk/scripts`.
- Print visible output from `main`; do not discard results with `let _ =`.
