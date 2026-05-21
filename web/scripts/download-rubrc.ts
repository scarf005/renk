const RUBRC_VERSION = "v0.2.0"
const RUBRC_BASE_URL = `https://oligamiq.github.io/rust_wasm/${RUBRC_VERSION}`
const OUTPUT_DIR = new URL(`../public/rubrc/${RUBRC_VERSION}/`, import.meta.url)

const artifacts = [
  { name: "rustc_opt.wasm.br", bytes: 19_781_183 },
  { name: "llvm_opt.wasm.br", bytes: 18_519_599 },
  { name: "wasm32-wasip1.tar.br", bytes: 18_940_938 },
] as const

await Deno.mkdir(OUTPUT_DIR, { recursive: true })

for (const artifact of artifacts) {
  const output = new URL(artifact.name, OUTPUT_DIR)
  const existing = await Deno.stat(output).catch(() => null)
  if (existing?.size === artifact.bytes) continue

  const url = `${RUBRC_BASE_URL}/${artifact.name}`
  const response = await fetch(url)
  if (!response.ok || !response.body) {
    throw new Error(`failed to download ${url}: ${response.status}`)
  }

  const temp = new URL(`${artifact.name}.tmp`, OUTPUT_DIR)
  await Deno.writeFile(temp, response.body)
  const downloaded = await Deno.stat(temp)
  if (downloaded.size !== artifact.bytes) {
    await Deno.remove(temp, { recursive: true }).catch(() => {})
    throw new Error(
      `${artifact.name} size mismatch: expected ${artifact.bytes}, got ${downloaded.size}`,
    )
  }
  await Deno.rename(temp, output)
  console.log(`downloaded ${artifact.name}`)
}
