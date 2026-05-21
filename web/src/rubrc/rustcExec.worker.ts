import {
  wait_async_polyfill,
  WASIFarmAnimal,
} from "@oligami/browser_wasi_shim-threads"

wait_async_polyfill()

self.onmessage = (event) => {
  const {
    compiler,
    wasiRefs,
    threadSpawnWorkerUrl,
    workerBackgroundWorkerUrl,
  } = event.data as {
    compiler: WebAssembly.Module
    wasiRefs: unknown[]
    threadSpawnWorkerUrl: string
    workerBackgroundWorkerUrl: string
  }

  try {
    const wasi = new WASIFarmAnimal(
      // deno-lint-ignore no-explicit-any
      wasiRefs as any,
      [
        "rustc",
        "/main.rs",
        "--sysroot",
        "/sysroot",
        "--target",
        "wasm32-wasip1",
        "--out-dir",
        "/tmp",
        "-Ccodegen-units=1",
        "-Clinker-flavor=wasm-ld",
        "-Clinker=wasm-ld",
      ],
      ["RUST_MIN_STACK=16777216"],
      {
        can_thread_spawn: true,
        thread_spawn_worker_url: threadSpawnWorkerUrl,
        thread_spawn_wasm: compiler,
        worker_background_worker_url: workerBackgroundWorkerUrl,
        share_memory: {
          memory: new WebAssembly.Memory({
            initial: 399,
            maximum: 16384,
            shared: true,
          }),
        },
      },
    )
    void (async () => {
      await wasi.wait_worker_background_worker()
      self.postMessage({ status: wasi.block_start_on_thread() })
    })()
  } catch (error) {
    self.postMessage({ error: stringifyError(error) })
  }
}

const stringifyError = (error: unknown) =>
  error instanceof Error ? error.message : String(error)
