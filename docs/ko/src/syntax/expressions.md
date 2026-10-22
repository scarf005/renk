# 표현식

## `if` 표현식

`if condition then a else b`는 한 줄짜리 `if` 표현식이다:

```rsk
pub fn clamp(total: i32) -> i32 = if total < 0 then 0 else total
```

블록을 여는 `if`는 `let x = if c:`, `x += if c:`, `return if c:`, `max(if c:`, `.map(|x| if c:`
처럼 값 자리에 올 수 있고, `else if d:`와 `else:` 줄은 `if`와 같은 들여쓰기에 둔다. `match`도
`let x = match y:`, `.filter(|v| match v:`, `[match x:`처럼 쓸 수 있고, `f(if c {` 뒤의 `else:`
가지도 마찬가지다.

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

머리와 같은 들여쓰기의 `}` 줄은 연산자로 값을 이어 간다:

```rsk
pub fn bucket(value: i32) -> i32:
    let base = match value:
        0 => 1
        _ => 2
    } * 10
    base
```

## `let ... else`

```rsk
pub fn first(values: &[i32]) -> i32:
    let [first, ..] = values else:
        return 0
    *first
```

## 클로저

클로저 머리는 `let f = |n: u8| -> String:`, `move ||:`, `xs.sort_by(|a, b|:`처럼 블록을 열 수
있다. 머리가 `(`나 `[`를 열어 둔 블록은 머리와 같은 들여쓰기에서 그것을 닫는 줄로 끝나고, 그 줄의
나머지는 표현식을 이어 간다:

```rsk
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

`{` 머리도 같은 방식으로 `})`로 닫는다. `{` 블록, 끝에 붙은 `match` 머리, 머리가 `{`로 끝나는
체인의 마지막 `else` 뒤에서는 `})?`나 `}).method()` 같은 닫는 줄이 뒤의 접미사를 유지한다. 한 줄로
쓴 마지막 `else`는 `else { floor })`처럼 그룹을 닫는 것으로 체인을 끝낸다:

```rsk
pub fn raised(values: &[i32], floor: i32) -> i32:
    values.iter().map(|value| if *value > floor:
        *value
    else { floor }).sum()
```

닫는 줄에서 다음 블록 인자를 열 수 있다:

```rsk
pub fn size(value: Option[usize]) -> usize:
    value.map_or_else(||:
        0
    , |n|:
        n * 2
    )
```

## Match arm

블록을 여는 match arm 본문은 `Some(x) => match x:`, `Ok(v) => unsafe:`, `None => Ok(unsafe:`처럼
그 뒤에 들여쓴 줄들을 차지한다.

```rsk
pub fn describe(value: Option[i32]) -> &'static str:
    match value:
        Some(x) => match x:
            0 => "zero"
            _ => "some"
        None => "none"
```

## 구조체 리터럴

구조체 리터럴은 Rust 문법을 그대로 쓴다: `Self{ id, name }`.
