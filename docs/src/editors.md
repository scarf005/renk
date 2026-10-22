# Editor setup

`rusk-lsp` is a Language Server Protocol server on stdio for `.rsk` files. Install it with
`cargo install --path crates/rusk --bin rusk-lsp`. It provides:

- diagnostics for Rusk syntax errors; type errors come from `rusk check` or `rusk script`,
- document symbols,
- hover, go to definition, and completion from `rust-analyzer` on the generated Rust, when
  `rust-analyzer` is on `PATH`.

A script, which is a `.rsk` file with a `#!` line or frontmatter or outside a Cargo package,
transpiles as `rusk script` does. It gets a `rust-analyzer` of its own over its generated
cargo-script file in `$XDG_CACHE_HOME/rusk/lsp/`, so its frontmatter dependencies resolve.

## Zed

The extension in [`editors/zed`](https://github.com/scarf005/rusk/tree/main/editors/zed)
registers the Rusk language, starts `rusk-lsp`, and highlights with Tree-sitter. Install it
with **Install Dev Extension** in Zed's extensions page, selecting `editors/zed`.

## Neovim

Neovim 0.11 or later:

```lua
vim.filetype.add({ extension = { rsk = "rusk" } })
vim.lsp.config("rusk", {
  cmd = { "rusk-lsp" },
  filetypes = { "rusk" },
  root_markers = { "Cargo.toml", ".git" },
})
vim.lsp.enable("rusk")
```

## Other editors

Configure a language server with the command `rusk-lsp`, no arguments, over stdio, for files
ending in `.rsk`.
