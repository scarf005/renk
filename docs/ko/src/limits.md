# 한계

Renk는 문법 프런트엔드다. 타입이 아니라 텍스트를 읽으므로 다음과 같은 한계가 있다:

- `a.b`는 타입이 아니라 이름 바인딩으로 판단한다. 잘못 읽힌 이름은 대개 컴파일에 실패한다.
  경로를 강제하려면 `foo::new()`를, 값 receiver를 강제하려면 `(Unit).method()`를 쓴다. 이런
  점에서 이름을 찾지 못하면 ``renk read `String.nope` as the path `String::nope`; write
  `(String).nope` for a value``처럼 다른 표기를 알려 주는 help 줄이 붙는다.
- 표현식 뒤의 `[...]`는 `name[...](`이거나 `::`나 `.name` 앞의 CamelCase `Name[...]`가 아니면
  인덱싱이다. 그 밖의 제네릭 적용은 Rust의 `::<...>`를 쓴다.
- 매크로와 속성의 토큰 트리는 해석하지 않는 Rust 토큰이다. [매크로](syntax/macros.md)에 나열한
  std 포맷 매크로와 표현식 매크로만 다시 쓴다.
- proc-macro DSL은 `view!`의 여러 줄 `{` 블록을 빼고는 파싱하지 않는다.

## 변환의 빈틈

`renk from-rust`는 컴파일된 의미는 유지하지만 텍스트가 항상 같지는 않다:

- 블록을 끝내는 unit 표현식이나 발산 표현식에 `;`가 붙을 수 있다. `panic!()` 뒤나, arm이
  `None => v.clear()` 같은 표현식인 `match` 문이 그렇다.
- `macro_rules! m {() => {`처럼 항목과 같은 줄에서 시작하는 `macro_rules!` arm은 자기 줄로 옮겨지고
  뒤에 `;`가 붙는다.
