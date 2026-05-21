import { Fd } from "@bjorn3/browser_wasi_shim"

export class CapturingFd extends Fd {
  private chunks: string[] = []

  text() {
    return this.chunks.join("")
  }

  reset() {
    this.chunks = []
  }

  override fd_write(data: Uint8Array) {
    this.chunks.push(new TextDecoder().decode(data))
    return { ret: 0, nwritten: data.byteLength }
  }

  override fd_seek() {
    return { ret: 8, offset: 0n }
  }

  override fd_filestat_get() {
    return { ret: 8, filestat: null }
  }
}
