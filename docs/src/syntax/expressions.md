# Expressions

## `if` expressions

`if condition then a else b` is a one-line `if` expression:

```rk
pub fn clamp(total: i32) -> i32 = if total < 0 then 0 else total
```

An `if` that opens blocks can stand where a value goes, as in `let x = if c:`, `x += if c:`,
`return if c:`, `max(if c:`, or `.map(|x| if c:`, with `else if d:` and `else:` lines at the
`if`'s indent. So can a `match`, as in `let x = match y:`, `.filter(|v| match v:`, or
`[match x:`, and `else:` branches after `let x = if c {` or `f(if c {`.

```rk
pub fn sign(value: i32) -> i32:
    let sign = if value > 0:
        1
    else if value < 0:
        -1
    else:
        0
    sign
```

The condition can be `if let`, and `then` can follow an operator:

```rk
pub fn first(o: Option[i32], n: i32) -> i32 = n + if let Some(v) = o then v else 0
```

becomes

```rust
pub fn first(o: Option<i32>, n: i32) -> i32 {
    n + if let Some(v) = o { v } else { 0 }
}
```

With Rust's `{` kept after the condition, `else:` still follows at the `if`'s indent:

```rk
pub fn pick(c: bool) -> i32:
    let x = if c {
        1
    else:
        2
    x
```

becomes

```rust
pub fn pick(c: bool) -> i32 {
    let x = if c {
        1
    } else {
        2
    };
    x
}
```

A `}` line at the head's indent continues the value with an operator:

```rk
pub fn bucket(value: i32) -> i32:
    let base = match value:
        0 => 1
        _ => 2
    } * 10
    base
```

## `let ... else`

```rk
pub fn first(values: &[i32]) -> i32:
    let [first, ..] = values else:
        return 0
    *first
```

## Closures

A closure head can open a block, as in `let f = |n: u8| -> String:`, `move ||:`, or
`xs.sort_by(|a, b|:`. A block whose head leaves `(` or `[` open ends with a line
at the head's indent that closes them, and the rest of that line continues the expression:

```rk
pub fn sorted(mut values: Vec[i32]) -> Vec[i32]:
    values.sort_by(|a, b|:
        b.cmp(a)
    )
    values

pub fn total(values: &[i32]) -> i32:
    values.iter().map(|value|:
        value * 2
    ).sum()
```

A `{` head closes the same way with `})`. A closing line such as `})?` or `}).method()` keeps
its suffix after a `{` block, a trailing `match` head, or the last `else` of a chain whose head
ends with `{`. A last `else` on one line ends the chain with what closes the group, as in
`else { floor })`:

```rk
pub fn raised(values: &[i32], floor: i32) -> i32:
    values.iter().map(|value| if *value > floor:
        *value
    else { floor }).sum()
```

A closing line can open the next block argument:

```rk
pub fn size(value: Option[usize]) -> usize:
    value.map_or_else(||:
        0
    , |n|:
        n * 2
    )
```

## Match arms

A match arm body that opens a block owns the indented lines after it, as in
`Some(x) => match x:`, `Ok(v) => unsafe:`, or `None => Ok(unsafe:`.

```rk
pub fn describe(value: Option[i32]) -> &'static str:
    match value:
        Some(x) => match x:
            0 => "zero"
            _ => "some"
        None => "none"
```

## Struct literals

Struct literals keep Rust syntax: `Self{ id, name }`. A line that ends with `:` can also open
one. Its lines are `name = value` fields, or `0 = value` for a tuple struct, and `Pair[i32]:`
is `Pair::<i32> {`. This form is only for a value on a line of its own, such as a function's
last value. `let p = Point:` and `return Point:` are not accepted: write `let p = Point{ x: 1, y: 2 }`.

```rk
pub struct Point:
    pub x: i32
    pub y: i32

pub struct W(pub i32)

pub fn point() -> Point:
    Point:
        x = 1
        y = 2

pub fn wrapped() -> W:
    W:
        0 = 1
```

becomes

```rust
pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub struct W(pub i32);

pub fn point() -> Point {
    Point {
        x: 1,
        y: 2,
    }
}

pub fn wrapped() -> W {
    W {
        0: 1,
    }
}
```

