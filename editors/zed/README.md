# Renk Zed extension

Local Zed extension for `.rk` files.

## Install the LSP

```sh
cargo install --path crates/renk --bin renk-lsp
```

`renk-lsp` is a standard stdio Language Server Protocol server and can be used by Zed, Vim, VS Code, or any other LSP client.

## Install the Zed extension locally

1. Open Zed extensions.
2. Choose `Install Dev Extension`.
3. Select this directory: `editors/zed`.

The extension registers the `Renk` language, starts `renk-lsp`, and provides Tree-sitter highlighting for `.rk` files.

If `renk-lsp` is not on Zed's PATH, configure it in Zed settings:

```json
{
  "lsp": {
    "renk-lsp": {
      "binary": {
        "path": "/absolute/path/to/renk-lsp"
      }
    }
  }
}
```

## Other editors

`tree-sitter-renk` has the layout tree-sitter expects, with `queries/highlights.scm` linked to the Zed query.
Build the parser with `tree-sitter build -o renk.so` in that directory.

- Neovim: register the library with `vim.treesitter.language.add("renk", { path = "renk.so" })` and copy
  `queries/highlights.scm` to `queries/renk/highlights.scm` in your runtime path.
- Helix: add a `[[language]]` and a `[[grammar]]` with `source = { path = "<repository>/editors/zed/tree-sitter-renk" }`
  to `languages.toml`, run `hx --grammar build`, and copy `queries/highlights.scm` to
  `runtime/queries/renk/highlights.scm` in the config directory.
