---
name: renk-script
description: Write, run, and debug single-file Renk (.rk) scripts. Use when creating or editing a .rk script with a `#!/usr/bin/env -S renk script` shebang, adding script dependencies, or fixing a script build error.
---

# Renk scripts

Full reference: `docs/src/scripts.md` and `docs/src/syntax/`.

## Skeleton

```rk
#!/usr/bin/env -S renk script
---
[dependencies]
anyhow = "1"
---

fn main():
    let args = std.env.args().skip(1).collect[Vec[String]]()
    println!("{args.len()} args")
```

- Run: `renk tool.rk ARGS` (same as `renk script tool.rk ARGS`), or `chmod +x tool.rk && ./tool.rk ARGS`. Needs nightly.
- Show the generated Rust: `renk transpile tool.rk`.
- Add a dependency: `renk add tool.rk regex [--features f]`. Do not hand-write versions.
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

- A missing-crate error suggests the `renk add` command; run it.
- Diagnostics and panics point at `.rk` lines. Fix the script, not the cached Rust in `$XDG_CACHE_HOME/renk/scripts`.
- A ``renk read `a.b` as ...`` help names the other spelling of a misread dot; use it.
- Print visible output from `main`; do not discard results with `let _ =`.
