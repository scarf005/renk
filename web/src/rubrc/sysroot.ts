// Ported from oligamiq/rubrc (MIT OR Apache-2.0).
import {
  Directory,
  File,
  type Inode,
  PreopenDirectory,
} from "@bjorn3/browser_wasi_shim"
import { fetchCompressedStream } from "./brotliStream.ts"
import { parseTar } from "./parseTar.ts"
import { rubrcAssetUrl } from "./artifacts.ts"

export const loadDefaultSysroot = async (): Promise<PreopenDirectory> => {
  const sysrootPart = await loadSysrootPart("wasm32-wasip1")
  const rustlibDir = new Directory([
    ["wasm32-wasip1", new Directory([["lib", sysrootPart]])],
  ])
  return new PreopenDirectory(
    "/sysroot",
    new Map([["lib", new Directory([["rustlib", rustlibDir]])]]),
  )
}

const loadSysrootPart = async (triple: string): Promise<Directory> => {
  const stream = await fetchCompressedStream(rubrcAssetUrl(`${triple}.tar.br`))
  const dir = new Map<string, Inode>()
  await parseTar(stream, (file) => {
    if (file.type !== "file") return
    const parts = file.name.split("/")
    const data = new File(file.data)
    if (parts.length === 1) {
      dir.set(file.name, data)
      return
    }

    const first = parts[0]
    const rest = parts.slice(1).join("/")
    const existing = dir.get(first)
    if (existing instanceof Directory) existing.contents.set(rest, data)
    else dir.set(first, new Directory([[rest, data]]))
  })
  return new Directory(dir)
}
