# Blocks

## Indentation

A line ending with `:` opens a block that holds the more indented lines after it. This works
for `struct`, `enum`, `union`, `trait`, `impl`, `mod`, `macro_rules!`, `fn`, `if`, `else`,
`match`, `loop`, `while`, `for`, `unsafe`, and `async`. Rusk writes the braces, the `,` after
fields and arms, and the `;` after statements.

```rsk
pub enum Shape:
    Circle(f64)
    Square(f64)

pub fn area(shape: &Shape) -> f64:
    match shape:
        Shape.Circle(radius) => 3.14 * radius * radius
        Shape.Square(side) => side * side
```

Any number of spaces is one level, as long as a block's lines agree. Tabs are an error.

`unsafe:`, `async:`, and `loop:` open a block as a statement or a match arm body. As a `let`
value or a call argument they need Rust braces, as in `let task = async move { ... }`. An
`extern` block also needs braces.

A `:` block may keep Rust's opening brace, as in `fn main() {` or `fn main() {:` followed by
indented lines; no closing `}` is needed. A Rust `{` block is accepted anywhere, for example to
keep a one-line body:

```rsk
pub fn main():
    let value = { 1 + 2 }
    if value > 2 { println!("big") }
```

## Semicolons

A statement ends at the end of its line. Rusk infers the `;` from the signature: a function
returning a value keeps its last expression as the value, and a unit function ends it with
`;`. Write `;` yourself only to discard a value that inference keeps.

Tails selected by `#[cfg(...)]` are each a value. One that is not already a block becomes one,
since Rust takes attributes only on statements and blocks:

```rsk
pub fn separator() -> char:
    #[cfg(unix)]
    '/'
    #[cfg(not(unix))]
    '\\'
```

becomes `#[cfg(unix)]` / `{ '/' }` / `#[cfg(not(unix))]` / `{ '\\' }`.

## Function shorthand

`fn name(...) -> T = expr` is a function whose body is one expression. The expression can open a
block, as in `= match n:` followed by indented arms.

```rsk
pub fn twice(value: i32) -> i32 = value * 2

pub fn name(n: i32) -> &'static str = match n:
    0 => "zero"
    _ => "many"
```

## Multi-line headers

A header continued over more indented lines, such as a `where` clause or a long condition,
ends its last line with `:`. The body is indented one level past the header's first line:

```rsk
pub fn show[T](value: T) -> String
where
    T: std.fmt.Debug:
    format!("{value:?}")

pub fn check(a: bool, b: bool) -> bool:
    if a
        && b:
        return true
    false
```

A bound that rustfmt continues on a deeper line, as in `Self::Item:` / `    Clone + Default:`,
ends the header on that deeper line. A `where` clause of an item without a body ends at its
last bound.

## Groups

A line ending with `(` or `[`, a struct literal or `use` tree ending with `{`, an unterminated
string, or a trailing `=` continues until the group closes. The group's contents are Rust
syntax. `.method()` or `?` lines at the same indent after it extend the expression.

```rsk
pub fn total(values: &[i32]) -> i32:
    values
        .iter()
        .map(|value| value * 2)
        .sum()
```
