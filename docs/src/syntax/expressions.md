# Expressions

## `if` expressions

`if condition then a else b` is a one-line `if` expression:

```rsk
pub fn clamp(total: i32) -> i32 = if total < 0 then 0 else total
```

An `if` that opens blocks can stand where a value goes, as in `let x = if c:`, `x += if c:`,
or `return if c:`, with `else if d:` and `else:` lines at the `if`'s indent.

```rsk
pub fn sign(value: i32) -> i32:
    let sign = if value > 0:
        1
    else if value < 0:
        -1
    else:
        0
    sign
```

## `let ... else`

```rsk
pub fn first(values: &[i32]) -> i32:
    let [first, ..] = values else:
        return 0
    *first
```

## Closures

A closure head can open a block. Rusk closes the call around it.

```rsk
pub fn sorted(mut values: Vec[i32]) -> Vec[i32]:
    values.sort_by(|a, b|:
        b.cmp(a)
    values
```

## Match arms

A match arm body that opens a block owns the indented lines after it, as in
`Some(x) => match x:` or `Ok(v) => unsafe:`.

```rsk
pub fn describe(value: Option[i32]) -> &'static str:
    match value:
        Some(x) => match x:
            0 => "zero"
            _ => "some"
        None => "none"
```

## Struct literals

Struct literals keep Rust syntax: `Self{ id, name }`.
