import { resolve } from "node:path"

const repoRoot = resolve("..")
const runnerManifest = resolve("../runner/Cargo.toml")
const runnerDeps = resolve("../runner/target/debug/deps")
const backendPath = resolve("../target/debug/renk-web")
const backendAddress = "127.0.0.1:8081"
const backendUrl = `http://${backendAddress}/`

const runChecked = async (
  command: string,
  args: string[],
  cwd: string,
): Promise<void> => {
  const child = new Deno.Command(command, {
    args,
    cwd,
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  }).spawn()
  const status = await child.status
  if (!status.success) {
    throw new Error(`${command} exited with status ${status.code}`)
  }
}

const waitForBackend = async (): Promise<void> => {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      await fetch(backendUrl)
      return
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 50))
    }
  }
  throw new Error(`renk-web did not start at ${backendUrl}`)
}

await runChecked(
  "cargo",
  ["build", "-p", "renk", "--bin", "renk-web"],
  repoRoot,
)
await runChecked(
  "cargo",
  ["build", "--manifest-path", runnerManifest],
  repoRoot,
)

const backend = new Deno.Command(backendPath, {
  cwd: repoRoot,
  env: {
    ...Deno.env.toObject(),
    RENK_WEB_ADDR: backendAddress,
    RENK_WEB_DIST: resolve("dist"),
    RENK_RUN_DEPS: runnerDeps,
  },
  stdin: "null",
  stdout: "inherit",
  stderr: "inherit",
}).spawn()

try {
  await Promise.race([
    waitForBackend(),
    backend.status.then((status) => {
      throw new Error(`renk-web exited with status ${status.code}`)
    }),
  ])

  const vite = new Deno.Command("deno", {
    args: ["run", "-A", "--node-modules-dir", "npm:vite"],
    cwd: resolve("."),
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  }).spawn()
  const status = await vite.status
  if (!status.success && status.code !== 130) {
    throw new Error(`vite exited with status ${status.code}`)
  }
} finally {
  try {
    backend.kill("SIGTERM")
  } catch {
    // Already exited.
  }
  await backend.status.catch(() => undefined)
}
