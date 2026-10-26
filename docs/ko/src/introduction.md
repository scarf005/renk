# 소개

[English](../index.html)

Renk는 [Rust](https://rust-lang.org)의 들여쓰기 기반 문법으로,
[Rust's Ugly Syntax](https://matklad.github.io/2023/01/26/rusts-ugly-syntax.html)에서 영감을 받았다.
`renk`는 `.rk` 파일을 Rust로 트랜스파일하고, Rust를 다시 Renk로 변환한다.

```rk
#[derive(Debug, Clone)]
pub struct User:
    pub id: u64
    pub name: String

impl User:
    pub fn new(id: u64, name: String) -> Self = Self{ id, name }
    pub fn display_name(&self) -> &str = &self.name

pub fn main():
    let user = User.new(1, "Ada".to_string())
    println!("{user.display_name()}")
```

생성된 Rust:

```rust
#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
}
impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self{ id, name }
    }
    pub fn display_name(&self) -> &str {
        &self.name
    }
}
pub fn main() {
    let user = User::new(1, "Ada".to_string());
    println!("{}", user.display_name());
}
```

Renk는 문법만 바꾼다. 타입, 빌림, 매크로, 크레이트는 Rust 그대로이고, 트랜스파일러가 다시 쓰지
않는 토큰은 Rust로 그대로 통과한다. Rust의 `{ ... }` 블록, `::` 경로, `::<T>` 터보피시는 언제나
쓸 수 있다.
