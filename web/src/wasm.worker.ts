import * as bindings from "./wasm/renk.internal.js"
import type { EngineMessage, EngineRequest } from "./engine.ts"

const post = (message: EngineMessage) => globalThis.postMessage(message)

const ready = WebAssembly.instantiateStreaming(
  fetch(new URL("./wasm/renk.wasm", import.meta.url)),
  { "./renk.internal.js": bindings },
).then(({ instance }) => {
  bindings.__wbg_set_wasm(instance.exports)
  ;(instance.exports.__wbindgen_start as () => void)()
})

ready.then(
  () => post({ ready: true }),
  (error: Error) => post({ failed: String(error?.message ?? error) }),
)

globalThis.addEventListener(
  "message",
  async ({ data: { id, fn, source } }: MessageEvent<EngineRequest>) => {
    try {
      await ready
      post({ id, value: bindings[fn](source) })
    } catch (error) {
      post({ id, error: String((error as Error)?.message ?? error) })
    }
  },
)
