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
