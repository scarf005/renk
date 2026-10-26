# Scripts

`renk script [--cargo] FILE.rk [ARGS...]`, or just `renk FILE.rk [ARGS...]`, transpiles a single-file script, builds it with
`cargo +nightly -Zscript`, and runs it with `ARGS`. It needs a nightly toolchain.

```rk
#!/usr/bin/env -S renk script
---
[dependencies]
regex = "1"
---

fn main():
    println!("{}", regex.Regex.new("a+").unwrap().is_match("caab"))
```

With that `#!` line and `chmod +x tool.rk`, `./tool.rk ARGS` runs the script.

`renk transpile tool.rk` prints the Rust that `renk script` builds: the frontmatter is kept and
top-level lines are wrapped in `fn main`. `--source-map` is rejected for scripts. A file is a
script when it starts with a `#!` line or `---` frontmatter.

## Dependencies

Dependencies go in the cargo-script `---` frontmatter, which is a Cargo manifest. A script that
names no edition gets `package.edition = "2024"`.

`renk add FILE.rk [CARGO_ADD_ARGS...]` runs `cargo add` on the frontmatter and writes the
result back. It creates the frontmatter after the `#!` line when there is none and leaves the
rest of the script untouched. Relative `--path` dependencies resolve from the script's
directory.

```sh
renk add tool.rk regex
renk add tool.rk serde --features derive
```

A build that fails on a crate missing from the frontmatter suggests the command, as in
``add it with `renk add tool.rk regex` ``.

## Top-level statements

A script without a top-level `fn main` runs its top-level lines in order inside
`fn main() -> Result<(), Box<dyn std::error::Error>>`, so they can use `?`:

```rk
#!/usr/bin/env -S renk script

let args = std.env.args().skip(1).collect[Vec[String]]()
let n: i32 = args.first().map_or("2", |s| s.as_str()).parse()?
println!("{n * 2}")
```

A script that depends on `anyhow`, `eyre`, or `color-eyre` returns that crate's `Result<()>`
instead, so its `bail!` works. A top-level `return` must be `return Ok(())`.

## Environment

- `RENK_SCRIPT` holds the script's absolute path.
- `CARGO_MANIFEST_DIR` and `CARGO_PKG_NAME` are set as `cargo run` sets them.
- renk exits with the script's exit code.

## Builds and caching

- The generated Rust goes to `$XDG_CACHE_HOME/renk/scripts/` (`~/.cache/renk/scripts/` when
  `XDG_CACHE_HOME` is unset) and is rewritten only when it changes.
- A run execs the cached binary without cargo and rebuilds only when the generated Rust
  changes. `renk script --cargo FILE.rk` builds through cargo on every run, which picks up
  dependency and toolchain updates.
- All scripts build in one shared `$XDG_CACHE_HOME/renk/build/`, so each dependency compiles
  once. Scripts building at the same time wait for each other. Deleting that directory costs a
  rebuild and drops the scripts' `Cargo.lock` files, so the next build can resolve newer
  dependency versions.
- renk names a script's package `<stem>-<hash>`, which `CARGO_PKG_NAME` and the binary name
  show. A script that sets its own `package.name` keeps it and builds in a target directory
  next to its cached Rust instead, even under a user `CARGO_TARGET_DIR`.

## Diagnostics

- Compiler warnings print only on runs that compile the script.
- Diagnostics point at the script's lines and columns and quote its source. A span on a line
  renk synthesizes, such as a closing brace, points at the end of the script line it closes.
- rustc's suggested replacements are Rust code, so only their message shows.
- When renk named the package, cargo's failure summary names the script, as in
  ``could not compile `tool.rk` ``.
