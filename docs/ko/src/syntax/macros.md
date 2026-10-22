# 매크로와 포맷 문자열

## 포맷 문자열

std 포맷 매크로에는 인라인 표현식을 쓸 수 있다. `{expr}`와 `{expr:spec}`은 위치 인자가 된다.
자리표시자 안의 문자열 리터럴은 이스케이프 없이 쓴다.

```rsk
pub fn report(name: &str, values: &[i32], tags: &[&str]):
    println!("{name} has {values.len()} values")
    println!("{tags.join(", ")}")
    assert!(!values.is_empty(), "{values.len():>4}")
```

이 코드는 다음이 된다.

```rust
pub fn report(name: &str, values: &[i32], tags: &[&str]) {
    println!("{} has {} values", name, values.len());
    println!("{}", tags.join(", "));
    assert!(!values.is_empty(), "{:>4}", values.len());
}
```

자리표시자가 모두 단순 식별자, 위치, `{}`뿐인 문자열은 Rust 그대로 둔다. 인라인 표현식이 있는
문자열에 명시적 인자가 없으면 단순 식별자도 인자가 되어, 인자가 문자열의 순서를 따른다.

적용되는 매크로는 `format!`, `print!`, `println!`, `eprint!`, `eprintln!`, `write!`, `writeln!`,
`panic!`, `format_args!`, `todo!`, `unreachable!`, `unimplemented!`이다. 이 매크로들과 `assert!`,
`assert_eq!`, `assert_ne!`, 이들의 `debug_` 형태, `dbg!`, `vec!`, `matches!`의 인자는 Rusk
표현식이다: `vec![Foo.new(); 3]`은 `vec![Foo::new(); 3]`이다.

`log::info!` 같은 다른 매크로는 `#[...]`, `#![...]` 속성의 내용과 마찬가지로 해석하지 않는 Rust
토큰이다.

## `macro_rules!`

arm은 들여쓰기로 구분한다. 패턴과 본문은 Rust 토큰이며 쓴 그대로 유지된다.

```rsk
macro_rules! make_message:
    ($name:expr) => format!("hello {}", $name)
    ($name:expr, $greeting:expr) =>:
        format!("{} {}", $greeting, $name)
```

여러 줄짜리 `pattern =>:` arm은 들여쓴 본문 줄을 모두 유지한다.
