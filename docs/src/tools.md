# Tools

## Formatter

`rusk fmt` trims trailing whitespace and validates indentation. It does not reflow
expressions, so compact and already-broken chains keep their line breaks. The only option is
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

- The generated Rust goes to `$XDG_CACHE_HOME/rusk/scripts/` and is rewritten only when it
  changes. All scripts build in one shared `$XDG_CACHE_HOME/rusk/build/`, so each dependency
  compiles once. Scripts building at the same time wait for each other.
- rusk names a script's package `<stem>-<hash>`, which `CARGO_PKG_NAME` and the binary name
  show. A script that sets its own `package.name` keeps it and builds in a target directory
  next to its cached Rust instead, even under a user `CARGO_TARGET_DIR`.
- Deleting the shared build directory costs a rebuild and drops the scripts' `Cargo.lock`
  files, so the next build can resolve newer dependencies.
- A script without a top-level `fn main` runs its top-level lines in order inside
  `fn main() -> Result<(), Box<dyn std::error::Error>>`, so they can use `?`. A script that
  depends on `anyhow`, `eyre`, or `color-eyre` returns that crate's `Result<()>`, so its
  `bail!` works. A top-level `return` then needs `return Ok(())`.
- `RUSK_SCRIPT` holds the script's absolute path. `CARGO_MANIFEST_DIR` and `CARGO_PKG_NAME`
  are set as `cargo run` sets them. The script's exit code is rusk's.
- Scripts exec their cached binary, skipping cargo's per-run resolution and freshness checks,
  and rebuild only when the generated Rust changes. `rusk script --cargo FILE.rsk` builds
  through cargo on every run to pick up dependency and toolchain updates.
- Compiler warnings print only on runs that compile the script. Diagnostics point at the
  script's lines and columns and quote its source. A span on a line rusk synthesizes, such as a
  closing brace, points at the end of the script line it closes. rustc's suggested
  replacements are Rust, so only their message shows. When rusk named the package, cargo's
  failure summary names the script instead.
- `rusk add FILE.rsk [CARGO_ADD_ARGS...]` runs `cargo add` on a scratch copy of the script's
  frontmatter, written beside the script, and puts the result back. It creates the frontmatter
  after the shebang when missing and leaves the rest of the script untouched. A missing-crate
  error suggests it, e.g. `rusk add tool.rsk regex`.

## Language server

`rusk-lsp` is a stdio Language Server Protocol server. It reports diagnostics and document
symbols for `.rsk` files, and proxies Rust features through the generated Rust when
`rust-analyzer` is available. A script, a `.rsk` file with a `#!` line or frontmatter or
outside a Cargo package, transpiles as `rusk script` does and gets a `rust-analyzer` of its
own over its generated cargo-script file, so its frontmatter dependencies resolve. That file
lives in `$XDG_CACHE_HOME/rusk/lsp/`. A Zed extension lives in
[`editors/zed`](https://github.com/scarf005/rusk/tree/main/editors/zed).

## Web runner

`podman compose up --build` serves the web editor at `http://localhost:8080`. It transpiles
in the browser and compiles and runs snippets with the image's `rustc`. `just web` starts the
editor from a checkout.

```sh
podman compose up --build
```

- The image builds the web UI at build time and bundles the `rusk` CLI and a small `rusk-web`
  HTTP server. Deno is used only in the build stage.
- The runner bundles `trpl` 0.3.0, so examples can use helpers such as `trpl::block_on`
  without a Cargo manifest. It does not relax the child-process sandbox, which governs network
  access.
- The service binds to `127.0.0.1:8080` on the host, for local use or a local Cloudflare Tunnel
  origin. The container is read-only except for `/tmp`, where snippets are compiled and run
  under `RUSK_RUN_TIMEOUT_MS`.
- Run requests are limited by default to 64 KiB of Rust source, 256 KiB of captured output, two
  concurrent runs, and ten runs per minute.
- Behind a Cloudflare Tunnel, put Cloudflare Access and a `/api/run` rate limit in front of the
  service.
