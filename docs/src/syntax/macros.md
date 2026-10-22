# Macros and format strings

## Format strings

The std format macros take inline expressions. `{expr}` and `{expr:spec}` become positional
arguments; plain identifiers stay Rust captures.

```rsk
pub fn report(name: &str, values: &[i32]):
    println!("{name} has {values.len()} values")
    assert!(!values.is_empty(), "{values.len():>4}")
```

This covers `format!`, `print!`, `println!`, `eprint!`, `eprintln!`, `write!`, `writeln!`,
`panic!`, `format_args!`, `todo!`, `unreachable!`, and `unimplemented!`. The arguments of these
macros, `assert!`, `assert_eq!`, `assert_ne!`, their `debug_` forms, `dbg!`, `vec!`, and
`matches!` are Rusk expressions: `vec![Foo.new(); 3]` is `vec![Foo::new(); 3]`.

## `macro_rules!`

Arms are indentation-based; patterns and bodies are Rust tokens, kept as written.

```rsk
macro_rules! make_message:
    ($name:expr) => format!("hello {}", $name)
    ($name:expr, $greeting:expr) =>:
        format!("{} {}", $greeting, $name)
```

A multi-line `pattern =>:` arm keeps every indented body line. Other macro invocations and
the payloads of `#[...]` and `#![...]` attributes are opaque Rust tokens.
