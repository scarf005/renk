/// <reference lib="deno.ns" />
import { assertEquals } from "jsr:@std/assert@1.0.13"
import { convert_rust_to_rusk, transpile_to_rust } from "./wasm/rusk.js"

Deno.test("deep nesting converts in WebAssembly", () => {
  const rust = `fn f() -> u8 {\n    ${"(".repeat(2500)}1${
    ")".repeat(2500)
  }\n}\n`
  assertEquals(transpile_to_rust(convert_rust_to_rusk(rust)), rust)
})
