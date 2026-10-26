# 시작하기

## 설치

체크아웃한 저장소에서 CLI와 언어 서버를 설치한다:

```sh
cargo install --path crates/renk --bin renk --bin renk-lsp
```

[스크립트](scripts.md)를 쓰려면 nightly 툴체인도 필요하다(`rustup toolchain install nightly`).
[언어 서버](editors.md)는 `PATH`에 `rust-analyzer`가 있으면 그것을 쓴다.

## 첫 프로그램

다음을 `hello.rk`로 저장한다:

```rk
pub fn sum_until(values: &[i32], limit: usize) -> i32:
    let mut index = 0
    let mut total = 0
    while index < limit:
        total += values[index]
        index += 1
    if total < 0 then 0 else total

pub fn main():
    println!("{sum_until(&[1, 2, 3], 2)}")
```

`renk transpile hello.rk -o hello.rs && rustc --edition 2024 hello.rs && ./hello`를 실행하면 `3`이
출력된다.

[examples](https://github.com/scarf005/rusk/tree/main/examples) 디렉터리에 예제 프로그램이 더 있다.

## 명령

```sh
renk transpile input.rk -o output.rs           # Renk to Rust
renk transpile input.rk --source-map out.json  # also write a JSON source map
cat input.rk | renk                            # stdin to stdout
renk from-rust input.rs -o output.rk           # Rust to Renk
renk fmt input.rk -o input.rk                 # format
renk run                                        # transpile a crate, then cargo run
renk cargo test                                 # any cargo command
renk script tool.rk arg                        # run a single-file script
renk add tool.rk regex                         # add a script dependency
renk-lsp                                        # language server on stdio
```

`-o`가 없으면 stdout으로 출력한다. 입력 없이 stdin이 터미널인 상태로 실행하면 `renk`는 도움말을
출력한다.
