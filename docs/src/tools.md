# Tools

## Formatter

`rusk fmt` checks that the file parses, trims trailing whitespace, empties whitespace-only
lines, and ends the file with one newline. It does not reflow expressions or change
indentation.

```sh
rusk fmt src/main.rsk -o src/main.rsk
```

## Source maps

`rusk transpile FILE.rsk --source-map MAP.json` also writes a JSON tree of the Rusk syntax
nodes, each with its source line, column, and text and the generated Rust lines it became.

## Web runner

`podman compose up --build` serves the web editor at `http://localhost:8080`. It transpiles
in the browser and compiles and runs snippets with the image's `rustc`. `just web` starts the
editor from a checkout.

- The image builds the web UI at build time and bundles the `rusk` CLI and a small `rusk-web`
  HTTP server. Deno is used only in the build stage.
- The runner bundles `trpl` 0.3.0, so snippets can use helpers such as `trpl::block_on`
  without a Cargo manifest.
- The service binds to `127.0.0.1:8080` on the host. The container is read-only except for
  `/tmp`, where snippets are compiled and run under `RUSK_RUN_TIMEOUT_MS`.
- Run requests are limited by default to 64 KiB of Rust source, 256 KiB of captured output, two
  concurrent runs, and ten runs per minute.
- Behind a Cloudflare Tunnel, put Cloudflare Access and a `/api/run` rate limit in front of the
  service.
