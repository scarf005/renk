# Paths and generics

## Dotted paths

`a.b` is a field or method when `a` is `self`, a `SCREAMING_CASE` constant, or a value bound
around it by a `let`, a parameter, or a pattern. Any other `a` names a type, module, or crate,
so `a.b` is a path.

| Rusk | Rust |
| --- | --- |
| `std.io.Read` | `std::io::Read` |
| `Foo.new()` | `Foo::new()` |
| `env.args()` | `env::args()` |
| `user.name()` after `let user = ...` | `user.name()` |

A binding is a value from its `let` to the end of its block, in its `if let`, `while let`, or
`for` block, in its match arm, and on its whole line.

After an explicit `::` path, a CamelCase last segment makes the following `.` a path:
`regex::Regex.new(s)` is `regex::Regex::new(s)`.

## Generics

Generic parameters and arguments use `[T]`:

```rsk
pub struct Boxed[T]:
    pub value: T

impl[T] Boxed[T]:
    pub fn new(value: T) -> Self = Self{ value }

pub fn parse(raw: &str) -> Result[Boxed[i32], std.num.ParseIntError]:
    let value = raw.parse[i32]()?
    Ok(Boxed.new(value))
```

| Rusk | Rust |
| --- | --- |
| `iter.collect[Vec[i32]]()` | `iter.collect::<Vec<i32>>()` |
| `size_of[T]()` | `size_of::<T>()` |
| `Vec[String].new()` | `Vec::<String>::new()` |
| `values[index]` | `values[index]` |
| `TABLE[i].len()` | `TABLE[i].len()` |

`name[...](` and a CamelCase `Name[...]` before `::` or `.name` are generic applications. Every
other `[...]` is indexing; it is never inferred from its contents. To index and then call,
parenthesize as in Rust: `(handlers[i])()`.

Type positions inside statements keep `[T]`: `let found: Option[i32] = None`.

## Escape hatches

- `::` and `::<...>` force a path or generic application.
- Parentheses force a value receiver; the Rust output drops them: `(Unit).method()` ->
  `Unit.method()`, `(Ordering::Less).reverse()` -> `Ordering::Less.reverse()`.
