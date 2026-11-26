import { useEffect, useRef } from "preact/hooks"
import { computed, signal, useSignal } from "@preact/signals"
import { useLocation } from "wouter-preact"
import { createEngine } from "./engine.ts"
import { ErrorBanner, InputEditor, Notice, OutputDisplay } from "./Editor.tsx"
import { Header, Layout, Main, Panel } from "./Layout.tsx"
import {
  DEFAULT_EXAMPLE_NAME,
  EXAMPLE_NAMES,
  ExampleName,
  exampleNameFromPath,
  examplePath,
  exampleSource,
} from "./constants.ts"

type EditSide = "source" | "rust"
type OutputMode = "rust" | "syntax-tree"
type RunState = "idle" | "running"

type RunResponse = {
  ok: boolean
  stage: "compile" | "run"
  status: number | null
  stdout: string
  stderr: string
  timedOut: boolean
  outputLimited?: boolean
  stdoutTruncated: boolean
  stderrTruncated: boolean
  compileMs: number
  runMs: number
  totalMs: number
}

const env = (import.meta as ImportMeta & {
  env: { DEV: boolean; VITE_RUN_API?: string }
}).env
const canRunRust = env.DEV || env.VITE_RUN_API === "1"
const BYTES_PER_KIB = 1024
const RUN_SOURCE_LIMIT_KIB = 64
const RUN_SOURCE_LIMIT_BYTES = RUN_SOURCE_LIMIT_KIB * BYTES_PER_KIB
const RUN_SOURCE_LIMIT_LABEL = `${RUN_SOURCE_LIMIT_KIB} KiB`
const AUTO_RUN_STORAGE_KEY = "renk:auto-run"
const AUTO_RUN_DELAY_MS = 1_000

const engine = createEngine(
  new Worker(new URL("./wasm.worker.ts", import.meta.url), { type: "module" }),
)

const editSide = signal<EditSide>("source")
const selectedExample = signal<ExampleName>(DEFAULT_EXAMPLE_NAME)
const sourceCode = signal<string>(exampleSource(DEFAULT_EXAMPLE_NAME))
const rustCode = signal("")
const syntaxTree = signal("")
const transpileError = signal("")
const outputMs = signal(0)
const computing = signal(true)
const wasmStatus = signal<"loading" | "ready" | "failed">("loading")
const wasmError = signal("")
const outputMode = signal<OutputMode>("rust")
const runOutput = signal("")
const runResult = signal<RunResponse | null>(null)
const runState = signal<RunState>("idle")

engine.ready.then(
  () => wasmStatus.value = "ready",
  (error) => {
    wasmError.value = stringifyError(error)
    wasmStatus.value = "failed"
  },
)

let latestRefresh = 0

// Recompute every output from the text the user last edited. A newer edit
// makes the older answers stale, so only the latest refresh publishes.
const refresh = async () => {
  const ticket = ++latestRefresh
  const started = performance.now()
  computing.value = true
  const fromRust = editSide.value === "rust"
  let error = ""
  let renk = sourceCode.value
  let rust = rustCode.value
  let tree = ""
  try {
    if (fromRust) renk = await engine.call("convert_rust_to_renk", rust)
    else rust = await engine.call("transpile_to_rust", renk)
    if (outputMode.value === "syntax-tree") {
      tree = await engine.call("transpile_syntax_tree_json", renk)
    }
  } catch (cause) {
    error = stringifyError(cause)
    if (!fromRust) rust = ""
  }
  if (ticket !== latestRefresh) return
  if (!error) sourceCode.value = renk
  rustCode.value = rust
  syntaxTree.value = tree
  transpileError.value = error
  outputMs.value = performance.now() - started
  computing.value = false
}

void refresh()

const outputText = computed(() =>
  outputMode.value === "rust" ? rustCode.value : syntaxTree.value
)

const stats = computed(() => ({
  sourceLines: countLines(sourceCode.value),
  outputLines: countLines(outputText.value),
}))

