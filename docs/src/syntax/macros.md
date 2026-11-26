# Macros and format strings

## Format strings

The std format macros take inline expressions. `{expr}` and `{expr:spec}` become positional
arguments. A string inside a placeholder can keep its quotes, as in `"{tags.join(", ")}"`, or
use the literal's escapes, as in `"{tags.join(\", \")}"`.

```rk
pub fn report(name: &str, values: &[i32], tags: &[&str]):
    println!("{name} has {values.len()} values")
    println!("{tags.join(\", \")}")
    println!("{tags.join(", ")} end")
    assert!(!values.is_empty(), "{values.len():>4}")
```

becomes

```rust
pub fn report(name: &str, values: &[i32], tags: &[&str]) {
    println!("{} has {} values", name, values.len());
    println!("{}", tags.join(", "));
    println!("{} end", tags.join(", "));
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

## `view!`

In a Leptos `view!` body, each `{` block that spans several lines holds Renk statements, so they
need no `;`. These blocks are children and attribute values (`<p>{`, `class={`), closure and
`match` arm bodies, and bodies of `if`, `else`, `for`, `while`, `loop`, `async`, and `unsafe`.
All other text stays Rust tokens: tags, one-line blocks such as `{count}`, struct literals such
as `Point {`, `match` arm lists, and element bodies of other `view!` macros, such as Sycamore's
`div(class="x") {`.

A tag name can carry generics, as in `<Show<u8> ...>`, and a `>` inside an attribute value, such
as `when=move || n > 1`, does not end the tag.

```rk,ignore
pub fn counter(n: u8):
    view! {
        <Show<u8> when=move || n > 1 on:click=move |_| {
            go()
            log::info!("clicked")
        }>
            {n}
        </Show>
    }
```

becomes

```rust
pub fn counter(n: u8) {
    view! {
        <Show<u8> when=move || n > 1 on:click=move |_| {
            go();
            log::info!("clicked")
        }>
            {n}
        </Show>
    }
}
```

```rk,ignore
pub fn counter():
    let (count, set_count) = signal(0)
    view! {
        <button on:click=move |_| {
            set_count.update(|n| *n += 1)
            log::info!("clicked")
        }>
            {count}
        </button>
    }
```

becomes

```rust
pub fn counter() {
    let (count, set_count) = signal(0);
    view! {
        <button on:click=move |_| {
            set_count.update(|n| *n += 1);
            log::info!("clicked")
        }>
            {count}
        </button>
    }
}
```

## `macro_rules!`

Arms are indentation-based. Patterns and bodies are Rust tokens, kept as written.

```rk
macro_rules! make_message:
    ($name:expr) => format!("hello {}", $name)
    ($name:expr, $greeting:expr) =>:
        format!("{} {}", $greeting, $name)
```

A multi-line `pattern =>:` arm keeps every indented body line.
