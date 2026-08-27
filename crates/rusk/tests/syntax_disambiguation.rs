use rusk::{rust_to_rusk, transpile};

fn rust(source: &str) -> String {
    transpile(source).unwrap().rust
}

#[test]
fn explicit_generic_application_does_not_compete_with_indexing() {
    let output = rust(
        "fn demo(xs: &[fn() -> i32], i: usize, raw: &str):\n    let called = xs[i]()\n    let parsed = raw.parse.[i32]()\n",
    );
    assert!(output.contains("let called = xs[i]();"));
    assert!(output.contains("let parsed = raw.parse::<i32>();"));

    let output = rust(
        "fn make() -> Vec.[String] = Vec.[String].new()\nfn collect(iter: Iter) = iter.collect.[Vec.[String]]()\n",
    );
    assert!(output.contains("fn make() -> Vec<String>"));
    assert!(output.contains("Vec::<String>::new()"));
    assert!(output.contains("iter.collect::<Vec<String>>()"));
}

#[test]
fn explicit_paths_and_parentheses_bound_dot_inference() {
    let output = rust(
        "fn demo():\n    let length = (TEXT).len()\n    let default_len = Config::DEFAULT.len()\n    let typed = Foo.[String]::DEFAULT.len()\n",
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
        "fn bytes() -> [u8; TABLE[N].len()] = todo!()\nfn array() -> Array.[u8, { TABLE[N].len() }] = todo!()\n",
    );
    assert!(output.contains("fn bytes() -> [u8; TABLE[N].len()]"));
    assert!(output.contains("fn array() -> Array<u8, { TABLE[N].len() }>") );
    assert!(!output.contains("TABLE<N>"));
    assert!(!output.contains("TABLE::"));
}

#[test]
fn rust_conversion_preserves_paths_and_marks_generic_application() {
    let output = rust_to_rusk(
        "fn id<T>(x: Vec<T>) -> Option<T> {\n    std::mem::drop(x);\n    None\n}\n\nfn parse(value: &str) {\n    let _ = value.parse::<i32>();\n}\n",
    )
    .unwrap();
    assert!(output.contains("fn id[T](x: Vec.[T]) -> Option.[T]:"));
    assert!(output.contains("std::mem::drop(x)"));
    assert!(output.contains("value.parse.[i32]()"));
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
