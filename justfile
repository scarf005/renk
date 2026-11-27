default:
    @just --list

# Run the whole renk test suite, the same command as CI.
test:
    cargo test -p renk --no-fail-fast

# Check the Rust formatting, the same command as CI.
fmt-check:
    cargo fmt --all --check

# Lint the workspace, the same command as CI.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Run the tree-sitter grammar corpus tests.
grammar-test:
    cd editors/zed/tree-sitter-renk && tree-sitter test

# Type-check and lint the web demo from the repository root.
web-check:
    cd web && deno task check

# Run the web demo tests from the repository root.
web-test:
    cd web && deno task test

# Round-trip pinned crates.io releases, the same command as the weekly CI run.
test-real-crates:
    cargo test -p renk --test real_crates -- --ignored

# Run the renk-web tests that need unprivileged user namespaces.
test-web-namespaces:
    cargo test -p renk --bin renk-web -- --ignored

# Start the Renk web demo from the repository root.
web:
    cd web && deno task dev

# Alias for `just web`.
dev: web

# Build the web demo from the repository root.
web-build:
    cd web && deno task build

# Preview the production web build from the repository root.
web-preview:
    cd web && deno task preview

# Rebuild only the WebAssembly bindings.
wasm:
    cd web && deno task wasmbuild
