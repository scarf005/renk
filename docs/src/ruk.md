# Ruk

Ruk (`.rk`) is Rust syntax with optional semicolons. Rusk infers each `;` from the function's
signature: a function returning a value keeps its last expression as the value, and every
other statement ends with `;`.

```rust
pub fn sum_until(values: &[i32], limit: usize) -> i32 {
    let mut index = 0
    let mut total = 0
    while index < limit {
        total += values[index]
        index += 1
    }
    if total < 0 { 0 } else { total }
}
```

A line continues on the next one when that line starts with `.method()`, `?`, `| pattern`, a
binary operator such as `+ ` or `&& `, or `else`. A `;` you write is kept.

```sh
rusk input.rk -o output.rs   # Ruk to Rust
rusk to-ruk input.rs         # Rust or Rusk to Ruk
```

`macro_rules!` bodies stay Rust tokens; Ruk adds no `;` inside them.
