# Changelog

## 0.2.0

### Breaking

- `TranspileError` has a new public field `column`, and it prints as `LINE:COLUMN: message` instead of `line LINE: message`.
- Every `renk` error that comes from an input file starts with the file, line and column, as in `src/main.rk:3:1: tabs are not supported`.
- `renk script SCRIPT ARGS` passes every argument after the script path to the script, `--` included.
- `renk build`, `check`, `run`, `test` and the other cargo commands transpile the `.rk` files of every package cargo can build, not only `src`, `examples`, `tests`, `benches` and `build.rk`.
- Those commands delete a generated `.rs` file when its `.rk` file is gone.
- `renk transpile` and `renk from-rust` refuse an output path that is the input file.
- `renk fmt` writes format arguments into the string: `println!("result: {}", result)` becomes `println!("result: {result}")`.
- `renk-web` answers `/api/run` with `403` unless the `Host` header is the listen address or is listed in `RENK_WEB_HOSTS`. Set it behind a reverse proxy.
- `renk-web` builds only on Linux and prints an error elsewhere.
- Editors that use the Tree-sitter grammar get new node names. Rewrite your own queries.

### Added

- `renk fmt --write` rewrites unformatted files in place, and `renk fmt` takes directories.
- `renk fmt --check` names a file with a byte order mark or CRLF line endings.
- `renk script` stops with `tool.rk:LINE: message` on an invalid frontmatter manifest.
- A relative `path` dependency in a script's frontmatter resolves from the script's directory.
- The `renk` crate has the features `cli`, `lsp`, `web` and `project-files`. The first three are on by default, and `cli` and `lsp` turn on `project-files`, which adds the `project` and `script` modules. Use `default-features = false` to depend on the library alone.
- `renk::rustfmt` and the `renk::project` module are public.
- `renk-lsp` answers signature help, document highlights and semantic tokens through `rust-analyzer`.
- `renk-lsp` formats documents as `renk fmt --write` does, and watches `.rk` files for changes and deletions.
- `renk-lsp` pulls diagnostics from `rust-analyzer`, restarts it when it exits and stops it on shutdown.
- A failed rename names the `.rk` file and line to edit by hand.
- The Tree-sitter grammar parses strings, comments, lifetimes, numbers and item headers as nodes, and has `dyn` and `super` keywords.
- The Tree-sitter grammar works with Neovim and Helix. See `editors/zed/README.md`.
- The web page runs the transpiler in a worker and shows transpile errors.
- `renk-web` gives each run its own namespaces, an address space limit of 1 GiB, at most 32 processes and a rate limit for each client address.

### Fixed

- A syntax error in a Renk file reports its column, and `renk-lsp` underlines it from that column.
- `renk from-rust` reports Rust with a syntax error and checks its rewrites for each item.
- A byte order mark at the start of a Renk, Rust or script file is ignored, and line 1 columns count it.
- `renk check` builds only the packages that cargo builds.
- Workspace `members` follow cargo's glob rules, and each package directory is listed once.
- Closure parameters, return types, match arm guards and operator lines continue onto the next lines.
- A struct literal opens after a lowercase type name, and a match after a range operator is an operand.
- `name[i](x)` is an index call when a `let`, parameter or pattern binds `name`.
- A macro call that ends an impl, mod or file gets a semicolon.
- `view!` tags end at the first top-level `>` and keep a `>` in generic arguments of the tag name.
- Only `std` and `core` macro paths count as std macros.
- Inline format arguments work after a quote in a comment or a lifetime.
- `renk from-rust` and `renk fmt` keep an integer format argument positional, as in `println!("{}", 1)`, because Rust reads `{1}` as a position. `{true}` and `{false}` in a format string become arguments.
- Rust attributes keep their tokens when converted to Renk.
- `renk-lsp` accepts any-case `Content-Length` headers and reads bodies as they arrive.
- `renk-lsp` maps completions, symbols and positions at the end of a line to Renk text.
- `renk-lsp` mirrors the workspace that `renk::project` finds for each file.
- `renk-lsp` drops code actions that change nothing or do not transpile back.
- `renk-web` hides `/proc` from the run step when the container masks it.

### Performance

- `view!` closure bodies end in linear time. Comment runs, closure parameters, where clauses, macro matchers and raw strings are scanned once.
- `renk-lsp` transpiles each document version once and reads the mirror on a blocking thread.
- The web page loads the WebAssembly module as a file instead of inline.

## 0.1.0

First release.
