// Ported from oligamiq/rubrc (MIT OR Apache-2.0).
import init, {
  BrotliDecStream,
  BrotliStreamResultCode,
} from "brotli-dec-wasm/web"
// @ts-ignore Vite resolves package wasm assets with the url query.
import brotliDecWasmUrl from "brotli-dec-wasm/web/bg.wasm?url"

const ready = init(brotliDecWasmUrl)
const OUTPUT_SIZE = 1024 * 1024

export const getBrotliDecompressStream = async (): Promise<
  TransformStream<Uint8Array, Uint8Array>
> => {
  await ready
  const decompressor = new BrotliDecStream()
  return new TransformStream({
    transform(chunk, controller) {
      let code: number
      let inputOffset = 0
      do {
        const input = chunk.slice(inputOffset)
        const result = decompressor.decompress(input, OUTPUT_SIZE)
        controller.enqueue(result.buf)
        code = result.code
        inputOffset += result.input_offset
      } while (code === BrotliStreamResultCode.NeedsMoreOutput)

      if (
        code !== BrotliStreamResultCode.NeedsMoreInput &&
        code !== BrotliStreamResultCode.ResultSuccess
      ) {
        controller.error(`Brotli decompression failed with code ${code}`)
      }
    },
    flush(controller) {
      controller.terminate()
    },
  })
}

export const fetchCompressedStream = async (
  url: string | URL | Request,
): Promise<ReadableStream<Uint8Array>> => {
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`failed to fetch ${url}: ${response.status}`)
  }
  if (!response.body) throw new Error(`missing response body for ${url}`)
  if (response.headers.get("Content-Encoding")?.includes("br")) {
    return response.body
  }
  return response.body.pipeThrough(await getBrotliDecompressStream())
}

export const getWasmModule = async (
  url: string,
): Promise<WebAssembly.Module> => {
  const response = new Response(await fetchCompressedStream(url))
  return await WebAssembly.compile(await response.arrayBuffer())
}
