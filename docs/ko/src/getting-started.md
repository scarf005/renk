# 시작하기

## 설치

체크아웃한 저장소에서 CLI와 언어 서버를 설치한다:

```sh
cargo install --path crates/rusk --bin rusk --bin rusk-lsp
```

[스크립트](scripts.md)를 쓰려면 nightly 툴체인도 필요하다(`rustup toolchain install nightly`).
[언어 서버](editors.md)는 `PATH`에 `rust-analyzer`가 있으면 그것을 쓴다.

## 첫 프로그램

다음을 `hello.rsk`로 저장한다:

```rsk
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

`rusk transpile hello.rsk -o hello.rs && rustc --edition 2024 hello.rs && ./hello`를 실행하면 `3`이
출력된다.

[examples](https://github.com/scarf005/rusk/tree/main/examples) 디렉터리에 예제 프로그램이 더 있다.

## 명령

```sh
rusk transpile input.rsk -o output.rs           # Rusk to Rust
rusk transpile input.rsk --source-map out.json  # also write a JSON source map
cat input.rsk | rusk                            # stdin to stdout
rusk from-rust input.rs -o output.rsk           # Rust to Rusk
rusk fmt input.rsk -o input.rsk                 # format
rusk run                                        # transpile a crate, then cargo run
rusk cargo test                                 # any cargo command
rusk script tool.rsk arg                        # run a single-file script
rusk add tool.rsk regex                         # add a script dependency
rusk-lsp                                        # language server on stdio
```

`-o`가 없으면 stdout으로 출력한다. 입력 없이 stdin이 터미널인 상태로 실행하면 `rusk`는 도움말을
출력한다.
