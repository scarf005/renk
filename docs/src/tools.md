# Tools

## Formatter

`renk fmt` lays out a file the way rustfmt lays out Rust: indentation, line breaks, and spacing.
It runs `rustfmt --edition 2024` on the generated Rust, so it needs rustfmt
(`rustup component add rustfmt`) and follows the `rustfmt.toml` of the working directory.
Comments and blank lines stay. Format arguments move into the string, so
`println!("result: {}", result)` becomes `println!("result: {result}")`. A script keeps its
`#!` line and frontmatter.
An item that cannot be formatted without changing its Rust keeps its lines, with trailing
whitespace trimmed.

```sh
renk fmt src/main.rk -o src/main.rk
renk fmt --check src/*.rk    # prints a diff and exits with 1 if a file is not formatted
renk fmt --write src         # rewrites each .rk file under src that is not formatted
```

`--check` and `--write` take files and directories. A directory stands for the `.rk` files
under it, except in `target`, `node_modules`, and hidden directories. Formatting more than one
file needs one of the two. `--check` also names a file with CRLF line endings or a byte order
mark, and `--write` removes the mark. An error starts with the file, line, and column, as in
`src/main.rk:3:1: tabs are not supported`, and formatting goes on with the next file.

## Source maps

`renk transpile FILE.rk --source-map MAP.json` also writes a JSON tree of the Renk syntax
nodes, each with its source line, column, and text and the generated Rust lines it became.

## Web runner

`podman compose up --build` serves the web editor at `http://localhost:8080`. It transpiles
in the browser and compiles and runs snippets with the image's `rustc`. `just web` starts the
editor from a checkout.

- The image builds the web UI at build time and bundles the `renk` CLI and a small `renk-web`
  HTTP server. Deno is used only in the build stage.
- The runner bundles `trpl` 0.3.0, so snippets can use helpers such as `trpl::block_on`
  without a Cargo manifest.
- The service binds to `127.0.0.1:8080` on the host. The container is read-only except for
  `/tmp`, where snippets are compiled and run under `RENK_RUN_TIMEOUT_MS`.
- Run requests are limited by default to 64 KiB of Rust source, 256 KiB of captured output, two
  concurrent runs, and ten runs per minute.
- Each compile and run step gets new user, mount, PID, network, IPC and UTS namespaces. The step
  can write only its own run directory. The server needs unprivileged user namespaces and Linux
  5.12 or later. At startup it logs whether the namespaces work. If they do not, the server still
  starts and each run fails. `RENK_WEB_UNSANDBOXED=1` runs code without namespaces.
- A program can read each file that the server user can read, except the run directories of
  other runs. Run a public server in the container.
- Behind a Cloudflare Tunnel, put Cloudflare Access and a `/api/run` rate limit in front of the
  service.
