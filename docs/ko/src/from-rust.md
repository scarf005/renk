# Rust에서 변환하기

`rusk from-rust`는 Rust를 Rusk로 변환한다:

```sh
rusk from-rust src/lib.rs -o src/lib.rsk
rusk from-rust src/lib.rs -o src/lib.rsk --aggressive
```

출력은 다시 트랜스파일했을 때 같은 Rust 토큰이 나오는 곳에서만 점 경로, `[T]` 제네릭,
`fn f() -> T = expr`, 추론된 `;`, `if c then a else b`를 쓴다. 각각 자리표시자 하나를 순서대로
채우는 포맷 인자는 인라인으로 바뀐다: `format!("{}", cli.url)`은 `format!("{cli.url}")`이 된다.
`::` 경로는 같은 의미로 읽히는 곳에서 점 경로가 된다: `std::env::args()`는 `std.env.args()`가
되지만 `Config::DEFAULT.len()`은 `::`를 유지한다. 터보피시 호출은 `s.parse[i32]()`처럼
`name[...](`가 되고, CamelCase 경로의 터보피시는 `Vec[String].new()`처럼 `Name[...]`가 된다.
그 밖의 터보피시는 Rust 그대로다. 인덱싱 후 호출하는 receiver와 경로 값 receiver는
`(handlers[i])()`, `(std.cmp.Ordering.Less).reverse()`처럼 괄호로 감싼다.

`--aggressive`는 Rust 토큰은 같지만 줄 배치가 달라지는 변환, 예를 들어 여러 줄짜리 `if`/`else`
값 체인도 허용한다.

주석, 문자열 리터럴, 매크로 본문은 쓴 그대로 유지된다. `match` scrutinee를 담은 `{` 블록처럼
Rusk에 들여쓰기 형태가 없는 것은 Rust 문법으로 남는다.

## 왕복 테스트

저장소는 Rust 파일 코퍼스와 버전을 고정한 실제 크레이트로 `Rust -> Rusk -> Rust` 왕복을
테스트한다. 실제 크레이트마다 왕복한 크레이트가 컴파일되고, 모든 주석을 유지하고, 소스 위치,
지역 변수와 basic block 번호, unit 임시값을 정규화한 뒤 원본과 같은 MIR을 만들어야 한다. MIR은
원본이 그렇게 빌드되는 경우에 한해 기본 feature, 모든 feature, 단위 테스트 각각으로 비교한다.

```sh
cargo test -p rusk --test real_crates -- --ignored --nocapture
```

이 테스트는 crates.io에서 크레이트를 내려받으며, `--emit=mir`에 nightly 툴체인이 필요하다.
