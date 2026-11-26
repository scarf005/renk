# 에디터 설정

`renk-lsp`는 `.rk` 파일용 stdio Language Server Protocol 서버다.
`cargo install renk --bin renk-lsp`로 설치한다. 제공하는 기능은 다음과 같다:

- Renk 문법 오류 진단.
- 문서 심볼.
- hover, 정의로 이동, 자동 완성. `rust-analyzer`가 없으면 열린 파일의 선언, 키워드, 내장
  타입으로 답한다.
- `PATH`에 `rust-analyzer`가 있으면, 생성된 Rust에 대한 `rust-analyzer`의 진단(저장할 때 도는
  `cargo check`의 타입 오류 포함), 타입을 보여 주는 hover, 다른 파일과 크레이트로 가는 정의로
  이동, 메서드와 스코프 안 이름의 자동 완성. 참조 찾기, 이름
  바꾸기, 코드 액션, 인레이 힌트는 `rust-analyzer`가 있어야 한다.

위치는 `.rk` 파일 기준으로 되돌려 매핑한다. 생성된 Rust와 다른 Renk 텍스트를 바꾸게 되는 이름
바꾸기는 실패하고, 편집을 `.rk` 줄로 옮길 수 없는 코드 액션은 빠진다.
`a.b` 점에서 난 이름 해석 오류에는 `renk check`와 같은 힌트가 붙는다([한계](limits.md) 참고).

`#!` 줄이나 frontmatter가 있거나 Cargo 패키지 밖에 있는 `.rk` 파일은 스크립트로 보고
`renk script`와 같은 방식으로 트랜스파일한다. 스크립트는 `$XDG_CACHE_HOME/renk/lsp/`에 생성된
cargo-script 파일을 대상으로 자기 전용 `rust-analyzer`를 띄우므로 frontmatter 의존성이 해석된다.

## Zed

[`editors/zed`](https://github.com/scarf005/renk/tree/main/editors/zed)의 확장은 Renk 언어를
등록하고 `renk-lsp`를 실행하며 Tree-sitter로 하이라이트한다. Zed 확장 페이지의
**Install Dev Extension**에서 `editors/zed`를 선택해 설치한다.

## Neovim

Neovim 0.11 이상:

```lua
vim.filetype.add({ extension = { rk = "renk" } })
vim.lsp.config("renk", {
  cmd = { "renk-lsp" },
  filetypes = { "renk" },
  root_markers = { "Cargo.toml", ".git" },
})
vim.lsp.enable("renk")
```

## 다른 에디터

`.rk`로 끝나는 파일에 대해 인자 없는 `renk-lsp` 명령을 stdio 언어 서버로 설정한다.
