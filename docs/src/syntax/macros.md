# Macros and format strings

## Format strings

The std format macros take inline expressions. `{expr}` and `{expr:spec}` become positional
arguments. A string inside a placeholder uses the literal's escapes, as in
`"{tags.join(\", \")}"`; in a raw literal its quotes are plain, as in `r#"{v.join("-")}"#`.

```rk
pub fn report(name: &str, values: &[i32], tags: &[&str]):
    println!("{name} has {values.len()} values")
    println!("{tags.join(\", \")}")
    assert!(!values.is_empty(), "{values.len():>4}")
```

becomes

```rust
pub fn report(name: &str, values: &[i32], tags: &[&str]) {
    println!("{} has {} values", name, values.len());
    println!("{}", tags.join(", "));
    assert!(!values.is_empty(), "{:>4}", values.len());
}
```

A string whose placeholders are all plain identifiers, positions, or `{}` is left as Rust. When
a string with an inline expression has no explicit arguments, its plain identifiers become
arguments too, so the arguments keep the string's order.

This covers `format!`, `print!`, `println!`, `eprint!`, `eprintln!`, `write!`, `writeln!`,
`panic!`, `format_args!`, `todo!`, `unreachable!`, and `unimplemented!`. The arguments of these
macros, `assert!`, `assert_eq!`, `assert_ne!`, their `debug_` forms, `dbg!`, `vec!`, and
`matches!` are Renk expressions: `vec![Foo.new(); 3]` is `vec![Foo::new(); 3]`.

Other macros, such as `log::info!`, are opaque Rust tokens, as are the payloads of `#[...]` and
`#![...]` attributes.

## `macro_rules!`

Arms are indentation-based. Patterns and bodies are Rust tokens, kept as written.

```rk
macro_rules! make_message:
    ($name:expr) => format!("hello {}", $name)
    ($name:expr, $greeting:expr) =>:
        format!("{} {}", $greeting, $name)
```

A multi-line `pattern =>:` arm keeps every indented body line.
