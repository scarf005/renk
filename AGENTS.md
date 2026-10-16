# Project Agent Notes

- Do not modify example file spacing, blank lines, or formatting unless the user explicitly requests that exact example formatting change.
- Never restore files from `HEAD` to clean up assistant changes when user-visible edits may be present. Inspect diffs and remove only the assistant-introduced hunks.
- Treat `web/src/wasm/rusk.js`, `web/src/wasm/rusk.internal.js`, `web/src/wasm/rusk.d.ts`, and `web/dist/` as build artifacts; do not commit them unless explicitly requested.
- Runnable example `main` functions should print visible output instead of discarding results with `let _ =`.
- Do not remove existing runtime libraries or framework choices while fixing a bug unless the user explicitly approves that dependency removal.
- Do not preserve backward compatibility unless the user explicitly asks for it
- Rusk syntax handling in `crates/rusk/src/lib.rs` is text-based. When adding or changing a heuristic, add adversarial neighbours (uppercase identifiers, `//` comments or strings ending in delimiters, nested groups, single-line and multi-line forms, macro or literal receivers) to `crates/rusk/tests/rusk_corpus/` or `crates/rusk/tests/rust_corpus/`, and confirm each new case fails before the fix.
