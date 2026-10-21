fn main() {
    // WebAssembly keeps a 1 MiB stack by default, which the Rust parser's recursion per nesting
    // level exhausts; native conversions run on a large thread instead.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg-cdylib=-zstack-size=8388608");
    }
}
