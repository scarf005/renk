# 경로와 제네릭

## 점 경로

`a`가 `self`이거나, `SCREAMING_CASE` 상수이거나, 그 주변에서 `let`, 매개변수(const 제네릭 포함),
패턴으로 묶인 값이면 `a.b`는 필드나 메서드다. 그 밖의 `a`는 타입, 모듈, 크레이트의 이름이므로
`a.b`는 경로다.

| Rusk | Rust |
| --- | --- |
| `std.io.Read` | `std::io::Read` |
| `Foo.new()` | `Foo::new()` |
| `env.args()` | `env::args()` |
| `let user = ...` 뒤의 `user.name()` | `user.name()` |

바인딩은 `let`부터 그 블록 끝까지, `if let`, `while let`, `for` 블록 안에서, 그 match arm 안에서,
그리고 `if let Some(v) = x then v.len() else 0`처럼 자기가 있는 줄 전체에서 값이다.

명시적인 `::` 경로 뒤에서는 CamelCase 마지막 세그먼트가 그다음 `.`를 경로로 만든다.
`regex::Regex.new(s)`는 `regex::Regex::new(s)`이고, `regex.Regex.new(s)`도 같다.
`Config::DEFAULT.len()`은 값 receiver를 유지한다.

`let env = load()`가 있으면 그 뒤의 `env.len()`은 메서드 호출이다. 이 규칙이 놓치는 값 receiver는
`(env).len()`처럼 괄호로 지킨다. 유니코드 식별자는 `목록.변환[Vec[i32]]()`처럼 Rust의 XID 규칙을
따른다.

## 제네릭

제네릭 매개변수와 인자는 `[T]`를 쓴다:

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
| `values.iter().collect[Vec[i32]]()` | `values.iter().collect::<Vec<i32>>()` |
| `size_of[T]()` | `size_of::<T>()` |
| `serde_json::from_str[T](s)` | `serde_json::from_str::<T>(s)` |
| `Vec[String].new()` | `Vec::<String>::new()` |
| `values[index]` | `values[index]` |
| `TABLE[i].len()` | `TABLE[i].len()` |

`name[...](`, 그리고 `::`나 `.name` 앞의 CamelCase `Name[...]`는 제네릭 적용이다. 그 밖의 모든
`[...]`는 인덱싱이며, 내용을 보고 추론하지 않는다. 인덱싱한 뒤 호출하려면 Rust처럼
`(handlers[i])()`로 괄호를 친다.

그 밖의 제네릭 적용은 `.map(str::parse::<i32>)`처럼 Rust 문법을 쓴다.

문장 안의 타입 자리, 즉 `let`, `static`, `const`의 타입 표기와 클로저 매개변수·반환 타입은
`let found: Option[i32] = None`처럼 `[T]`를 쓴다.

## 탈출구

- `::`와 `::<...>`는 경로나 제네릭 적용을 강제한다.
- 괄호는 값 receiver를 강제하고, Rust 출력에서는 사라진다: `(Unit).method()` ->
  `Unit.method()`, `(Ordering::Less).reverse()` -> `Ordering::Less.reverse()`.
