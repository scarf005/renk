# Editor setup

`renk-lsp` is a Language Server Protocol server on stdio for `.rk` files. Install it with
`cargo install --path crates/renk --bin renk-lsp`. It provides:

- diagnostics for Renk syntax errors,
- document symbols,
- from `rust-analyzer` on the generated Rust, when `rust-analyzer` is on `PATH`: its
  diagnostics, including type errors from its `cargo check` on save; hover; go to definition;
  completion; find references; rename; code actions; and inlay hints.

Positions map back to the `.rk` file. A rename fails when it would change Renk text that
differs from the generated Rust, and a code action is left out when its edit does not map to
the `.rk` lines. A resolution error at an `a.b` dot carries the same hint as `renk check` (see
[Limits](limits.md)).

A script, which is a `.rk` file with a `#!` line or frontmatter or outside a Cargo package,
transpiles as `renk script` does. It gets a `rust-analyzer` of its own over its generated
cargo-script file in `$XDG_CACHE_HOME/renk/lsp/`, so its frontmatter dependencies resolve.

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
