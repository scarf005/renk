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

The `wasmbuild` task emits inline browser-compatible bindings to `src/wasm/` before Vite starts or builds.

## Self-host with Podman

```sh
podman compose up --build
```

The image build uses Deno to produce static files, then serves them with `renk-web`; the runtime image contains `rustc` and `renk`, but no Deno binary is needed.
