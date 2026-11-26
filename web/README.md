# Renk Web Demo

A Vite + Deno + Preact demo for `renk`. The Rust/Renk converters are compiled to WebAssembly with [`denoland/wasmbuild`](https://github.com/denoland/wasmbuild), then called from Preact Signals for instant browser-side output.

## Running

```sh
deno task dev
```

## Build

```sh
deno task build
```

The `wasmbuild` task emits browser-compatible bindings and an optimized `renk.wasm` to `src/wasm/` before Vite starts or builds.

## Self-host with Podman

```sh
podman compose up --build
```

The image build uses Deno to produce static files, then serves them with `renk-web`; the runtime image contains `rustc` and `renk`, but no Deno binary is needed.

### Behind a reverse proxy

`/api/run` answers `403` with `{"error":"forbidden"}` unless the `Host` header is one of these values:

- the listen address of the server, `localhost:<port>`, `127.0.0.1:<port>` or `[::1]:<port>`
- a value in `RENK_WEB_HOSTS`, a list separated by commas

An `Origin` header must have the same host. Behind a reverse proxy:

1. Set `RENK_WEB_HOSTS` to the public host names, for example `RENK_WEB_HOSTS=renk.example.com podman compose up --build`. Add the port when the public URL has one: `renk.example.com:8443`.
2. Make the proxy pass the `Host` header of the browser unchanged. For nginx, add `proxy_set_header Host $http_host;`.

`RENK_RUN_RATE_LIMIT_PER_MINUTE` limits the runs per client address, or per /64 network for IPv6. The server sees only the address of the proxy, so all clients behind one proxy share one limit.

### Sandbox

Each compile and run step gets new user, mount, PID, network, IPC and UTS namespaces. The step sees only its own run directory in the temporary directory, and its own `/proc` with only its own processes. The rest of the file system stays visible with the rights of the server user, so the server user must own no private files. The compose file runs the server as uid 10001 with a read-only root. The run step gets at most 1 GiB of address space and 32 processes and threads.

When a container masks paths below `/proc` (podman and docker do), the kernel refuses the new `/proc`. The program then runs without `/proc`. The compile step sees the `/proc` of the server, and `include_bytes!` with a literal path can read from it, so the environment of the server must hold no secret.

The server needs unprivileged user namespaces for this. At start it logs one of these lines:

- `renk-web: namespaces work`
- `renk-web: namespaces work, /proc is hidden (<error>)`, when the container masks `/proc`
- `renk-web: namespaces do not work (<error>)`. Every run then fails.

Set `RENK_WEB_UNSANDBOXED=1` to run programs without namespaces. A submitted program can then read the files of the server user, use the network of the server and see other runs. Use it only when a container or a VM isolates the server.
