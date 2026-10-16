use rusk::{rust_to_rusk, transpile};

fn rust(source: &str) -> String {
    transpile(source).unwrap().rust
}

#[test]
fn bracket_calls_and_uppercase_paths_are_generic_application() {
    let output = rust(
        "fn demo(xs: &[fn() -> i32], i: usize, raw: &str):\n    let called = (xs[i])()\n    let parsed = raw.parse[i32]()\n    let size = size_of[u8]()\n    let value = serde_json::from_str[Config](raw)\n    let length = xs[i].len()\n    let row = (TABLE)[i].len()\n    let rows = TABLE[i][0].len()\n    let item = (config::TABLE)[i].len()\n    let made = config::Type[u8].new()\n",
    );
    assert!(output.contains("let called = (xs[i])();"), "{output}");
    assert!(output.contains("let parsed = raw.parse::<i32>();"), "{output}");
    assert!(output.contains("let size = size_of::<u8>();"), "{output}");
    assert!(output.contains("serde_json::from_str::<Config>(raw);"), "{output}");
    assert!(output.contains("let length = xs[i].len();"), "{output}");
    assert!(output.contains("let row = (TABLE)[i].len();"), "{output}");
    assert!(output.contains("let rows = TABLE[i][0].len();"), "{output}");
    assert!(output.contains("let item = (config::TABLE)[i].len();"), "{output}");
    assert!(output.contains("let made = config::Type::<u8>::new();"), "{output}");

    let output = rust(
        "fn make() -> Vec[String] = Vec[String].new()\nfn empty() -> Vec[u8] = Vec[u8]::new()\nfn collect(iter: Iter) = iter.collect[Vec[String]]()\n",
    );
    assert!(output.contains("fn make() -> Vec<String>"), "{output}");
    assert!(output.contains("Vec::<String>::new()"), "{output}");
    assert!(output.contains("Vec::<u8>::new()"), "{output}");
    assert!(output.contains("iter.collect::<Vec<String>>()"), "{output}");
}

#[test]
fn explicit_paths_and_parentheses_bound_dot_inference() {
    let output = rust(
        "fn demo():\n    let length = (TEXT).len()\n    let default_len = Config::DEFAULT.len()\n    let typed = Foo[String]::DEFAULT.len()\n",
    );
    assert!(output.contains("(TEXT).len()"));
    assert!(output.contains("Config::DEFAULT.len()"));
    assert!(output.contains("Foo::<String>::DEFAULT.len()"));
    assert!(!output.contains("DEFAULT::len"));
}

