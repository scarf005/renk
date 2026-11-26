# Editor setup

`renk-lsp` is a Language Server Protocol server on stdio for `.rk` files. Install it with
`cargo install renk --bin renk-lsp`. It provides:

- diagnostics for Renk syntax errors,
- document symbols,
- formatting that gives the text `renk fmt --write` writes, which needs `rustfmt`,
- hover, go to definition, and completion. Without `rust-analyzer` they answer from the
  declarations, keywords, and builtin types of the open file,
- from `rust-analyzer` on the generated Rust, when `rust-analyzer` is on `PATH`: its
  diagnostics, including type errors from its `cargo check` on save; hover with types; go to
  definition into other files and crates; and completion of methods and names in scope.
  Signature help, highlights of a name, semantic tokens, find references, rename, code actions,
  and inlay hints need it.

Positions map back to the `.rk` file. A rename fails when it would change Renk text that
differs from the generated Rust. The error names the `.rk` file and line to rename by hand. It
also fails, naming the file, when a `.rk` file it edits does not transpile. A highlight or a
semantic token is left out where the `.rk` text differs from the Rust, as at some names in
format strings. A code action is left out when its edit does not map to the `.rk` lines. A
resolution error at an `a.b` dot carries the same hint as `renk check` (see [Limits](limits.md)).

A script, which is a `.rk` file with a `#!` line or frontmatter or outside a Cargo package,
transpiles as `renk script` does. It gets a `rust-analyzer` of its own over its generated
cargo-script file in `$XDG_CACHE_HOME/renk/lsp/`, so its frontmatter dependencies resolve. A
relative dependency `path` starts at the script's folder.

## Zed

The extension in [`editors/zed`](https://github.com/scarf005/renk/tree/main/editors/zed)
registers the Renk language, starts `renk-lsp`, and highlights with Tree-sitter. Install it
with **Install Dev Extension** in Zed's extensions page, selecting `editors/zed`.

## Neovim

Neovim 0.11 or later:

```lua
vim.filetype.add({ extension = { rk = "renk" } })
vim.lsp.config("renk", {
  cmd = { "renk-lsp" },
  filetypes = { "renk" },
  root_markers = { "Cargo.toml", ".git" },
})
vim.lsp.enable("renk")
```

## Other editors

Configure a language server with the command `renk-lsp`, no arguments, over stdio, for files
ending in `.rk`.
