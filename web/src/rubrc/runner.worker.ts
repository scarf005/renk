import {
  Directory,
  File,
  type Inode,
  PreopenDirectory,
} from "@bjorn3/browser_wasi_shim"
import {
  wait_async_polyfill,
  WASIFarm,
} from "@oligami/browser_wasi_shim-threads"
import { RUBRC_DOWNLOAD_LABEL, rubrcAssetUrl } from "./artifacts.ts"
import { CapturingFd } from "./fd.ts"
import { getWasmModule } from "./brotliStream.ts"
import { loadDefaultSysroot } from "./sysroot.ts"
// @ts-ignore Vite worker URL query.
import threadSpawnWorkerUrl from "./threadSpawn.worker.ts?worker&url"
// @ts-ignore Vite worker URL query.
import workerBackgroundWorkerUrl from "./workerBackground.worker.ts?worker&url"
// @ts-ignore Vite worker URL query.
import rustcExecWorkerUrl from "./rustcExec.worker.ts?worker&url"
// @ts-ignore Vite worker URL query.
import wasmExecWorkerUrl from "./wasmExec.worker.ts?worker&url"

wait_async_polyfill()

type RunResponse = {
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

type ReadyState = {
  compiler: WebAssembly.Module
  wasiRefs: unknown[]
  mainFile: File
  tmpDir: Directory
  stdout: CapturingFd
  stderr: CapturingFd
}

let ready: Promise<ReadyState> | null = null

self.onmessage = (event: MessageEvent) => {
  const { id, type, rust } = event.data as {
    id: number
    type: "load" | "run"
    rust?: string
  }

  void (async () => {
    try {
      if (type === "load") {
        await setup()
        post({ id, type: "ready" })
        return
      }
      if (typeof rust !== "string") throw new Error("rust must be a string")
      post({ id, type: "progress", message: "Compiling in browser..." })
      post({ id, type: "result", result: await compileAndRun(rust) })
    } catch (error) {
      post({ id, type: "error", error: stringifyError(error) })
    }
  })()
}

const setup = async () => {
  ready ??= setupRubrc()
  return await ready
}

const setupRubrc = async (): Promise<ReadyState> => {
  assertSupported()
  post({
    type: "progress",
    message: `Downloading Rubrc (${RUBRC_DOWNLOAD_LABEL})...`,
  })
  const [compiler, sysroot] = await Promise.all([
    getWasmModule(rubrcAssetUrl("rustc_opt.wasm.br")),
    loadDefaultSysroot(),
  ])

  const stdout = new CapturingFd()
  const stderr = new CapturingFd()
  const mainFile = new File(new Uint8Array())
  const tmpDir = new Directory([])
  const rootDir = new PreopenDirectory(
    "/",
    new Map<string, Inode>([
      ["sysroot", new Directory([])],
      ["tmp", tmpDir],
      ["main.rs", mainFile],
    ]),
  )

  const sysrootFarm = new WASIFarm(undefined, undefined, undefined, [sysroot], {
    allocator_size: 1024 * 1024 * 1024,
  })
  const rootFarm = new WASIFarm(stdout, stdout, stderr, [rootDir], {
    allocator_size: 1024 * 1024 * 1024,
  })

  return {
    compiler,
    wasiRefs: [sysrootFarm.get_ref(), rootFarm.get_ref()],
    mainFile,
    tmpDir,
    stdout,
    stderr,
  }
}

const compileAndRun = async (rust: string): Promise<RunResponse> => {
  const state = await setup()
  state.mainFile.data = new TextEncoder().encode(rust)
  state.tmpDir.contents.clear()
  state.stdout.reset()
  state.stderr.reset()

  const totalStarted = performance.now()
  const compileStarted = performance.now()
  const compileStatus = await runRustc(state)
  const compileMs = performance.now() - compileStarted
  const compileStdout = state.stdout.text()
  const compileStderr = state.stderr.text()

  if (compileStatus !== 0) {
    return {
      ok: false,
      stage: "compile",
      status: compileStatus,
      stdout: compileStdout,
      stderr: compileStderr,
      timedOut: false,
      stdoutTruncated: false,
      stderrTruncated: false,
      compileMs,
      runMs: 0,
      totalMs: performance.now() - totalStarted,
    }
  }

  state.stdout.reset()
  state.stderr.reset()
  post({ type: "progress", message: "Running WASI output..." })
  const runStarted = performance.now()
  const runStatus = await runMainWasm(state)
  const runMs = performance.now() - runStarted

  return {
    ok: runStatus === 0,
    stage: "run",
    status: runStatus,
    stdout: state.stdout.text(),
    stderr: state.stderr.text(),
    timedOut: false,
    stdoutTruncated: false,
    stderrTruncated: false,
    compileMs,
    runMs,
    totalMs: performance.now() - totalStarted,
  }
}

const runRustc = async (state: ReadyState): Promise<number> =>
  await new Promise((resolve, reject) => {
    const worker = new Worker(rustcExecWorkerUrl, { type: "module" })
    worker.onmessage = (event) => {
      const { status, error } = event.data as {
        status?: number
        error?: string
      }
      worker.terminate()
      if (error) reject(new Error(error))
      else resolve(status ?? 1)
    }
    worker.onerror = (event) => {
      worker.terminate()
      reject(new Error(event.message || "rustc worker failed"))
    }
    worker.postMessage({
      compiler: state.compiler,
      wasiRefs: state.wasiRefs,
      threadSpawnWorkerUrl,
      workerBackgroundWorkerUrl,
    })
  })

const runMainWasm = async (state: ReadyState): Promise<number> => {
  const output = state.tmpDir.contents.get("main.wasm")
  if (!(output instanceof File)) {
    throw new Error("rustc did not emit /tmp/main.wasm")
  }

  const bytes = output.data.buffer.slice(
    output.data.byteOffset,
    output.data.byteOffset + output.data.byteLength,
  ) as ArrayBuffer
  return await new Promise((resolve, reject) => {
    const worker = new Worker(wasmExecWorkerUrl, { type: "module" })
    worker.onmessage = (event) => {
      const { status, error } = event.data as {
        status?: number
        error?: string
      }
      worker.terminate()
      if (error) reject(new Error(error))
      else resolve(status ?? 1)
    }
    worker.onerror = (event) => {
      worker.terminate()
      reject(new Error(event.message || "WASI output worker failed"))
    }
    worker.postMessage({ bytes, wasiRefs: state.wasiRefs }, [bytes])
  })
}

const assertSupported = () => {
  if (!globalThis.crossOriginIsolated) {
    throw new Error("SharedArrayBuffer requires cross-origin isolation")
  }
  if (typeof SharedArrayBuffer !== "function") {
    throw new Error("SharedArrayBuffer is not supported by this browser")
  }
  if (
    typeof (Atomics as typeof Atomics & { waitAsync?: unknown }).waitAsync !==
      "function"
  ) {
    throw new Error("Atomics.waitAsync is not supported by this browser")
  }
}

const post = (message: Record<string, unknown>) => {
  self.postMessage(message)
}

const stringifyError = (error: unknown) =>
  error instanceof Error ? error.message : String(error)