#[test]
fn macro_attribute_and_literal_tokens_are_opaque() {
    let output = rust(
        r##"#[query(table.column, Vec<T>)]
fn demo():
    dsl!(Foo . [T], Vec<T>, std.foo.Bar)
    let text = r#"Foo.bar Vec.[T]"#
    value /* Foo.bar Vec.[T] */ .len()
"##,
    );
    assert!(output.contains("#[query(table.column, Vec<T>)]"));
    assert!(output.contains("dsl!(Foo . [T], Vec<T>, std.foo.Bar)"));
    assert!(output.contains(r##"r#"Foo.bar Vec.[T]"#"##));
    assert!(output.contains("/* Foo.bar Vec.[T] */"));
}

#[test]
fn macro_rules_bodies_use_rust_tokens() {
    let output = rust(
        "macro_rules! make_message:\n    ($name:expr) => format!(\"hello {}\", $name)\n",
    );
    assert!(output.contains("($name:expr) => { format!(\"hello {}\", $name) };"));
}

#[test]
fn type_lowering_reenters_expression_context_for_consts() {
    let output = rust(
        "fn bytes() -> [u8; (TABLE)[N].len()] = todo!()\nfn array() -> Array[u8, { size_of[T]() }] = todo!()\n",
    );
    assert!(output.contains("fn bytes() -> [u8; (TABLE)[N].len()]"), "{output}");
    assert!(output.contains("fn array() -> Array<u8, { size_of::<T>() }>"), "{output}");
    assert!(!output.contains("TABLE<N>"));
    assert!(!output.contains("TABLE::"));
}

#[test]
fn rust_conversion_preserves_paths_and_marks_generic_application() {
    let output = rust_to_rusk(
        "fn id<T>(x: Vec<T>) -> Option<T> {\n    std::mem::drop(x);\n    None\n}\n\nfn parse(value: &str) {\n    let _ = value.parse::<i32>();\n    let _ = std::mem::size_of::<Vec<u8>>();\n    let _ = Vec::<String>::new();\n    let _ = serde_json::from_str::<Config>(value);\n    let _ = [\"1\"].map(str::parse::<Vec<i32>>);\n    let _ = TABLE[0].len();\n    let _ = crate::config::TABLE[0].len();\n}\n",
    )
    .unwrap();
    assert!(output.contains("fn id[T](x: Vec[T]) -> Option[T]:"), "{output}");
    assert!(output.contains("std::mem::drop(x)"), "{output}");
    assert!(output.contains("value.parse[i32]()"), "{output}");
    assert!(output.contains("std::mem::size_of[Vec[u8]]()"), "{output}");
    assert!(output.contains("Vec[String]::new()"), "{output}");
    assert!(output.contains("serde_json::from_str[Config](value)"), "{output}");
    assert!(output.contains("str::parse::<Vec<i32>>"), "{output}");
    assert!(output.contains("(TABLE)[0].len()"), "{output}");
    assert!(output.contains("(crate::config::TABLE)[0].len()"), "{output}");
}

#[test]
fn supports_general_expression_continuations_and_labels() {
    let output = rust(
        "fn add(lhs: i32, rhs: i32) -> i32 = lhs\n    + rhs\n\nfn first(xs: &[i32], i: usize) -> i32 = xs\n    [i]\n\nfn labelled():\n    'outer: loop:\n        break 'outer\n",
    );
    assert!(output.contains("lhs\n        + rhs"));
    assert!(output.contains("xs\n        [i]"));
    assert!(output.contains("'outer: loop {"));
    assert!(output.contains("break 'outer;"));
}

#[test]
fn inline_if_keywords_are_contextual() {
    let output = rust("fn choose(then: bool) -> i32 = if then then 1 else 2\n");
    assert!(output.contains("if then { 1 } else { 2 }"));

    let output = rust("fn choose(value: bool) -> i32 = if value /* then */ then 1 else 2\n");
    assert!(output.contains("if value /* then */ { 1 } else { 2 }"));
}

#[test]
fn let_and_closure_annotations_are_type_positions() {
    let output = rust(
        "fn demo(xs: Vec[i32]):\n    let mut found: Option[i32] = None\n    let (a, b): (Vec[u8], Option[i32]) = (vec![], None)\n    let pick = |x: Vec[i32], i: usize| -> Option[i32] { x.get(i).copied() }\n    let first = xs[0]\n    let mask = xs[0] | xs[1]\n",
    );
    assert!(output.contains("let mut found: Option<i32> = None;"));
    assert!(output.contains("let (a, b): (Vec<u8>, Option<i32>) = (vec![], None);"));
    assert!(output.contains("|x: Vec<i32>, i: usize| -> Option<i32> { x.get(i).copied() }"));
    assert!(output.contains("let first = xs[0];"));
    assert!(output.contains("let mask = xs[0] | xs[1];"));
}

#[test]
fn inline_if_lowers_after_bindings_and_returns() {
    let output = rust(
        "fn pick(c: bool) -> i32:\n    let x = if c then 1 else 2\n    let y =\n        if c then 3 else 4\n    return if x > y then x else y\n",
    );
    assert!(output.contains("let x = if c { 1 } else { 2 };"));
    assert!(output.contains("let y =\n        if c { 3 } else { 4 };"));
    assert!(output.contains("return if x > y { x } else { y };"));
}

#[test]
fn unicode_identifiers_lower_like_ascii_ones() {
    let output = rust(
        "pub struct 상자[T]:\n    값: T\n\nfn 변환[T](값: T) -> T = 값\n\nfn demo() -> 상자[i32]:\n    let 수 = 변환[i32](1)\n    상자 { 값: 수 }\n",
    );
    assert!(output.contains("pub struct 상자<T> {"), "{output}");
    assert!(output.contains("fn 변환<T>(값: T) -> T {"), "{output}");
    assert!(output.contains("fn demo() -> 상자<i32> {"), "{output}");
    assert!(output.contains("let 수 = 변환::<i32>(1);"), "{output}");
}

#[test]
fn method_brackets_before_a_call_are_generic_application() {
    let output = rust(
        "fn demo(iter: I, raw: &str, h: H, xs: X, i: usize, pair: P, 목록: L):\n    let a = iter.collect[Vec[i32]]()\n    let b = raw.parse[i32]()\n    let c = (h.handlers[i])()\n    let d = (xs[i])()\n    let e = h.handlers[i]\n    let f = 목록.변환[Vec[문자열]]()\n    let g = pair.0[i]()\n    let h = iter.map[u8, _](f).sum[u8]()\n",
    );
    assert!(output.contains("let a = iter.collect::<Vec<i32>>();"), "{output}");
    assert!(output.contains("let b = raw.parse::<i32>();"), "{output}");
    assert!(output.contains("let c = (h.handlers[i])();"), "{output}");
    assert!(output.contains("let d = (xs[i])();"), "{output}");
    assert!(output.contains("let e = h.handlers[i];"), "{output}");
    assert!(output.contains("let f = 목록.변환::<Vec<문자열>>();"), "{output}");
    assert!(output.contains("let g = pair.0[i]();"), "{output}");
    assert!(output.contains("let h = iter.map::<u8, _>(f).sum::<u8>();"), "{output}");
}

#[test]
fn rust_conversion_parenthesizes_index_then_call() {
    let output = rust_to_rusk(
        "fn run(h: &H, i: usize, v: Vec<u8>) -> u8 {\n    (h.handlers[i])();\n    h.handlers[i]();\n    handlers[i](i);\n    HANDLERS[i](i);\n    self.table.rows[i][0](i);\n    v.iter().sum::<u8>()\n}\n",
    )
    .unwrap();
    assert!(!output.contains("((h.handlers[i]))()"), "{output}");
    assert_eq!(output.matches("(h.handlers[i])()").count(), 2, "{output}");
    assert!(output.contains("(handlers[i])(i)"), "{output}");
    assert!(output.contains("(HANDLERS[i])(i)"), "{output}");
    assert!(output.contains("self.table.rows[i][0](i)"), "{output}");
    assert!(output.contains("v.iter().sum[u8]()"), "{output}");
}

#[test]
fn uppercase_operand_before_block_opener_is_not_a_struct_literal() {
    let output = rust_to_rusk(
        "fn pick(n: i32) -> i32 {\n    let x = match MODE {\n        1 => 10,\n        _ => 0,\n    };\n    if n > MAX {\n        1\n    } else {\n        x\n    }\n}\n",
    )
    .unwrap();
    assert!(output.contains("    let x = match MODE:\n"), "{output}");
    assert!(output.contains("    if n > MAX:\n"), "{output}");
}

#[test]
fn tab_errors_report_the_physical_line() {
    let error = transpile("fn f() -> i32:\n    g(\n        1,\n\t2,\n    )\n").unwrap_err();
    assert_eq!(error.line, 4);
    let error = rust_to_rusk("fn f() -> i32 {\n    g(\n        1,\n\t2,\n    )\n}\n").unwrap_err();
    assert_eq!(error.line, 4);
}
