# 블록

## 들여쓰기

`:`로 끝나는 줄은 그 뒤에 오는 더 깊이 들여쓴 줄들을 담는 블록을 연다. `struct`, `enum`, `union`,
`trait`, `impl`, `mod`, `macro_rules!`, `fn`, `if`, `else`, `match`, `loop`, `while`, `for`,
`unsafe`, `async`에 쓸 수 있다. 중괄호, 필드와 arm 뒤의 `,`, 문장 뒤의 `;`는 Rusk가 써 넣는다.

```rsk
pub enum Shape:
    Circle(f64)
    Square(f64)

pub fn area(shape: &Shape) -> f64:
    match shape:
        Shape.Circle(radius) => 3.14 * radius * radius
        Shape.Square(side) => side * side
```

한 블록의 줄들이 같은 칸 수를 쓰기만 하면 공백 몇 칸이든 한 단계가 된다. 탭은 오류다.

`unsafe:`, `async:`, `loop:`는 문장이나 match arm 본문에서만 블록을 연다. `let`의 값이나 함수
인자로 쓸 때는 `let task = async move { ... }`처럼 Rust 중괄호가 필요하다. `extern` 블록도
중괄호가 필요하다.

`:` 블록은 `fn main() {`이나 `fn main() {:` 뒤에 들여쓴 줄을 두는 식으로 Rust의 여는 중괄호를
남겨 둘 수 있다. 닫는 `}`는 필요 없다. Rust `{` 블록은 어디서나 쓸 수 있으므로, 예를 들어
한 줄짜리 본문을 그대로 둘 수 있다:

```rsk
pub fn main():
    let value = { 1 + 2 }
    if value > 2 { println!("big") }
```

## 세미콜론

문장은 줄 끝에서 끝난다. Rusk는 시그니처를 보고 `;`를 추론한다. 값을 반환하는 함수는 마지막
표현식을 값으로 남기고, unit 함수는 마지막 표현식 뒤에 `;`를 붙인다. 추론이 남기는 값을 버리고
싶을 때만 `;`를 직접 쓴다.

`#[cfg(...)]`로 고르는 꼬리 표현식은 각각 값이다. Rust는 문장과 블록에만 속성을 허용하므로,
블록이 아닌 꼬리는 블록이 된다:

```rsk
pub fn separator() -> char:
    #[cfg(unix)]
    '/'
    #[cfg(not(unix))]
    '\\'
```

이 코드는 `#[cfg(unix)]` / `{ '/' }` / `#[cfg(not(unix))]` / `{ '\\' }`가 된다.

## 함수 축약형

`fn name(...) -> T = expr`는 본문이 표현식 하나인 함수다. `= match n:` 뒤에 arm을 들여써 두는
것처럼, 그 표현식이 블록을 열 수도 있다.

```rsk
pub fn twice(value: i32) -> i32 = value * 2

pub fn name(n: i32) -> &'static str = match n:
    0 => "zero"
    _ => "many"
```

## 여러 줄 헤더

`where` 절이나 긴 조건처럼 더 깊이 들여쓴 줄로 이어지는 헤더는 마지막 줄을 `:`로 끝낸다. 본문은
헤더 첫 줄보다 한 단계 깊게 들여쓴다:

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

`Self::Item:` / `    Clone + Default:`처럼 rustfmt가 bound를 더 깊은 줄로 이어 쓰면, 헤더는 그
깊은 줄에서 끝난다. 본문이 없는 항목의 `where` 절은 마지막 bound에서 끝난다.

## 그룹

`(`나 `[`로 끝나는 줄, `{`로 끝나는 구조체 리터럴이나 `use` 트리, 닫히지 않은 문자열, 끝에 붙은
`=`는 그룹이 닫힐 때까지 이어진다. 그룹 안은 Rust 문법이다. 그 뒤 같은 들여쓰기의 `.method()`나
`?` 줄은 표현식을 이어 간다.

```rsk
pub fn total(values: &[i32]) -> i32:
    values
        .iter()
        .map(|value| value * 2)
        .sum()
```
