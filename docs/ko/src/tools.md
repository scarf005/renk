# 도구

## 포매터

`renk fmt`는 rustfmt가 Rust를 배치하듯 파일의 들여쓰기, 줄바꿈, 공백을 정리한다. 생성한 Rust에
`rustfmt --edition 2024`를 돌리므로 rustfmt(`rustup component add rustfmt`)가 필요하고, 작업
디렉터리의 `rustfmt.toml`을 따른다. 주석, 빈 줄, 문자열 내용은 그대로 둔다. 스크립트의 `#!` 줄과
frontmatter도 그대로다. Rust를 바꾸지 않고는 정리할 수 없는 항목은 줄 끝 공백만 지우고 원래 줄을
유지한다.

```sh
renk fmt src/main.rk -o src/main.rk
renk fmt --check src/*.rk    # 정리되지 않은 파일이 있으면 diff를 출력하고 1로 끝난다
```

## 소스 맵

`renk transpile FILE.rk --source-map MAP.json`은 Renk 문법 노드의 JSON 트리도 쓴다. 각 노드에는
소스의 줄, 열, 텍스트와 그 노드가 된 생성 Rust의 줄 범위가 들어 있다.

## 웹 러너

`podman compose up --build`는 `http://localhost:8080`에서 웹 에디터를 띄운다. 트랜스파일은
브라우저에서 하고, 스니펫의 컴파일과 실행은 이미지의 `rustc`로 한다. 체크아웃한 저장소에서는
`just web`으로 에디터를 띄운다.

- 이미지는 빌드할 때 웹 UI를 빌드하고 `renk` CLI와 작은 `renk-web` HTTP 서버를 포함한다. Deno는
  빌드 단계에서만 쓴다.
- 러너는 `trpl` 0.3.0을 포함하므로 스니펫은 Cargo 매니페스트 없이 `trpl::block_on` 같은 헬퍼를 쓸
  수 있다.
- 서비스는 호스트의 `127.0.0.1:8080`에 바인딩된다. 컨테이너는 `/tmp`를 빼고 읽기 전용이며,
  스니펫은 `/tmp`에서 `RENK_RUN_TIMEOUT_MS` 제한 아래 컴파일되고 실행된다.
- 실행 요청은 기본적으로 Rust 소스 64 KiB, 캡처 출력 256 KiB, 동시 실행 2개, 분당 실행 10회로
  제한된다.
- Cloudflare Tunnel 뒤에 둘 때는 서비스 앞에 Cloudflare Access와 `/api/run` 요청 빈도 제한을 둔다.
