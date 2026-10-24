# 스크립트

`rusk script [--cargo] FILE.rsk [ARGS...]`, 줄여서 `rusk FILE.rsk [ARGS...]`는 단일 파일 스크립트를 트랜스파일하고,
`cargo +nightly -Zscript`로 빌드한 뒤 `ARGS`를 넘겨 실행한다. nightly 툴체인이 필요하다.

```rsk
#!/usr/bin/env -S rusk script
---
[dependencies]
regex = "1"
---

fn main():
    println!("{}", regex.Regex.new("a+").unwrap().is_match("caab"))
```

이 `#!` 줄이 있으면 `chmod +x tool.rsk` 후 `./tool.rsk ARGS`로 스크립트를 실행할 수 있다.

`rusk transpile tool.rsk`는 `rusk script`가 빌드하는 Rust를 출력한다. frontmatter는 유지되고 최상위 줄은
`fn main`으로 감싸진다. 스크립트에는 `--source-map`을 쓸 수 없다. `#!` 줄이나 `---` frontmatter로
시작하는 파일이 스크립트다.

## 의존성

의존성은 cargo-script `---` frontmatter에 적는다. frontmatter는 Cargo 매니페스트다. edition을
지정하지 않은 스크립트는 `package.edition = "2024"`를 받는다.

`rusk add FILE.rsk [CARGO_ADD_ARGS...]`는 frontmatter에 `cargo add`를 실행하고 결과를 다시 써
넣는다. frontmatter가 없으면 `#!` 줄 뒤에 만들고, 스크립트의 나머지는 건드리지 않는다. 상대
`--path` 의존성은 스크립트가 있는 디렉터리를 기준으로 해석한다.

```sh
rusk add tool.rsk regex
rusk add tool.rsk serde --features derive
```

frontmatter에 없는 크레이트 때문에 빌드가 실패하면 오류 메시지가
``add it with `rusk add tool.rsk regex` ``처럼 해당 명령을 알려 준다.

## 최상위 문장

최상위 `fn main`이 없는 스크립트는 최상위 줄들을
`fn main() -> Result<(), Box<dyn std::error::Error>>` 안에서 차례로 실행하므로 `?`를 쓸 수 있다:

```rsk
#!/usr/bin/env -S rusk script

let args = std.env.args().skip(1).collect[Vec[String]]()
let n: i32 = args.first().map_or("2", |s| s.as_str()).parse()?
println!("{n * 2}")
```

`anyhow`, `eyre`, `color-eyre`에 의존하는 스크립트는 대신 그 크레이트의 `Result<()>`를 반환하므로
`bail!`을 쓸 수 있다. 최상위 `return`은 `return Ok(())`로 써야 한다.

## 환경

- `RUSK_SCRIPT`에는 스크립트의 절대 경로가 들어 있다.
- `CARGO_MANIFEST_DIR`와 `CARGO_PKG_NAME`은 `cargo run`과 같은 방식으로 설정된다.
- rusk는 스크립트의 종료 코드로 끝난다.

## 빌드와 캐시

- 생성된 Rust는 `$XDG_CACHE_HOME/rusk/scripts/`(`XDG_CACHE_HOME`이 없으면
  `~/.cache/rusk/scripts/`)에 저장되며, 내용이 바뀔 때만 다시 쓴다.
- 실행할 때는 캐시된 바이너리를 cargo 없이 exec하고, 생성된 Rust가 바뀔 때만 다시 빌드한다.
  `rusk script --cargo FILE.rsk`는 매번 cargo를 거쳐 빌드하므로 의존성과 툴체인 업데이트를
  반영한다.
- 모든 스크립트는 공유 디렉터리 `$XDG_CACHE_HOME/rusk/build/`에서 빌드하므로 각 의존성은 한 번만
  컴파일된다. 동시에 빌드하는 스크립트는 서로를 기다린다. 이 디렉터리를 지우면 다시 빌드해야 하고
  스크립트들의 `Cargo.lock`도 사라지므로, 다음 빌드는 더 새로운 의존성 버전을 고를 수 있다.
- rusk는 스크립트의 패키지 이름을 `<stem>-<hash>`로 정하며, 이 이름은 `CARGO_PKG_NAME`과 바이너리
  이름에 나타난다. `package.name`을 직접 정한 스크립트는 그 이름을 유지하고, 사용자가
  `CARGO_TARGET_DIR`을 지정했더라도 캐시된 Rust 옆의 별도 target 디렉터리에서 빌드한다.

## 진단

- 컴파일러 경고는 스크립트를 컴파일하는 실행에서만 출력된다.
- 진단은 스크립트의 줄과 열을 가리키고 스크립트 소스를 인용한다. 닫는 중괄호처럼 rusk가 만들어 낸
  줄의 span은 그 줄이 닫는 스크립트 줄의 끝을 가리킨다.
- rustc가 제안하는 수정은 Rust 코드이므로 메시지만 보여 준다.
- 패키지 이름을 rusk가 정했으면 cargo의 실패 요약은 ``could not compile `tool.rsk` ``처럼
  스크립트 이름을 쓴다.
