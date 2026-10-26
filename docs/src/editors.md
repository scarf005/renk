# Editor setup

`renk-lsp` is a Language Server Protocol server on stdio for `.rk` files. Install it with
`cargo install --path crates/renk --bin renk-lsp`. It provides:

- diagnostics for Renk syntax errors; type errors come from `renk check` or `renk script`,
- document symbols,
- hover, go to definition, and completion from `rust-analyzer` on the generated Rust, when
  `rust-analyzer` is on `PATH`.

A script, which is a `.rk` file with a `#!` line or frontmatter or outside a Cargo package,
transpiles as `renk script` does. It gets a `rust-analyzer` of its own over its generated
cargo-script file in `$XDG_CACHE_HOME/renk/lsp/`, so its frontmatter dependencies resolve.

## Zed

The extension in [`editors/zed`](https://github.com/scarf005/rusk/tree/main/editors/zed)
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
