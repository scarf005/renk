# 매크로와 포맷 문자열

## 포맷 문자열

std 포맷 매크로에는 인라인 표현식을 쓸 수 있다. `{expr}`와 `{expr:spec}`은 위치 인자가 된다.
자리표시자 안의 문자열은 `"{tags.join(", ")}"`처럼 따옴표를 그대로 쓰거나
`"{tags.join(\", \")}"`처럼 리터럴의 이스케이프로 쓴다. raw 리터럴에서는 `\"`가 이스케이프가
아니므로 `r#"{v.join("-")}"#`처럼 따옴표를 그대로 쓴다.

```rk
pub fn report(name: &str, values: &[i32], tags: &[&str]):
    println!("{name} has {values.len()} values")
    println!("{tags.join(\", \")}")
    println!("{tags.join(", ")} end")
    assert!(!values.is_empty(), "{values.len():>4}")
```

이 코드는 다음이 된다.

```rust
pub fn report(name: &str, values: &[i32], tags: &[&str]) {
    println!("{} has {} values", name, values.len());
    println!("{}", tags.join(", "));
    println!("{} end", tags.join(", "));
    assert!(!values.is_empty(), "{:>4}", values.len());
}
```

자리표시자가 모두 단순 식별자, 위치, `{}`뿐인 문자열은 Rust 그대로 둔다. 인라인 표현식이 있는
문자열에 명시적 인자가 없으면 단순 식별자도 인자가 되어, 인자가 문자열의 순서를 따른다.

적용되는 매크로는 `format!`, `print!`, `println!`, `eprint!`, `eprintln!`, `write!`, `writeln!`,
`panic!`, `format_args!`, `todo!`, `unreachable!`, `unimplemented!`이다. 이 매크로들과 `assert!`,
`assert_eq!`, `assert_ne!`, 이들의 `debug_` 형태, `dbg!`, `vec!`, `matches!`의 인자는 Renk
표현식이다: `vec![Foo.new(); 3]`은 `vec![Foo::new(); 3]`이다.

`log::info!` 같은 다른 매크로는 `#[...]`, `#![...]` 속성의 내용과 마찬가지로 해석하지 않는 Rust
토큰이다.

## `view!`

Leptos `view!` 본문에서 여러 줄에 걸친 `{` 블록은 Renk 문장을 담으므로 `;`가 필요 없다.
자식과 속성 값(`<p>{`, `class={`), 클로저와 `match` 팔의 본문, `if`, `else`, `for`, `while`,
`loop`, `async`, `unsafe`의 본문이 이런 블록이다. 나머지는 Rust 토큰으로 남는다. 태그,
`{count}` 같은 한 줄 블록, `Point {` 같은 구조체 리터럴, `match` 팔 목록, Sycamore의
`div(class="x") {` 같은 다른 `view!` 매크로의 요소 본문이 그렇다.

태그 이름에는 `<Show<u8> ...>`처럼 제네릭을 쓸 수 있고, `when=move || n > 1`처럼 속성 값 안에
있는 `>`는 태그를 끝내지 않는다.

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

이 코드는 다음이 된다.

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

는 다음이 된다.

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

arm은 들여쓰기로 구분한다. 패턴과 본문은 Rust 토큰이며 쓴 그대로 유지된다.

```rk
macro_rules! make_message:
    ($name:expr) => format!("hello {}", $name)
    ($name:expr, $greeting:expr) =>:
        format!("{} {}", $greeting, $name)
```

여러 줄짜리 `pattern =>:` arm은 들여쓴 본문 줄을 모두 유지한다.
