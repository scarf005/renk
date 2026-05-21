import { wait_async_polyfill } from "@oligami/browser_wasi_shim-threads"
import run from "./vendor/worker_background_worker.min.js"

wait_async_polyfill()
;(run as () => void)()
