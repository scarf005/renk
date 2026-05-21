import { WASIFarmAnimal } from "@oligami/browser_wasi_shim-threads"

self.onmessage = (event) => {
  const { bytes, wasiRefs } = event.data as {
    bytes: ArrayBuffer
    wasiRefs: unknown[]
  }

  void (async () => {
    try {
      const module = await WebAssembly.compile(bytes)
      const wasi = new WASIFarmAnimal(
        // deno-lint-ignore no-explicit-any
        wasiRefs as any,
        ["/tmp/main.wasm"],
        [],
      )
      const instance = await WebAssembly.instantiate(module, {
        wasi_snapshot_preview1: wasi.wasiImport,
      }) as WebAssembly.Instance & {
        exports: { memory: WebAssembly.Memory; _start: () => unknown }
      }
      self.postMessage({ status: wasi.start(instance) })
    } catch (error) {
      self.postMessage({ error: stringifyError(error) })
    }
  })()
}

const stringifyError = (error: unknown) =>
  error instanceof Error ? error.message : String(error)
