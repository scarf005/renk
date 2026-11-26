export type WasmFunction =
  | "transpile_to_rust"
  | "transpile_syntax_tree_json"
  | "format_renk"
  | "convert_rust_to_renk"

export type EngineRequest = { id: number; fn: WasmFunction; source: string }
export type EngineMessage =
  | { ready: true }
  | { failed: string }
  | { id: number; value: string }
  | { id: number; error: string }

export type WorkerLike = {
  postMessage(message: EngineRequest): void
  addEventListener(
    type: "message",
    listener: (event: { data: EngineMessage }) => void,
  ): void
}

export type Engine = {
  ready: Promise<void>
  call(fn: WasmFunction, source: string): Promise<string>
}

export const createEngine = (worker: WorkerLike): Engine => {
  const pending = new Map<number, PromiseWithResolvers<string>>()
  const ready = Promise.withResolvers<void>()
  let nextId = 0

  worker.addEventListener("message", ({ data }) => {
    if ("ready" in data) ready.resolve()
    else if ("failed" in data) ready.reject(new Error(data.failed))
    else {
      const call = pending.get(data.id)
      pending.delete(data.id)
      if ("error" in data) call?.reject(new Error(data.error))
      else call?.resolve(data.value)
    }
  })

  return {
    ready: ready.promise,
    call: (fn, source) => {
      const id = nextId++
      const call = Promise.withResolvers<string>()
      pending.set(id, call)
      worker.postMessage({ id, fn, source })
      return call.promise
    },
  }
}
