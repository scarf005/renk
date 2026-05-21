// Ported from nanotar via oligamiq/rubrc (MIT).
export type TarFileItem = {
  name: string
  type: "file" | "directory" | number
  size: number
  data: Uint8Array
}

export async function parseTar(
  stream: ReadableStream<Uint8Array>,
  callback: (file: TarFileItem) => void,
) {
  const reader = stream.getReader()
  let buffer = new Uint8Array(0)
  let done = false

  const readMore = async () => {
    const chunk = await reader.read()
    if (chunk.value) {
      const next = new Uint8Array(buffer.length + chunk.value.length)
      next.set(buffer)
      next.set(chunk.value, buffer.length)
      buffer = next
    }
    done = chunk.done
  }

  while (true) {
    while (buffer.length < 512 && !done) await readMore()

    const name = readString(buffer, 0, 100)
    if (name.length === 0) break

    const size = readNumber(buffer, 124, 12)
    const rawType = readNumber(buffer, 156, 1)
    const type = rawType === 0 ? "file" : rawType === 5 ? "directory" : rawType

    while (buffer.length < 512 + size && !done) await readMore()
    callback({ name, type, size, data: buffer.slice(512, 512 + size) })

    const adjustedSize = 512 + Math.ceil(size / 512) * 512
    while (buffer.length < adjustedSize && !done) await readMore()
    if (done && buffer.length < adjustedSize) break
    buffer = buffer.slice(adjustedSize)
  }
}

function readString(buffer: Uint8Array, offset: number, size: number) {
  const view = buffer.slice(offset, offset + size)
  const end = view.indexOf(0)
  return new TextDecoder().decode(view.slice(0, end === -1 ? view.length : end))
}

function readNumber(buffer: Uint8Array, offset: number, size: number) {
  const text = String.fromCodePoint(...buffer.slice(offset, offset + size))
    .replace(/\0.*$/, "")
    .trim()
  return text ? Number.parseInt(text, 8) : 0
}
