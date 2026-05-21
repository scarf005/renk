import { RUBRC_DOWNLOAD_LABEL } from "./artifacts.ts"

export { RUBRC_DOWNLOAD_LABEL }

export type BrowserRunState = "idle" | "loading" | "ready" | "running" | "error"

export type BrowserRunResponse = {
  ok: boolean
  stage: "compile" | "run"
  status: number | null
  stdout: string
  stderr: string
  timedOut: boolean
  stdoutTruncated: boolean
  stderrTruncated: boolean
  compileMs: number
  runMs: number
  totalMs: number
}

type Pending = {
  resolve: (value: BrowserRunResponse | void) => void
  reject: (error: Error) => void
  onProgress: (message: string) => void
}

let worker: Worker | null = null
let nextId = 1
const pending = new Map<number, Pending>()

export const isBrowserRunSupported = () =>
  typeof Worker === "function" &&
  typeof SharedArrayBuffer === "function" &&
  globalThis.crossOriginIsolated === true

export const loadRubrc = (onProgress: (message: string) => void) =>
  request("load", undefined, onProgress) as Promise<void>

export const runRustInBrowser = (
  rust: string,
  onProgress: (message: string) => void,
) => request("run", rust, onProgress) as Promise<BrowserRunResponse>

const request = (
  type: "load" | "run",
  rust: string | undefined,
  onProgress: (message: string) => void,
) => {
  const current = getWorker()
  const id = nextId++
  const promise = new Promise<BrowserRunResponse | void>((resolve, reject) => {
    pending.set(id, { resolve, reject, onProgress })
  })
  current.postMessage({ id, type, rust })
  return promise
}

const getWorker = () => {
  if (worker) return worker
  const newWorker = new Worker(new URL("./runner.worker.ts", import.meta.url), {
    type: "module",
  })
  worker = newWorker
  newWorker.onmessage = (event) => {
    const { id, type, result, error, message } = event.data as {
      id?: number
      type: "ready" | "result" | "error" | "progress"
      result?: BrowserRunResponse
      error?: string
      message?: string
    }
    if (type === "progress") {
      if (id) pending.get(id)?.onProgress(message ?? "Loading Rubrc...")
      else {for (const item of pending.values()) {
          item.onProgress(message ?? "Loading Rubrc...")
        }}
      return
    }
    if (!id) return
    const item = pending.get(id)
    if (!item) return
    pending.delete(id)
    if (type === "error") item.reject(new Error(error ?? "Rubrc failed"))
    else if (type === "result") item.resolve(result)
    else item.resolve()
  }
  newWorker.onerror = (event) => {
    const error = new Error(event.message || "Rubrc worker failed")
    for (const [id, item] of pending) {
      pending.delete(id)
      item.reject(error)
    }
    worker?.terminate()
    worker = null
  }
  return newWorker
}
