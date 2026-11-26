/// <reference lib="deno.ns" />
import { assertEquals, assertRejects } from "jsr:@std/assert@1.0.13"
import { createEngine, EngineMessage, EngineRequest } from "./engine.ts"

const fakeWorker = () => {
  const requests: EngineRequest[] = []
  let listener: (event: { data: EngineMessage }) => void = () => {}
  return {
    requests,
    reply: (data: EngineMessage) => listener({ data }),
    postMessage: (request: EngineRequest) => requests.push(request),
    addEventListener: (
      _type: "message",
      next: (event: { data: EngineMessage }) => void,
    ) => listener = next,
  }
}

Deno.test("each call posts one request and resolves with its own reply", async () => {
  const worker = fakeWorker()
  const engine = createEngine(worker)
  const first = engine.call("transpile_to_rust", "a")
  const second = engine.call("format_renk", "b")
  assertEquals(worker.requests.map(({ fn }) => fn), [
    "transpile_to_rust",
    "format_renk",
  ])
  worker.reply({ id: 1, value: "B" })
  worker.reply({ id: 0, value: "A" })
  assertEquals([await first, await second], ["A", "B"])
})

Deno.test("a worker error rejects only that call", async () => {
  const worker = fakeWorker()
  const engine = createEngine(worker)
  const bad = engine.call("transpile_to_rust", "x")
  const good = engine.call("transpile_to_rust", "y")
  worker.reply({ id: 0, error: "line 3: unexpected token" })
  worker.reply({ id: 1, value: "ok" })
  await assertRejects(() => bad, Error, "line 3")
  assertEquals(await good, "ok")
})

Deno.test("ready follows the worker and a failed load rejects it", async () => {
  const loaded = fakeWorker()
  const ok = createEngine(loaded)
  loaded.reply({ ready: true })
  await ok.ready

  const broken = fakeWorker()
  const engine = createEngine(broken)
  broken.reply({ failed: "no wasm" })
  await assertRejects(() => engine.ready, Error, "no wasm")
})

Deno.test("only the worker imports the wasm bindings", async () => {
  const importers: string[] = []
  for await (const entry of Deno.readDir(new URL(".", import.meta.url))) {
    if (!/\.tsx?$/.test(entry.name) || entry.name.endsWith("_test.ts")) continue
    const text = await Deno.readTextFile(new URL(entry.name, import.meta.url))
    if (/from\s+"\.\/wasm\//.test(text)) importers.push(entry.name)
  }
  assertEquals(importers, ["wasm.worker.ts"])
})
