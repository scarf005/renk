# Tools

## Formatter

`rusk fmt` trims trailing whitespace and validates indentation without reflowing expressions,
so both compact and already-broken chains keep their line breaks. The only option is
`--line-width`.

```sh
rusk fmt src/main.rsk -o src/main.rsk
rusk fmt src/main.rsk --line-width 120
```

## Cargo wrapper

`rusk` wraps Cargo commands. It transpiles `.rsk` files under Cargo source roots (`src`,
`examples`, `tests`, `benches`, and `build.rsk`) into generated `.rs` files, then runs Cargo.
Existing `.rs` files that rusk did not generate are never overwritten.

```sh
rusk run        # cargo run
rusk check      # cargo check
rusk cargo test # cargo test
```

## Scripts

`rusk script FILE.rsk [ARGS...]` builds a single-file script with
`cargo +nightly -Zscript build` and runs it. A script keeps the cargo-script `---`
frontmatter, and gets `package.edition = "2024"` when it names no edition.

```rsk
#!/usr/bin/env -S rusk script
---
[dependencies]
regex = "1"
---

fn main():
    println!("{}", regex::Regex::new("a+").unwrap().is_match("caab"))
```

- All scripts build in one shared `$XDG_CACHE_HOME/rusk/build/`, so each dependency compiles
  once. A script's package is named `<stem>-<hash>` unless it sets `package.name`; a script
  with its own name builds in a target directory of its own instead.
- A script without a top-level `fn main` runs its top-level lines in order inside
  `fn main() -> Result<(), Box<dyn std::error::Error>>`, so they can use `?`. A script that
  depends on `anyhow`, `eyre`, or `color-eyre` returns that crate's `Result<()>` instead, so
  its `bail!` works.
- `RUSK_SCRIPT` holds the script's absolute path, `CARGO_MANIFEST_DIR` and `CARGO_PKG_NAME`
  are set as `cargo run` sets them, and the script's exit code is rusk's.
- Compiler diagnostics point at the script's lines and quote its source.
  Cargo's failure summary names the script instead of the generated package.
- Scripts run their cached binary without cargo's freshness checks and rebuild only when the
  generated Rust changes; `rusk script --cargo FILE.rsk` builds through cargo on every run to
  pick up dependency and toolchain updates.
- `rusk add FILE.rsk [CARGO_ADD_ARGS...]` runs `cargo add` on the script's frontmatter, creating it
  when missing; the shebang and body stay as they were. A missing-crate error suggests it.

## Language server

`rusk-lsp` is a stdio Language Server Protocol server. It reports diagnostics and document
symbols for `.rsk` files, and proxies Rust features through the generated Rust when
`rust-analyzer` is available. A script, a `.rsk` file with a `#!` line or frontmatter or
outside a Cargo package, gets a `rust-analyzer` of its own over its generated cargo-script file,
so its frontmatter dependencies resolve. A Zed extension lives in
[`editors/zed`](https://github.com/scarf005/rusk/tree/main/editors/zed).

## Web runner

`podman compose up --build` serves the web editor at `http://localhost:8080`. It transpiles
in the browser and compiles and runs snippets with the image's `rustc`, limited to 64 KiB of
source, 256 KiB of output, two concurrent runs, and ten runs per minute.