export function App() {
  const [location, navigate] = useLocation()
  const copied = useSignal(false)
  const settingsOpen = useSignal(false)
  const autoRun = useSignal(false)
  const sourceRef = useRef<HTMLElement>(null)
  const outputRef = useRef<HTMLElement>(null)
  const isScrollingRef = useRef(false)
  const lastRunSourceRef = useRef("")

  const clearRun = () => {
    lastRunSourceRef.current = ""
    runOutput.value = ""
    runResult.value = null
  }

  const setSourceCode = (value: string) => {
    editSide.value = "source"
    sourceCode.value = value
    clearRun()
    void refresh()
  }

  useEffect(() => {
    try {
      autoRun.value =
        globalThis.localStorage.getItem(AUTO_RUN_STORAGE_KEY) === "1"
    } catch (_) {
      // Storage can be unavailable in hardened/private browser contexts.
    }
  }, [])

  const setAutoRun = (enabled: boolean) => {
    autoRun.value = enabled
    try {
      globalThis.localStorage.setItem(AUTO_RUN_STORAGE_KEY, enabled ? "1" : "0")
    } catch (_) {
      // Keep the in-memory setting even when persistence is unavailable.
    }
  }

  const setRustCode = (value: string) => {
    editSide.value = "rust"
    rustCode.value = value
    clearRun()
    void refresh()
  }

  const showExample = (name: ExampleName) => {
    editSide.value = "source"
    selectedExample.value = name
    sourceCode.value = exampleSource(name)
    clearRun()
    void refresh()
    sourceRef.current?.scrollTo({ top: 0 })
    outputRef.current?.scrollTo({ top: 0 })
  }

  useEffect(() => {
    const path = location === "/" ? globalThis.location.pathname : location
    const renkName = path === "/"
      ? DEFAULT_EXAMPLE_NAME
      : exampleNameFromPath(path)

    if (path === "/") {
      navigate(examplePath(DEFAULT_EXAMPLE_NAME))
      return
    }

    if (renkName) {
      if (
        selectedExample.value === renkName &&
        sourceCode.value === exampleSource(renkName)
      ) return
      showExample(renkName)
      return
    }

    navigate(examplePath(DEFAULT_EXAMPLE_NAME))
  }, [location, navigate])

  const handleScroll = (source: "source" | "output") => {
    if (isScrollingRef.current) return

    const sourceEl = sourceRef.current
    const outputEl = outputRef.current
    if (!sourceEl || !outputEl) return

    isScrollingRef.current = true
    if (source === "source") {
      const ratio = sourceEl.scrollTop /
        (sourceEl.scrollHeight - sourceEl.clientHeight || 1)
      outputEl.scrollTop = ratio *
        (outputEl.scrollHeight - outputEl.clientHeight)
    } else {
      const ratio = outputEl.scrollTop /
        (outputEl.scrollHeight - outputEl.clientHeight || 1)
      sourceEl.scrollTop = ratio *
        (sourceEl.scrollHeight - sourceEl.clientHeight)
    }
    setTimeout(() => isScrollingRef.current = false, 10)
  }

  const loadExample = (name: ExampleName) => {
    showExample(name)
    const path = examplePath(name)
    if (location !== path) navigate(path)
  }

  const copyOutput = async () => {
    await navigator.clipboard.writeText(outputText.value)
    copied.value = true
    setTimeout(() => copied.value = false, 900)
  }

  const formatInput = async () => {
    const source = sourceCode.value
    try {
      const formatted = await engine.call("format_renk", source)
      if (sourceCode.value === source) setSourceCode(formatted)
    } catch (error) {
      transpileError.value = stringifyError(error)
    }
  }

  const runRust = async () => {
    if (
      transpileError.value || computing.value || runState.value === "running"
    ) {
      return
    }
    const rust = rustCode.value
    if (
      new TextEncoder().encode(rust).length > RUN_SOURCE_LIMIT_BYTES
    ) {
      runOutput.value =
        `Rust source exceeds ${RUN_SOURCE_LIMIT_LABEL} run limit`
      return
    }

    lastRunSourceRef.current = rust
    runState.value = "running"
    runResult.value = null
    runOutput.value = "Running..."
    try {
      const response = await fetch("/api/run", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ rust }),
      })
      const result = await response.json() as RunResponse | { error: string }
      if ("error" in result) runOutput.value = result.error
      else {
        runResult.value = result
        runOutput.value = formatRunOutput(result)
      }
    } catch (error) {
      runOutput.value = stringifyError(error)
    } finally {
      runState.value = "idle"
    }
  }

  useEffect(() => {
    if (!canRunRust || !autoRun.value || transpileError.value) return
    if (runState.value === "running" || computing.value) return
    const rust = rustCode.value
    if (rust === lastRunSourceRef.current) return
    const timer = globalThis.setTimeout(() => {
      if (rust !== lastRunSourceRef.current) void runRust()
    }, AUTO_RUN_DELAY_MS)
    return () => globalThis.clearTimeout(timer)
  }, [
    autoRun.value,
    rustCode.value,
    transpileError.value,
    computing.value,
    runState.value,
  ])

  return (
    <Layout>
      <Header>
        <div class="flex-none flex items-center px-4 py-3 border-b-2 lg:border-b-0 lg:border-r-2 border-black min-w-[200px] bg-black text-white">
          <h1 class="text-lg font-bold tracking-tighter uppercase">
            RENK DEMO
          </h1>
        </div>

        <div class="flex-1 flex overflow-x-auto w-full border-b-2 lg:border-b-0 lg:border-r-2 border-black no-scrollbar bg-white">
          {(["rust", "syntax-tree"] as OutputMode[]).map((mode) => (
            <button
              type="button"
              key={mode}
              onClick={() => {
                outputMode.value = mode
                void refresh()
              }}
              class={`flex-none px-4 md:px-6 py-3 font-bold text-xs md:text-sm uppercase transition-none border-r-2 border-black focus:outline-none whitespace-nowrap ${
                outputMode.value === mode
                  ? "bg-black text-white"
                  : "bg-white text-black hover:bg-gray-200"
              }`}
            >
              {mode === "rust" ? "Rust" : "Syntax Tree"}
            </button>
          ))}
        </div>

        <div class="flex-none w-full lg:w-auto flex flex-row border-b-2 lg:border-b-0 border-black bg-white">
          <div class="relative flex-none border-r-2 border-black">
            <button
              type="button"
              onClick={() => settingsOpen.value = !settingsOpen.value}
              class="h-full px-4 py-3 text-xs md:text-sm font-bold uppercase bg-white hover:bg-black hover:text-white"
              aria-expanded={settingsOpen.value}
            >
              Set
            </button>
            {settingsOpen.value && (
              <div class="absolute right-0 top-full z-30 w-52 border-2 border-black bg-white p-2">
                <button
                  type="button"
                  disabled={!canRunRust}
                  onClick={() => setAutoRun(!autoRun.value)}
                  class="w-full flex items-center justify-between gap-3 px-2 py-2 text-xs font-bold uppercase text-left hover:bg-black hover:text-white disabled:opacity-40 disabled:hover:bg-white disabled:hover:text-black"
                >
                  <span>Auto Run</span>
                  <span>{autoRun.value ? "On · 1s" : "Off · 1s"}</span>
                </button>
              </div>
            )}
          </div>
          <div class="flex-1 lg:flex-none relative border-r-2 border-black">
            <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
              <span class="text-[10px] md:text-xs font-bold uppercase mr-2 opacity-50">
                EXPL:
              </span>
            </div>
            <select
              aria-label="Example"
              value={selectedExample.value}
              onChange={(event) =>
                loadExample(event.currentTarget.value as ExampleName)}
              class="appearance-none w-full lg:w-[210px] bg-white pl-14 pr-8 py-3 text-xs md:text-sm font-bold border-none focus:ring-0 cursor-pointer uppercase rounded-none h-full"
            >
              {EXAMPLE_NAMES.map((name) => (
                <option key={name} value={name}>{name}</option>
              ))}
            </select>
            <div class="absolute inset-y-0 right-0 pr-3 flex items-center pointer-events-none">
              <svg
                class="w-3 h-3 md:w-4 md:h-4 fill-current"
                viewBox="0 0 20 20"
              >
                <path d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" />
              </svg>
            </div>
          </div>
        </div>
      </Header>

      <Main>
        <Panel
          title={`RENK (${stats.value.sourceLines} lines)`}
          class="w-full md:w-1/2 h-1/2 md:h-full"
          action={
            <button
              type="button"
              onClick={formatInput}
              class="text-xs uppercase font-bold px-3 py-1 border-2 border-black bg-white hover:bg-black hover:text-white"
            >
              Format
            </button>
          }
        >
          <InputEditor
            editorRef={sourceRef}
            label="Renk source"
            value={sourceCode.value}
            language="renk"
            onChange={setSourceCode}
            onScroll={() => handleScroll("source")}
          />
        </Panel>

        <div class="w-full md:w-1/2 h-1/2 md:h-full flex flex-col min-h-0">
          <Panel
            title={`${
              outputMode.value === "rust" ? "Rust" : "Syntax Tree"
            } (${stats.value.outputLines} lines, ${formatMs(outputMs.value)})`}
            class="w-full h-1/2"
            border="border-b-2 border-black"
            action={
              <div class="flex gap-2">
                {canRunRust && outputMode.value === "rust" && (
                  <button
                    type="button"
                    onClick={runRust}
                    disabled={runState.value === "running" ||
                      computing.value || !!transpileError.value}
                    class="text-xs uppercase font-bold px-3 py-1 border-2 border-black bg-white hover:bg-black hover:text-white disabled:opacity-50 disabled:hover:bg-white disabled:hover:text-black"
                  >
                    {runState.value === "running" ? "Running" : "▶ Run"}
                  </button>
                )}
                <button
                  type="button"
                  onClick={copyOutput}
                  class="text-xs uppercase font-bold px-3 py-1 border-2 border-black bg-white hover:bg-black hover:text-white"
                >
                  {copied.value ? "Copied" : "Copy"}
                </button>
              </div>
            }
          >
            {wasmStatus.value === "ready"
              ? (
                <div class="flex flex-col h-full">
                  {transpileError.value && (
                    <ErrorBanner error={transpileError.value} />
                  )}
                  <div class="flex-1 relative min-h-0">
                    {outputMode.value === "rust"
                      ? (
                        <InputEditor
                          editorRef={outputRef}
                          label="Rust"
                          value={rustCode.value}
                          language="rust"
                          onChange={setRustCode}
                          onScroll={() => handleScroll("output")}
                        />
                      )
                      : (
                        <OutputDisplay
                          outputRef={outputRef}
                          label="Syntax tree"
                          value={syntaxTree.value}
                          language="json"
                          onScroll={() => handleScroll("output")}
                        />
                      )}
                  </div>
                </div>
              )
              : wasmStatus.value === "loading"
              ? <Notice text="Loading..." />
              : (
                <Notice
                  text={`Could not load WebAssembly: ${wasmError.value}`}
                  error
                />
              )}
          </Panel>

          <Panel
            title={`Run Output (${countLines(runOutput.value)} lines${
              runResult.value ? `, ${formatMs(runResult.value.totalMs)}` : ""
            })`}
            class="w-full h-1/2"
            border="border-b-0 border-black"
          >
            <OutputDisplay
              label="Run output"
              value={runOutput.value}
              language="text"
            />
          </Panel>
        </div>
      </Main>
    </Layout>
  )
}

function countLines(value: string): number {
  if (!value) return 0
  return value.replace(/\n$/, "").split("\n").length
}

function formatMs(value: number): string {
  return `${value.toFixed(value < 10 ? 2 : 1)} ms`
}

function formatRunOutput(result: RunResponse): string {
  const truncated = result.stdoutTruncated || result.stderrTruncated
    ? "\n\n[output truncated]"
    : ""
  const output = `${result.stdout}${result.stderr}${truncated}`.trimEnd()
  const status = result.timedOut
    ? `${result.stage} timed out`
    : result.outputLimited
    ? `${result.stage} stopped: output limit reached`
    : `${result.stage} exit ${result.status ?? "unknown"}`
  const timing = `compile ${formatMs(result.compileMs)}, run ${
    formatMs(result.runMs)
  }, total ${formatMs(result.totalMs)}`
  return output
    ? `${output}\n\n[${status}, ${timing}]`
    : `[${status}, ${timing}]`
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) return error.message
  return String(error)
}
