use rusk::{FormatOptions, format_source, rust_to_rusk, transpile};

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
fn camel_case_segments_of_explicit_paths_lower_dot_calls() {
    let output = rust(
        "fn demo():\n    let command = std::process::Command.new(\"true\")\n    let re = regex::Regex.new(\"a\").unwrap()\n    let made = crate::config::Type.default().len()\n    let local = Self::Builder.new()\n    let generic = Foo[u8]::Inner.new()\n    let default_len = Config::DEFAULT.len()\n    let bits = i32::MAX.count_ones()\n    let order = (std::cmp::Ordering::Less).reverse()\n    let path = std::cmp::Ordering::Less::reverse(order)\n    let lower = module::value.len()\n    let text = \"regex::Regex.new(\" // regex::Regex.new(\n",
    );
    for expected in [
        "std::process::Command::new(\"true\");",
        "regex::Regex::new(\"a\").unwrap();",
        "crate::config::Type::default().len();",
        "Self::Builder::new();",
        "Foo::<u8>::Inner::new();",
        "Config::DEFAULT.len();",
        "i32::MAX.count_ones();",
        "(std::cmp::Ordering::Less).reverse();",
        "std::cmp::Ordering::Less::reverse(order);",
        "module::value.len();",
        "\"regex::Regex.new(\"",
        "// regex::Regex.new(",
    ] {
        assert!(output.contains(expected), "missing {expected}\n{output}");
    }
}

#[test]
fn rust_conversion_parenthesizes_camel_case_path_value_receivers() {
    let output = rust_to_rusk(
        "fn f() -> u32 {\n    let order = std::cmp::Ordering::Less.reverse();\n    let less = Ordering::Less.is_lt();\n    let made = String::new().len();\n    i32::MAX.count_ones()\n}\n",
    )
    .unwrap();
    let roundtrip = rust(&output);
    assert!(roundtrip.contains("Ordering::Less).reverse()"), "{output}");
    assert!(roundtrip.contains("(Ordering::Less).is_lt()"), "{output}");
    assert!(roundtrip.contains("String::new().len()"), "{output}");
    assert!(roundtrip.contains("i32::MAX.count_ones()"), "{output}");
    assert!(!output.contains("(i32::MAX)"), "{output}");
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
fn rust_conversion_dots_paths_and_marks_generic_application() {
    let output = rust_to_rusk(
        "fn id<T>(x: Vec<T>) -> Option<T> {\n    std::mem::drop(x);\n    None\n}\n\nfn parse(value: &str) {\n    let _ = value.parse::<i32>();\n    let _ = std::mem::size_of::<Vec<u8>>();\n    let _ = Vec::<String>::new();\n    let _ = serde_json::from_str::<Config>(value);\n    let _ = [\"1\"].map(str::parse::<Vec<i32>>);\n    let _ = TABLE[0].len();\n    let _ = crate::config::TABLE[0].len();\n}\n",
    )
    .unwrap();
    assert!(output.contains("fn id[T](x: Vec[T]) -> Option[T]:"), "{output}");
    assert!(output.contains("std.mem.drop(x)"), "{output}");
    assert!(output.contains("value.parse[i32]()"), "{output}");
    assert!(output.contains("std.mem.size_of[Vec[u8]]()"), "{output}");
    assert!(output.contains("Vec[String].new()"), "{output}");
    assert!(
        output.contains("serde_json.from_str[Config](value)"),
        "{output}"
    );
    assert!(output.contains("str.parse::<Vec<i32>>"), "{output}");
    assert!(output.contains("let _ = TABLE[0].len()"), "{output}");
    assert!(output.contains("crate.config.TABLE[0].len()"), "{output}");
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
}

#[test]
fn only_indentation_tabs_are_rejected_and_rust_indentation_tabs_expand() {
    let output = rust("fn f() -> &'static str:\n    // a\tb\n    let s = \"x\ty\"\n    \"\n\tz\"\n");
    assert!(output.contains("// a\tb"), "{output}");
    assert!(output.contains("\"x\ty\""), "{output}");
    assert!(output.contains("\"\n\tz\""), "{output}");
    let rusk = rust_to_rusk("fn f() -> i32 {\n\tlet x = 1;\t// a\tb\n\tx\n}\n").unwrap();
    assert!(rusk.contains("    let x = 1\t// a\tb\n"), "{rusk}");
    assert!(rust(&rusk).contains("    let x = 1;"), "{rusk}");
}

#[test]
fn rust_conversion_keeps_arrows_inside_generic_arguments() {
    let output = rust_to_rusk(
        "pub struct Handlers {\n    pub list: Vec<fn(i32) -> i32>,\n    pub named: HashMap<String, Box<dyn Fn(&str) -> Option<usize>>>,\n}\n",
    )
    .unwrap();
    assert!(output.contains("pub list: Vec[fn(i32) -> i32]\n"), "{output}");
    assert!(
        output.contains("pub named: HashMap[String, Box[dyn Fn(&str) -> Option[usize]]]\n"),
        "{output}"
    );
}

#[test]
fn non_ascii_text_before_a_brace_does_not_panic() {
    let output = rust("fn f(참: bool) -> i32:\n    if 참 {\n        1\n    } else {\n        2\n    }\n");
    assert!(output.contains("    if 참 {\n"), "{output}");
    let _ = transpile("⳥{");
    assert!(rust_to_rusk("fn f() {\n    let 값 = é {\n    };\n}\n").is_ok());
}

#[test]
fn rust_brace_else_lines_continue_the_brace_chain() {
    let output = rust("fn pick(a: bool, b: bool) -> i32:\n    if a {\n        1\n    } else if b {\n        2\n    } else {\n        3\n    }\n");
    assert_eq!(
        output,
        "fn pick(a: bool, b: bool) -> i32 {\n    if a {\n        1\n    } else if b {\n        2\n    } else {\n        3\n    }\n}\n"
    );
    let output = rust("fn pick(a: bool) -> usize:\n    let v = if a {\n        vec![1]\n    } else {\n        vec![]\n    }\n    v.len()\n");
    assert!(output.contains("    } else {\n        vec![]\n    };\n"), "{output}");
}

#[test]
fn multiline_string_contents_survive_every_conversion() {
    let literal = "\"a  \nb // c;\n  d\"";
    let rusk = format!("fn main():\n    let s = {literal}\n    println!(\"{{s}}\")\n");
    let expected = format!("fn main() {{\n    let s = {literal};\n    println!(\"{{s}}\");\n}}\n");

    assert_eq!(rust(&rusk), expected);
    assert_eq!(format_source(&rusk, FormatOptions::default()).unwrap(), rusk);
    assert!(rust_to_rusk(&expected).unwrap().contains(literal));
}

#[test]
fn unclosed_groups_and_literals_at_end_of_file_are_errors() {
    for (source, line) in [
        ("fn main():\n    let x = (\n", 2),
        ("fn main():\n    let x = [\n        1,\n", 2),
        ("fn main():\n    let s = \"open\n", 2),
        ("fn main():\n    let s = r#\"open\"\n", 2),
        ("fn main():\n    /* open\n", 2),
        ("fn main():\n    let x =\n", 2),
        ("fn main():\n    let x = // value\n\n", 2),
    ] {
        let error = transpile(source).expect_err(source);
        assert_eq!(error.line, line, "{source:?}: {error}");
    }
    assert!(transpile("fn main():\n    let x = (\n        1\n    )\n").is_ok());
}

#[test]
fn block_headers_without_a_body_emit_an_empty_block() {
    assert_eq!(rust("fn main():\n"), "fn main() {\n}\n");
    assert_eq!(rust("impl A:\n    fn f(&self):\n"), "impl A {\n    fn f(&self) {\n    }\n}\n");
}

#[test]
fn formatting_drops_trailing_blank_lines() {
    let once = format_source("fn main():\n    f()\n\n    \n", FormatOptions::default()).unwrap();
    assert_eq!(once, "fn main():\n    f()\n");
    assert_eq!(format_source(&once, FormatOptions::default()).unwrap(), once);
}

#[test]
fn raw_strings_stay_open_until_their_closing_hashes() {
    let source = "fn main():\n    let s = r#\"say \"\" \nhi\"#\n    print!(\"{s}\")\n";
    assert_eq!(format_source(source, FormatOptions::default()).unwrap(), source);
    assert!(rust(source).contains("r#\"say \"\" \nhi\"#;"), "{}", rust(source));
}

#[test]
fn use_trees_lower_globs_and_groups() {
    let output = rust(
        "use rayon.prelude.*\npub use std.{fmt, io.{self, Read as R}}\npub(crate) use crate.a.B\nconst R: std.ops.Range[usize] = 0..2\nfn f(x: &[u8]) -> usize = x.len()\n",
    );
    assert!(output.contains("use rayon::prelude::*;\n"), "{output}");
    assert!(
        output.contains("pub use std::{fmt, io::{self, Read as R}};\n"),
        "{output}"
    );
    assert!(output.contains("pub(crate) use crate::a::B;\n"), "{output}");
    assert!(output.contains("const R: std::ops::Range<usize> = 0..2;"), "{output}");
    assert!(output.contains("    x.len()\n"), "{output}");
}

#[test]
fn inline_function_bodies_can_open_a_block() {
    let inline = "impl A:\n    fn f(x: i32) -> i32 = match x:\n        1 => 2\n        _ =>:\n            let y = x\n            y * 3\n";
    let block = "impl A:\n    fn f(x: i32) -> i32:\n        match x:\n            1 => 2\n            _ =>:\n                let y = x\n                y * 3\n";
    assert_eq!(rust(inline), rust(block));
    assert_eq!(
        format_source(inline, FormatOptions::default()).unwrap(),
        inline
    );
}

#[test]
fn every_std_format_macro_lowers_inline_expressions() {
    let output = rust(
        "fn show(out: &mut String, user: &User):\n    eprintln!(\"{user.name} {}\", 1)\n    write!(out, \"{user.name:>8}\").unwrap()\n    writeln!(\n        out,\n        \"{user.name}\"\n    ).unwrap()\n    my_eprintln!(\"{user.name}\")\n    let s = \"eprintln!(\\\"{user.name}\\\")\"\n    panic!(\"{user.name}\") // eprintln!(\"{user.name}\")\n",
    );
    assert!(
        output.contains("eprintln!(\"{1} {}\", 1, user.name);"),
        "{output}"
    );
    assert!(
        output.contains("write!(out, \"{:>8}\", user.name).unwrap();"),
        "{output}"
    );
    assert!(
        output.contains("writeln!(out, \"{}\", user.name).unwrap();"),
        "{output}"
    );
    assert!(
        output.contains("my_eprintln!(\"{user.name}\");"),
        "{output}"
    );
    assert!(
        output.contains("let s = \"eprintln!(\\\"{user.name}\\\")\";"),
        "{output}"
    );
    assert!(output.contains("panic!(\"{}\", user.name);"), "{output}");
}

#[test]
fn rust_conversion_inlines_format_arguments_used_once_in_order() {
    let output = rust_to_rusk(
        "fn f(cli: &Cli, index: usize, name: &str, out: &mut String) {\n    println!(\"Invalid {}\", cli.url);\n    let file = format!(\"{:03}.{name}\", index + 1);\n    eprintln!(\"{} {:?}\", name, cli.url);\n    write!(out, \"{}\", cli.url).unwrap();\n    println!(\n        \"{} on {}\",\n        name,\n        cli.url\n    );\n    println!(\"{0} {0}\", name);\n    println!(\"{1} {0}\", name, cli.url);\n    println!(\"{} {x}\", name, x = 1);\n    println!(\"{:>1$}\", name, 8);\n    println!(\"{}\", \"literal\");\n    println!(\"{}\", |x: u8| x);\n    println!(\"{{}} {}\", name);\n    my_println!(\"{}\", name);\n}\n",
    )
    .unwrap();
    for expected in [
        "println!(\"Invalid {cli.url}\")",
        "let file = format!(\"{index + 1:03}.{name}\")",
        "eprintln!(\"{name} {cli.url:?}\")",
        "write!(out, \"{cli.url}\").unwrap()",
        "println!(\"{name} on {cli.url}\")",
        "println!(\"{0} {0}\", name)",
        "println!(\"{1} {0}\", name, cli.url)",
        "println!(\"{} {x}\", name, x = 1)",
        "println!(\"{:>1$}\", name, 8)",
        "println!(\"{}\", \"literal\")",
        "println!(\"{}\", |x: u8| x)",
        "println!(\"{{}} {name}\")",
        "my_println!(\"{}\", name)",
    ] {
        assert!(output.contains(expected), "missing {expected}\n{output}");
    }
}

#[test]
fn rust_round_trip_keeps_format_argument_order() {
    let rusk = rust_to_rusk(
        "fn f(pos: usize, name: &str) {\n    println!(\"{} {}\", pos, g(pos));\n    println!(\"{:03}.{name}\", g(pos));\n    println!(\"{name} {}\", g(pos));\n    println!(\"{0} {0}\", name);\n}\n",
    )
    .unwrap();
    let output = rust(&rusk);
    for expected in [
        "println!(\"{} {}\", pos, g(pos));",
        "println!(\"{:03}.{}\", g(pos), name);",
        "println!(\"{name} {}\", g(pos));",
        "println!(\"{0} {0}\", name);",
    ] {
        assert!(
            output.contains(expected),
            "missing {expected}\n{rusk}\n{output}"
        );
    }
}

#[test]
fn rust_conversion_inlines_constant_format_receivers() {
    let output = rust_to_rusk(
        "static NAME: &str = \"a\";\nstatic NAMES: [&str; 1] = [\"a\"];\nfn f(name: &str) -> String {\n    println!(\"{} {}\", NAME.len(), name);\n    println!(\"{:?} {}\", NAMES[0].len(), NAMES.len());\n    println!(\"{} {}\", NAME, a::NAME.len());\n    format!(\n        \"Expected {} but \\\"{}\\\"\",\n        NAME.len(),\n        name\n    )\n}\n",
    )
    .unwrap();
    for expected in [
        "println!(\"{NAME.len()} {name}\")",
        "println!(\"{NAMES[0].len():?} {NAMES.len()}\")",
        "println!(\"{NAME} {a::NAME.len()}\")",
        "format!(\"Expected {NAME.len()} but \\\"{name}\\\"\")",
    ] {
        assert!(output.contains(expected), "missing {expected}\n{output}");
    }
}

#[test]
fn let_else_owns_a_colon_block() {
    let output = rust(
        "fn f(v: Option<i32>) -> i32:\n    let Some(x) = v else:\n        return 0\n    let s = \"let a = b else:\" // let c = d else:\n    let Some(y) = v.map(|n| n + 1) else: // none\n        eprintln!(\"none\")\n        return 1\n    if x > y:\n        let Some(z) = v else:\n            return 2\n        z\n    else:\n        x\n",
    );
    assert!(
        output.contains("    let Some(x) = v else {\n        return 0;\n    };\n"),
        "{output}"
    );
    assert!(
        output.contains("let s = \"let a = b else:\"; // let c = d else:"),
        "{output}"
    );
    assert!(
        output.contains(
            "    let Some(y) = v.map(|n| n + 1) else { // none\n        eprintln!(\"none\");\n        return 1;\n    };\n"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "        let Some(z) = v else {\n            return 2;\n        };\n        z\n"
        ),
        "{output}"
    );
}

#[test]
fn rust_conversion_writes_let_else_blocks_with_a_colon() {
    let output = rust_to_rusk(
        "fn f(v: Option<i32>) -> i32 {\n    let Some(x) = v else {\n        return 0;\n    };\n    let Some(y) = v else { return 1 };\n    x + y\n}\n",
    )
    .unwrap();
    assert!(
        output.contains("    let Some(x) = v else:\n        return 0\n"),
        "{output}"
    );
    assert!(
        output.contains("let Some(y) = v else { return 1 }"),
        "{output}"
    );
}

#[test]
fn trailing_if_owns_a_colon_block_chain() {
    let output = rust(
        "struct S;\nimpl S:\n    const BIG: i32 = 5\n    fn f(n: i32) -> i32:\n        let size = if n > 10:\n            1\n        else if n < 0: // negative\n            2\n        else:\n            3\n        let s = \"let a = if b:\" // let c = if d:\n        let t = if n > 0 then 1 else 2\n        let mut total = 0\n        total += if Self::BIG < n:\n            let inner = if n % 2 == 0:\n                n\n            else:\n                -n\n            inner\n        else:\n            0\n        let notif = size\n        return if total == 0:\n            notif + t\n        else:\n            s.len() as i32\n",
    );
    assert!(
        output.contains(
            "        let size = if n > 10 {\n            1\n        } else if n < 0 { // negative\n            2\n        } else {\n            3\n        };\n"
        ),
        "{output}"
    );
    assert!(
        output.contains("let s = \"let a = if b:\"; // let c = if d:"),
        "{output}"
    );
    assert!(
        output.contains(
            "        total += if Self::BIG < n {\n            let inner = if n % 2 == 0 {\n                n\n            } else {\n                -n\n            };\n            inner\n        } else {\n            0\n        };\n"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "        return if total == 0 {\n            notif + t\n        } else {\n            s.len() as i32\n        }\n"
        ),
        "{output}"
    );
}

#[test]
fn rust_conversion_writes_trailing_if_blocks_with_a_colon() {
    let output = rust_to_rusk(
        "fn f(n: i32) -> i32 {\n    let size = if n > 10 {\n        1\n    } else if n < 0 {\n        2\n    } else {\n        3\n    };\n    let t = if n > 0 { 1 } else { 2 };\n    return if size == t {\n        let inner = if n % 2 == 0 {\n            n\n        } else {\n            -n\n        };\n        inner\n    } else {\n        t\n    };\n}\n",
    )
    .unwrap();
    assert!(
        output.contains("    let size = if n > 10:\n        1\n    else if n < 0:\n        2\n    else:\n        3\n"),
        "{output}"
    );
    assert!(
        output.contains("let t = if n > 0 then 1 else 2"),
        "{output}"
    );
    assert!(
        output.contains(
            "    return if size == t:\n        let inner = if n % 2 == 0:\n            n\n        else:\n            -n\n        inner\n    else:\n        t\n"
        ),
        "{output}"
    );
}

#[test]
fn unused_block_values_end_with_statements() {
    let output = rust(
        "fn f(c: bool, m: M) -> i32:\n    for i in 0..3:\n        m.insert(i, i)\n    'outer: while c:\n        m.insert(0, 0)\n    if c:\n        m.insert(1, 1)\n    match c:\n        true => m.insert(2, 2)\n        false => None\n    if c:\n        1\n    else:\n        2\nfn g(c: bool, m: M):\n    if c:\n        m.insert(3, 3)\n    else:\n        m.insert(4, 4)\n",
    );
    for expected in [
        "        m.insert(i, i);\n",
        "        m.insert(0, 0);\n",
        "        m.insert(1, 1);\n",
        "        false => None,\n    };\n",
        "    if c {\n        1\n    } else {\n        2\n    }\n}\n",
        "        m.insert(3, 3);\n",
        "        m.insert(4, 4);\n",
    ] {
        assert!(output.contains(expected), "missing {expected:?}\n{output}");
    }

}

#[test]
fn std_macro_arguments_lower_like_expressions() {
    let output = rust(
        "fn demo(a: bool, xs: X, op: Op):\n    println!(\"{}, {}\", \"a, Foo.new()\", Foo.new())\n    writeln!(std.io.stdout(), \"{}\", std.i32.MAX)\n    let v = vec![Vec[i32].new(), std.vec.Vec.new()]\n    let r = vec![Foo.new(); 3]\n    assert!(xs.iter().any(|x| *x == std.i32.MAX), \"missing {xs.len()}\")\n    assert_eq!(Foo.new(), if a then 1 else 2)\n    dbg!(Foo.new())\n    let m = matches!(op, Op.Add(_) | Op.Mul(_))\n    let u = my_macro!(Foo.new())\n    let w = vec![\n        Foo.new(), // a, Foo.new()\n        HashMap::<u8, u8>::new(),\n    ]\n",
    );
    for expected in [
        "println!(\"{}, {}\", \"a, Foo.new()\", Foo::new());",
        "writeln!(std::io::stdout(), \"{}\", std::i32::MAX);",
        "vec![Vec::<i32>::new(), std::vec::Vec::new()];",
        "vec![Foo::new(); 3];",
        "assert!(xs.iter().any(|x| *x == std::i32::MAX), \"missing {}\", xs.len());",
        "assert_eq!(Foo::new(), if a { 1 } else { 2 });",
        "dbg!(Foo::new());",
        "matches!(op, Op::Add(_) | Op::Mul(_));",
        "my_macro!(Foo.new());",
        "        Foo::new(), // a, Foo.new()\n        HashMap::<u8, u8>::new(),\n",
    ] {
        assert!(output.contains(expected), "missing {expected}\n{output}");
    }
}

#[test]
fn rust_conversion_keeps_std_macro_argument_receivers() {
    let rusk = rust_to_rusk(
        "const TEXT: &str = \"a\";\nfn f(xs: &[fn() -> usize]) -> usize {\n    assert!(TEXT.len() > 0);\n    let v = vec![TEXT.len(); 2];\n    let called = vec![xs[0]()];\n    dbg!(std::cmp::Ordering::Less.reverse());\n    v.len() + called.len()\n}\n",
    )
    .unwrap();
    let output = rust(&rusk);
    for expected in [
        "assert!(TEXT.len() > 0);",
        "vec![TEXT.len(); 2];",
        "vec![(xs[0])()];",
        "dbg!((std::cmp::Ordering::Less).reverse());",
    ] {
        assert!(output.contains(expected), "missing {expected}\nRusk:\n{rusk}\n{output}");
    }
}

#[test]
fn names_not_bound_as_values_are_paths() {
    let output = rust(
        "use std.{env, fs, io.{self, Read as _}}\nuse std.collections as coll\nmod util:\n    pub fn one() -> i32 = 1\nconst TABLE: [&str; 1] = [\"a\"]\nfn main():\n    let args = env.args()\n    fs.write(\"a\", \"b\")\n    let input = io.stdin()\n    let map = coll.HashMap[u8, u8].new()\n    let one = util.one()\n    let env_len = (env).len()\n    let text = \"env.args()\" // env.args(\n    let re = regex.Regex.new(\"a\")\n    let null = serde_json.Value.Null\n    let shown = format!(\"{env.var(\"A\").is_ok()}\")\n    let first = TABLE[0].len() + TABLE.len()\n    let size = make().inner.len() + args.len()\n    let range = 0..one.max(1)\n",
    );
    for expected in [
        "let args = env::args();",
        "fs::write(\"a\", \"b\");",
        "let input = io::stdin();",
        "let map = coll::HashMap::<u8, u8>::new();",
        "let one = util::one();",
        "let env_len = (env).len();",
        "let text = \"env.args()\"; // env.args(",
        "let re = regex::Regex::new(\"a\");",
        "let null = serde_json::Value::Null;",
        "format!(\"{}\", env::var(\"A\").is_ok())",
        "let first = TABLE[0].len() + TABLE.len();",
        "let size = make().inner.len() + args.len();",
        "let range = 0..one.max(1);",
    ] {
        assert!(output.contains(expected), "missing {expected}\n{output}");
    }
    assert!(rust("fn f(env: &str) -> usize = env.len()\n").contains("env.len()"));
}

#[test]
fn rust_conversion_keeps_bound_receivers_and_dots_paths() {
    let source = "use std::env;\nfn f(path: &str, guid: Guid) -> usize {\n    let env = path;\n    let count = env::args().count();\n    let re = regex::Regex::new(\"a\");\n    env.len() + count + guid.Data1 as usize + TABLE[0].len()\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    let output = rust(&rusk);
    for expected in [
        "let count = env::args().count();",
        "let re = regex::Regex::new(\"a\");",
        "env.len() + count + guid.Data1 as usize + TABLE[0].len()",
    ] {
        assert!(output.contains(expected), "missing {expected}\nRusk:\n{rusk}\n{output}");
    }
    for expected in [
        "env::args().count()",
        "regex.Regex.new(",
        "env.len() + count + guid.Data1",
    ] {
        assert!(rusk.contains(expected), "missing {expected}\n{rusk}");
    }
}

#[test]
fn closure_heads_own_colon_blocks() {
    let output = rust(
        "fn demo(xs: &mut Vec[i32], a: bool, b: bool) -> usize:\n    let describe = |n: usize| -> String:\n        if n > 1:\n            format!(\"long {n}\")\n        else:\n            \"short\".to_string()\n    let shout = move |s: &str|:\n        let upper = s.to_uppercase()\n        upper + \"!\"\n    xs.sort_by(|a, b|:\n        b.cmp(a)\n    )\n    let handle = std.thread.spawn(move ||:\n        1 + 1\n    )\n    if a || b:\n        xs.clear()\n    describe(2).len() + shout(\"a\").len()\n",
    );
    for expected in [
        "    let describe = |n: usize| -> String {\n        if n > 1 {\n",
        "    let shout = move |s: &str| {\n        let upper = s.to_uppercase();\n        upper + \"!\"\n    };\n",
        "    xs.sort_by(|a, b| {\n        b.cmp(a)\n    });\n",
        "    let handle = std::thread::spawn(move || {\n        1 + 1\n    });\n",
        "    if a || b {\n        xs.clear();\n    }\n",
    ] {
        assert!(output.contains(expected), "missing {expected:?}\n{output}");
    }
}

fn assert_lines(output: &str, expected: &[&str]) {
    for expected in expected {
        assert!(output.contains(expected), "missing {expected:?}\n{output}");
    }
}

#[test]
fn let_bindings_shadow_imported_module_names_after_the_statement() {
    let output = rust(
        "use std.{env, fs}\nfn main():\n    let args = env.args()\n    let env = env.var(\"HOME\").unwrap()\n    let n = env.len()\n    let text = \"let fs = 1\" // let fs = 2\n    fs.write(\"a\", \"b\")\n",
    );
    assert_lines(
        &output,
        &[
            "let args = env::args();",
            "let env = env::var(\"HOME\").unwrap();",
            "let n = env.len();",
            "fs::write(\"a\", \"b\");",
        ],
    );
}

#[test]
fn parameters_and_closure_parameters_shadow_module_names() {
    let output = rust(
        "use std.{env, fs}\nfn f(env: &str) -> usize = env.len()\nfn g(mut fs: Vec[u8], (a, env): (u8, String)):\n    let n = fs.len() + env.len()\nfn h(xs: Vec[String]) -> usize = xs.iter().map(|env: &String| env.len()).sum()\nfn k():\n    let run = move |fs|:\n        fs.len()\n    let paths = env.args()\n    fs.write(\"a\", \"b\")\n",
    );
    assert_lines(
        &output,
        &[
            "    env.len()\n",
            "let n = fs.len() + env.len();",
            "|env: &String| env.len()",
            "        fs.len()\n",
            "let paths = env::args();",
            "fs::write(\"a\", \"b\");",
        ],
    );
}

#[test]
fn pattern_bindings_scope_to_their_arm_or_block() {
    let output = rust(
        "use std.env\nfn main():\n    if let Some(env) = env.args().next():\n        println!(\"{}\", env.len())\n    else:\n        env.args().count()\n    let args = env.args()\n    match env.var(\"A\"):\n        Ok(env) => env.len()\n        Err(_) => env.args().count()\n    match pair:\n        whole @ (ref mut env, _) if env.len() > 0 =>:\n            env.clear()\n        _ => env.args().count()\n    for (i, env) in env.args().enumerate():\n        env.len()\n    while let Some(Point { x: env, y }) = next():\n        env.len()\n    env.vars()\n",
    );
    assert_lines(
        &output,
        &[
            "if let Some(env) = env::args().next() {",
            "println!(\"{}\", env.len());",
            "        env::args().count();\n",
            "let args = env::args();",
            "match env::var(\"A\") {",
            "Ok(env) => env.len(),",
            "Err(_) => env::args().count(),",
            "whole @ (ref mut env, _) if env.len() > 0 => {",
            "            env.clear()\n",
            "_ => env::args().count(),",
            "for (i, env) in env::args().enumerate() {",
            "while let Some(Point { x: env, y }) = next() {",
            "        env.len();\n",
            "    env::vars();\n",
        ],
    );
    assert!(!output.contains("env::len"), "{output}");
}

#[test]
fn block_bindings_go_out_of_scope_after_the_block() {
    let output = rust(
        "use std.env\nfn main():\n    if ready:\n        let env = 1\n        env.count_ones()\n    env.args()\n",
    );
    assert_lines(
        &output,
        &["        env.count_ones();\n", "    env::args();\n"],
    );

    let output = rust(
        "use std.env\nfn main():\n    let (\n        env,\n        other,\n    ) = pair\n    env.len()\n",
    );
    assert_lines(&output, &["    env.len();\n"]);
}

#[test]
fn rust_conversion_drops_receiver_parentheses_for_bound_values() {
    let source = "use std::env;\nfn f(path: &str) -> usize {\n    let env = path;\n    env.len() + env::args().count()\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    assert!(rusk.contains("env.len() + env::args().count()"), "{rusk}");
    assert!(
        rust(&rusk).contains("env.len() + env::args().count()"),
        "{rusk}"
    );
}

#[test]
fn trait_parameters_bind_and_nested_items_do_not_see_enclosing_locals() {
    let output = rust(
        "use std.env\ntrait T:\n    fn f(&self, env: &str) -> usize = env.len()\nfn main():\n    let env = 1\n    fn inner() -> usize = env.args().count()\n    env.count_ones()\n",
    );
    assert_lines(
        &output,
        &[
            "        env.len()\n",
            "        env::args().count()\n",
            "    env.count_ones();\n",
        ],
    );
}

#[test]
fn bindings_inside_one_line_are_values_on_that_line() {
    let output = rust(
        "use std.env\nfn main():\n    let n = if let Some(env) = x then env.len() else 0\n    let ok = matches!(v, Some(env) if env.len() > 0)\n    let m = { let env = 1; env.count_ones() }\n    if let Some(env) = x then env.len() else 0\n    env.args()\n",
    );
    assert_lines(
        &output,
        &[
            "let n = if let Some(env) = x { env.len() } else { 0 };",
            "matches!(v, Some(env) if env.len() > 0)",
            "let env = 1; env.count_ones()",
            "if let Some(env) = x { env.len() } else { 0 }",
            "    env::args();\n",
        ],
    );
}

#[test]
fn lifetimes_const_generics_and_later_lets_on_a_line_bind_values() {
    let output = rust(
        "use std.env\nfn s(a: &'static str, b: &str) -> usize = a.len() + b.len()\nfn lt['a](x: &'a str, env: &'a str) -> usize = env.len()\nfn konst[const N: usize, T: Default]() -> T = T.default().max(N.pow(2))\nfn other() -> usize = N.pow(2)\nimpl[const N: usize] Grid[N]:\n    fn size(&self) -> usize = N.pow(2)\nfn main():\n    let v = 1; let env = vec![1]\n    env.len()\n",
    );
    assert_lines(
        &output,
        &[
            "    a.len() + b.len()\n",
            "    env.len()\n",
            "    T::default().max(N.pow(2))\n",
            "fn other() -> usize {\n    N::pow(2)\n",
            "        N.pow(2)\n",
            "    env.len();\n",
        ],
    );

    let source = "fn s<'a>(a: &'a str, b: &str) -> usize {\n    a.len() + b.len()\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    assert!(rust(&rusk).contains("a.len() + b.len()"), "{rusk}");
}

#[test]
fn multi_line_where_clauses_keep_their_lines() {
    let source = "impl<T> Show for Wrapper<T>\nwhere\n    T: Debug,\n{\n    fn show<F>(&self, f: F) -> String\n    where\n        F: Fn(&str) -> String,\n    {\n        f(\"a\")\n    }\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    assert!(
        rusk.contains("impl[T] Show for Wrapper[T]\nwhere\n    T: Debug:\n"),
        "{rusk}"
    );
    assert!(
        rusk.contains("    where\n        F: Fn(&str) -> String:\n"),
        "{rusk}"
    );
    assert_eq!(rust(&rusk), source);
}

#[test]
fn empty_loop_bodies_stay_empty() {
    let output = rust("fn f(mut n: u8):\n    while n > 0 {}\n    for _ in 0..n {}\n    n = 1\n");
    assert_lines(&output, &["    while n > 0 {}\n", "    for _ in 0..n {}\n"]);
    assert!(!output.contains("{} {"), "{output}");
}

#[test]
fn module_declarations_keep_their_semicolon() {
    let rusk = rust_to_rusk("#[macro_use]\nmod backtrace;\npub(crate) mod chain;\nmod inline {}\n").unwrap();
    let output = rust(&rusk);
    assert_lines(&output, &["mod backtrace;\n", "pub(crate) mod chain;\n"]);
    assert!(!output.contains("; {"), "{output}");
    assert_eq!(rust("mod a\nmod b;\n"), "mod a;\nmod b;\n");
}

#[test]
fn trailing_heads_with_struct_literal_scrutinees_own_blocks() {
    let rusk = rust_to_rusk(
        "fn f(k: u8) -> u8 {\n    let v = if get(K { k }) {\n        1\n    } else {\n        2\n    };\n    v\n}\n",
    )
    .unwrap();
    assert_lines(&rusk, &["    let v = if get(K { k }):\n"]);
}

#[test]
fn comments_keep_format_arguments() {
    let rusk = rust_to_rusk(
        "/// eprintln!(\"{}\", err);\npub fn f(x: u8) {\n    // println!(\"{}\", x);\n    println!(\"{}\", x); // print!(\"{}\", x);\n}\n",
    )
    .unwrap();
    assert_lines(
        &rusk,
        &[
            "/// eprintln!(\"{}\", err);\n",
            "    // println!(\"{}\", x);\n",
            "    println!(\"{x}\") // print!(\"{}\", x);\n",
        ],
    );
}

#[test]
fn macro_rules_comments_keep_their_semicolons() {
    let rusk = rust_to_rusk("macro_rules! m {\n    // m!(a);\n    ($a:expr) => { $a };\n}\n").unwrap();
    assert_lines(&rusk, &["    // m!(a);\n"]);
}

#[test]
fn closure_return_types_without_bodies_do_not_panic() {
    let _ = transpile("||->let :r#\"\"#");
    let _ = transpile("let f = || -> u8 let x: u8 = 1\n");
}

#[test]
fn closure_parameters_bind_after_fields_async_and_branches() {
    let output = rust(
        "fn f(c: bool, xs: Vec<String>):\n    let p = P{ f: |s: String| s.len() }\n    let g = async |s: String| s.len()\n    let h = if c then |s: String| s.len() else |t: String| t.len()\n    println!(\"{xs.iter().map(|s| s.len()).sum::<usize>()}\")\n",
    );
    assert_lines(
        &output,
        &[
            "P{ f: |s: String| s.len() }",
            "async |s: String| s.len();",
            "if c { |s: String| s.len() } else { |t: String| t.len() }",
            "xs.iter().map(|s| s.len()).sum::<usize>()",
        ],
    );
}

#[test]
fn lets_in_match_arm_bodies_bind_values() {
    let output = rust(
        "fn f(o: Option<String>, q: Option<Option<String>>) -> usize:\n    let a = match o:\n        Some(s) => { let t = s; t.len() }\n        None => 0\n    match q:\n        Some(i) => if let Some(v) = i then v.len() else a\n        None => 0\n",
    );
    assert_lines(&output, &["let t = s; t.len() }", "{ v.len() }"]);
}

#[test]
fn trailing_comments_keep_a_tail_match_the_value() {
    let output = rust(
        "fn f(c: Option<u8>) -> u8:\n    match c:\n        Some(x) => x\n        None => 0\n    // end\n",
    );
    assert_lines(&output, &["        None => 0,\n    }\n    // end\n}"]);
}

#[test]
fn if_then_else_with_an_indented_block_is_not_let_else() {
    assert!(transpile("fn f(c: bool):\n    let x = if c then 1 else:\n        2\n").is_err());
}

#[test]
fn statics_are_values() {
    let output =
        rust("static mut counter: u8 = 0\nfn f() -> u32 = unsafe { counter.count_ones() }\n");
    assert_lines(&output, &["unsafe { counter.count_ones() }"]);
}

#[test]
fn closure_parameters_in_std_macros_lower_their_types() {
    let output = rust("fn f():\n    let v = vec![|a: u8, b: Vec[u8]| b.len()]\n");
    assert_lines(&output, &["vec![|a: u8, b: Vec<u8>| b.len()]"]);
}

#[test]
fn inline_macro_arms_without_a_closing_brace_do_not_panic() {
    let _ = rust_to_rusk("macro_rules! r#\"{>\n    \"#é");
}

#[test]
fn inline_macro_arms_before_a_block_comment_keep_the_comment_closed() {
    let output = rust_to_rusk("macro_rules! m {() => {\n    1\n}} /* end */\n").unwrap();
    assert_lines(&output, &["/* end */"]);
}

#[test]
fn modules_keep_paths_beside_functions_of_the_same_name() {
    let output = rust(
        "use std.env\nmod config:\n    pub fn load() -> u32 = 1\n\npub fn config() -> u32 = 2\n\npub fn g() -> usize:\n    let a = config.load()\n    env.args().count()\nfn env() {}\n",
    );
    assert_lines(&output, &["config::load()", "env::args()"]);
}

#[test]
fn local_imports_of_a_function_keep_it_a_value() {
    let output = rust(
        "fn parse_args(t: u8) -> u8 = t\n\nfn f(t: u8):\n    let n = parse_args.clone()(t)\n\nmod tests:\n    use super.parse_args\n    use crate.parse_args as p\n",
    );
    assert_lines(&output, &["parse_args.clone()(t)"]);
}

#[test]
fn matches_whose_arms_end_with_values_end_with_a_semicolon() {
    let output = rust(
        "fn g():\n    ()\nfn f(o: Option[u8], c: bool):\n    match o:\n        Some(_) =>:\n            if c:\n                1\n            else:\n                2\n        None =>:\n            loop:\n                break 5\n    g()\n",
    );
    assert_lines(
        &output,
        &["            } else {\n                2\n            }\n", "    };\n    g();\n"],
    );
}

#[test]
fn blocks_opened_inside_groups_end_with_a_closing_line() {
    assert_eq!(
        rust(
            "fn f(mut xs: Vec[i32]) -> i32:\n    xs.sort_by(|a, b|:\n        b.cmp(a)\n    )\n    xs.iter().map(|x|:\n        x * 2 // (\n    ).sum()\n"
        ),
        "fn f(mut xs: Vec<i32>) -> i32 {\n    xs.sort_by(|a, b| {\n        b.cmp(a)\n    });\n    xs.iter().map(|x| {\n        x * 2 // (\n    }).sum()\n}\n"
    );
    assert_eq!(
        rust(
            "fn f(xs: &[i32]) -> usize:\n    xs\n        .iter()\n        .filter(|x|:\n            **x > 0\n        )\n        .count()\n"
        ),
        "fn f(xs: &[i32]) -> usize {\n    xs\n        .iter()\n        .filter(|x| {\n            **x > 0\n        })\n        .count()\n}\n"
    );
    assert_eq!(
        rust(
            "fn f(a: Option[u8], c: bool) -> u8:\n    let m = max(match a:\n        Some(x) => x\n        None => 0\n    , 1)\n    match a:\n        Some(p) => max(if c:\n            1\n        else:\n            2\n        , p)\n        None => m\n"
        ),
        "fn f(a: Option<u8>, c: bool) -> u8 {\n    let m = max(match a {\n        Some(x) => x,\n        None => 0,\n    }, 1);\n    match a {\n        Some(p) => max(if c {\n            1\n        } else {\n            2\n        }, p),\n        None => m,\n    }\n}\n"
    );
    assert_eq!(
        rust(
            "fn f(o: Option[Vec[u8]]) -> usize:\n    match o:\n        Some(v) => v.iter().map(|x|:\n            *x as usize\n        ).sum()\n        None => 0\n"
        ),
        "fn f(o: Option<Vec<u8>>) -> usize {\n    match o {\n        Some(v) => v.iter().map(|x| {\n            *x as usize\n        }).sum(),\n        None => 0,\n    }\n}\n"
    );
    let source = "fn f(mut xs: Vec<i32>) -> usize {\n    xs.sort_by(|a, b| {\n        b.cmp(a)\n    });\n    xs.iter()\n        .filter(|x| {\n            **x > 0\n        })\n        .count()\n}\n";
    assert_eq!(
        rust_to_rusk(source).unwrap(),
        "fn f(mut xs: Vec[i32]) -> usize:\n    xs.sort_by(|a, b| {\n        b.cmp(a)\n    })\n    xs.iter()\n        .filter(|x| {\n            **x > 0\n        })\n        .count()\n"
    );
    assert_eq!(
        rust(
            "fn f(a: Option[u8], b: Option[u8], xs: &[u8]) -> u8:\n    let m = max(match a:\n        Some(x) => x\n        None => 0\n    , match b:\n        Some(x) => x\n        None => 0\n    )\n    g(|x|:\n        x\n    , |y| {\n        y\n    })\n    xs.iter().map(|x|:\n        *x\n    // between\n    ).sum()\n"
        ),
        "fn f(a: Option<u8>, b: Option<u8>, xs: &[u8]) -> u8 {\n    let m = max(match a {\n        Some(x) => x,\n        None => 0,\n    }, match b {\n        Some(x) => x,\n        None => 0,\n    });\n    g(|x| {\n        x\n    }, |y| {\n        y\n    });\n    xs.iter().map(|x| {\n        *x\n        // between\n    }).sum()\n}\n"
    );
    for source in [
        "fn f(a: Option<u8>, b: Option<u8>) -> u8 {\n    let m = max(match a {\n        Some(x) => x,\n        None => 0,\n    }, match b {\n        Some(x) => x,\n        None => 0,\n    });\n    g(|x| {\n        x\n    }, |y| {\n        y\n    });\n    m\n}\n",
        "fn f(total: i32, c: bool) -> i32 {\n    max(if c {\n        total\n    } else if total > 1 { 1 } else { 0 }, 0)\n}\n",
        "fn f(o: Option<u8>) -> u8 {\n    o.map_or_else(|| {\n        0\n    }, move |y| { // keep\n        y\n    })\n}\n",
        "fn f() -> u8 {\n    let s = S { a: g(|x| {\n        x\n    })};\n    s.a\n}\n",
    ] {
        assert_eq!(rust(&rust_to_rusk(source).unwrap()), source);
    }
}

#[test]
fn blocks_opened_inside_groups_without_a_closing_line_are_errors() {
    for (source, line) in [
        (
            "fn f(mut xs: Vec[i32]):\n    xs.sort_by(|a, b|:\n        b.cmp(a)\n    xs.len()\n",
            2,
        ),
        (
            "fn f(xs: &[i32]) -> i32:\n    xs.iter().map(|x|:\n        x * 2\n",
            2,
        ),
        (
            "fn f(xs: &[i32]) -> i32:\n    xs.iter().map(|x| {\n        x * 2\n",
            2,
        ),
        (
            "fn f(a: Option[u8], c: bool) -> u8:\n    match a:\n        Some(p) => max(if c:\n            1\n        else:\n            2\n        None => 0\n",
            5,
        ),
        (
            "fn f(xs: &[i32]) -> usize:\n    xs\n        .iter()\n        .filter(|x|:\n            **x > 0\n        .count()\n",
            4,
        ),
        ("fn f() -> usize:\n    xs.map(|x|:\n        1\n    ]\n    2\n", 4),
        ("fn f() -> usize:\n    xs.map(|x|:\n        1\n    }\n    2\n", 4),
        ("fn f() -> usize:\n    xs.map(|x|:\n        1\n    ))\n    2\n", 4),
        ("fn f() -> usize:\n    xs.map(|x| {\n        1\n    }\n    2\n", 4),
        ("fn f() -> u8:\n    let s = S { a: g(|x| {\n        x\n    })\n    s.a\n", 4),
        ("fn f() -> u8:\n    let x = 1\n    , |y|:\n        y\n", 3),
        ("fn f(c: bool) -> u8:\n    if c:\n        1;\n    , |y|:\n        y\n    0\n", 4),
        ("fn f(c: bool) -> u8:\n    if c {\n        1;\n    }, |y| {\n        y\n    }\n    0\n", 4),
    ] {
        let error = transpile(source).expect_err(source);
        assert_eq!(error.line, line, "{source:?}: {error}");
    }
    assert!(transpile("fn f():\n    let g = |x: u8|:\n        x + 1\n    g(1)\n").is_ok());
}

#[test]
fn multi_line_headers_end_with_a_colon_on_their_last_line() {
    assert_eq!(
        rust(
            "pub fn show[T](value: T) -> String\nwhere\n    T: std.fmt.Debug:\n    format!(\"{value:?}\")\n"
        ),
        "pub fn show<T>(value: T) -> String\nwhere\n    T: std::fmt::Debug,\n{\n    format!(\"{value:?}\")\n}\n"
    );
    assert_eq!(
        rust(
            "fn check(a: bool, B: bool) -> bool:\n    if a // (\n        && B: // ends in :\n        return true\n    let x = if a\n        && B:\n        1\n    else:\n        2\n    while let Some(y) = it\n        .next():\n        f(y)\n    x > 1\n"
        ),
        "fn check(a: bool, B: bool) -> bool {\n    if a // (\n        && B // ends in :\n    {\n        return true;\n    }\n    let x = if a\n        && B\n    {\n        1\n    } else {\n        2\n    };\n    while let Some(y) = it\n        .next()\n    {\n        f(y);\n    }\n    x > 1\n}\n"
    );
    assert_eq!(
        rust("pub fn long(\n    a: u8,\n    b: u8,\n) -> u8:\n    a + b\n"),
        "pub fn long(\n    a: u8,\n    b: u8,\n) -> u8 {\n    a + b\n}\n"
    );
    assert_eq!(
        rust(
            "pub fn names(xs: Vec[String]) -> Vec[String] = xs\n    .into_iter()\n    .map(|x|:\n        x.to_uppercase()\n    )\n    .collect()\n"
        ),
        "pub fn names(xs: Vec<String>) -> Vec<String> {\n    xs\n        .into_iter()\n        .map(|x| {\n            x.to_uppercase()\n        })\n        .collect()\n}\n"
    );
    assert_eq!(
        rust(
            "trait Show:\n    fn show[T](&self, value: T)\n    where\n        T: Copy\n    fn name(&self) -> String\n"
        ),
        "trait Show {\n    fn show<T>(&self, value: T)\n    where\n        T: Copy;\n    fn name(&self) -> String;\n}\n"
    );
    assert_eq!(
        rust("fn f(b: bool) -> u8:\n    let y =\n        if b:\n            1\n        else:\n            2\n    y\n"),
        "fn f(b: bool) -> u8 {\n    let y =\n        if b {\n            1\n        } else {\n            2\n        };\n    y\n}\n"
    );
    assert_eq!(
        rust("impl Foo for Bar\nwhere\n    T: Copy,\n{}\n"),
        "impl Foo for Bar\nwhere\n    T: Copy,\n{}\n"
    );
    assert_eq!(
        rust("fn f[T](t: T) -> usize\nwhere T:\n    Clone + Copy:\n    0\n"),
        "fn f<T>(t: T) -> usize\nwhere T:\n    Clone + Copy,\n{\n    0\n}\n"
    );
    assert_eq!(
        rust("fn h[T](t: T, u: u8)\n    -> u8\nwhere T: Fn(u8, u8):\n    u\n"),
        "fn h<T>(t: T, u: u8)\n    -> u8\nwhere T: Fn(u8, u8),\n{\n    u\n}\n"
    );
    assert_eq!(
        rust("fn h<T>(t: T)\n    -> u8\nwhere T: Into<Result<u8, ()>>:\n    0\n"),
        "fn h<T>(t: T)\n    -> u8\nwhere T: Into<Result<u8, ()>>,\n{\n    0\n}\n"
    );
    assert_eq!(
        rust("fn f(b: &str) -> u8:\n    let y =\n        if b == \"=>\":\n            1\n        else:\n            2\n    y\n"),
        "fn f(b: &str) -> u8 {\n    let y =\n        if b == \"=>\" {\n            1\n        } else {\n            2\n        };\n    y\n}\n"
    );
    let raw = "fn s() -> usize:\n    let c = vec![\n        {\n        },\n    ]\n    c.len()\n";
    assert!(rust(raw).contains("        {\n        },\n"), "{}", rust(raw));
    assert_eq!(
        rust_to_rusk("pub trait Bits:\n    Clone\n    + Copy\n{\n    const ONE: Self;\n}\n").unwrap(),
        "pub trait Bits: Clone + Copy:\n    const ONE: Self;\n"
    );
    for source in [
        "impl<T> Other for Vec<T>\nwhere\n    T: Copy,\n{\n}\n",
        "fn f(b: u8) {\n    while b\n        .next()\n        .is_some()\n    {}\n    g();\n}\n",
    ] {
        let rusk = rust_to_rusk(source).unwrap();
        assert!(!rusk.contains("{}"), "{rusk}");
        assert_eq!(rust(&rusk), source);
    }
    let rust_source = "pub fn show<T>(value: T) -> String\nwhere\n    T: std::fmt::Debug, // bound\n{\n    if value.is_ok()\n        && value.len() > 1\n    {\n        return String::new();\n    }\n    format!(\"{value:?}\")\n}\n";
    let rusk = rust_to_rusk(rust_source).unwrap();
    assert_eq!(
        rusk,
        "pub fn show[T](value: T) -> String\nwhere\n    T: std.fmt.Debug: // bound\n    if value.is_ok()\n        && value.len() > 1:\n        return String.new()\n    format!(\"{value:?}\")\n"
    );
    assert_eq!(rust(&rusk), rust_source);
}

#[test]
fn a_colon_line_alone_does_not_end_a_header() {
    for (source, line) in [
        (
            "fn f(a: bool, b: bool):\n    if a\n        && b\n    :\n        g()\n",
            4,
        ),
        ("fn f[T](x: T)\nwhere\n    T: Copy,\n:\n    g()\n", 4),
    ] {
        let error = transpile(source).expect_err(source);
        assert_eq!(error.line, line, "{source:?}: {error}");
    }
}

#[test]
fn equals_signs_inside_signature_generics_do_not_start_a_body() {
    assert_eq!(
        rust("pub fn evens(xs: Vec[u8]) -> impl Iterator<Item = u8> = xs.into_iter()\n"),
        "pub fn evens(xs: Vec<u8>) -> impl Iterator<Item = u8> {\n    xs.into_iter()\n}\n"
    );
    assert_eq!(
        rust("fn f[T](t: T) -> u8\nwhere T: Iterator<Item = (u8, u16)>:\n    0\n"),
        "fn f<T>(t: T) -> u8\nwhere T: Iterator<Item = (u8, u16)>,\n{\n    0\n}\n"
    );
    assert_eq!(
        rust("fn f() -> impl Fn(u8) -> Box<dyn Iterator<Item = u8>> = |x| g(x >= 1)\n"),
        "fn f() -> impl Fn(u8) -> Box<dyn Iterator<Item = u8>> {\n    |x| g(x >= 1)\n}\n"
    );
    assert_eq!(
        rust("pub trait Items:\n    fn items(&self) -> Box<dyn Iterator<Item = u8>>\n    fn one(&self) -> u8 = 1\n"),
        "pub trait Items {\n    fn items(&self) -> Box<dyn Iterator<Item = u8>>;\n    fn one(&self) -> u8 {\n        1\n    }\n}\n"
    );
}

#[test]
fn angle_brackets_inside_signature_groups_and_literals_do_not_hide_the_body() {
    for (source, signature, body) in [
        (
            "fn f() -> [u8; 1 << 2] = [0; 4]\n",
            "fn f() -> [u8; 1 << 2]",
            "[0; 4]",
        ),
        (
            "fn f(x: [u8; 8 >> 1]) -> u8 = x[0]\n",
            "fn f(x: [u8; 8 >> 1]) -> u8",
            "x[0]",
        ),
        (
            "fn f() -> Foo<{ 1 < 2 }> = Foo\n",
            "fn f() -> Foo<{ 1 < 2 }>",
            "Foo",
        ),
        ("fn f(s: S<'<'>) -> u8 = 1\n", "fn f(s: S<'<'>) -> u8", "1"),
    ] {
        assert_eq!(
            rust(source),
            format!("{signature} {{\n    {body}\n}}\n"),
            "{source:?}"
        );
    }
    assert_eq!(
        rust("pub trait A:\n    fn a(&self) -> [u8; 1 << 2] = [0; 4]\n"),
        "pub trait A {\n    fn a(&self) -> [u8; 1 << 2] {\n        [0; 4]\n    }\n}\n"
    );
}

#[test]
fn rust_conversion_keeps_items_rusk_cannot_read_as_rust() {
    for source in [
        "default impl T for Foo {\n    fn f() -> u8 {\n        1\n    }\n}\n",
        "auto trait A {\n    fn f() {}\n}\n\nfn z() {}\n",
        "impl X {\n    default fn f() {\n        g();\n    }\n}\n",
        "mod m {\n    default unsafe impl<T> T for Vec<T> {\n        fn f() {}\n    }\n}\n",
        "fn f() {\n    extern crate x; fn g() {\n        h();\n    }\n    auto trait X {\n        // c\n    }\n    g();\n}\n",
        "unsafe auto trait X {\n}\n\nfn z() {}\n\nmod m {\n    pub unsafe auto trait Y {\n    }\n}\n",
    ] {
        let rusk = rust_to_rusk(source).unwrap();
        assert_eq!(rust(&rusk), source, "{rusk}");
    }
}

#[test]
fn rust_conversion_keeps_the_semicolon_after_item_values_ending_in_braces() {
    for source in [
        "extern crate x; static S: T = T {};\n",
        "impl<X> T for X {\n    default const B: u32 = { 1 };\n    default const C: S = S {\n        a: 1,\n    };\n    default fn f(&self) -> u32 {\n        1\n    }\n}\n",
        "fn f() {\n    extern crate y; let v = T { a: 1 };\n    g(v);\n}\n",
    ] {
        let rusk = rust_to_rusk(source).unwrap();
        assert_eq!(rust(&rusk), source, "{rusk}");
    }
}

#[test]
fn rust_conversion_reads_gen_as_an_identifier() {
    let source = "use crate::{\n    a::b,\n    gen::consts,\n};\n\nfn main(rng: R) {\n    let x: u8 = rng.gen();\n    if x > 0 {\n        println!(\"{x}\");\n    }\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    assert_eq!(rust(&rusk), source, "{rusk}");
}

#[test]
fn closing_lines_continue_after_closing_braces_opened_in_the_head() {
    assert_eq!(
        rust("fn f() -> u8:\n    S { a: g(|x| {\n        x\n    })}.a\n"),
        "fn f() -> u8 {\n    S { a: g(|x| {\n        x\n    })}.a\n}\n"
    );
    assert_eq!(
        rust("fn f(xs: &[u8]) -> Vec[P]:\n    xs.iter().map(|x| P { v: x.min(match x:\n        0 => &1\n        _ => x\n    })}).collect()\n"),
        "fn f(xs: &[u8]) -> Vec<P> {\n    xs.iter().map(|x| P { v: x.min(match x {\n        0 => &1,\n        _ => x,\n    })}).collect()\n}\n"
    );
}

#[test]
fn if_and_match_heads_inside_open_groups_own_their_blocks() {
    assert_eq!(
        rust("fn f(c: bool) -> u8:\n    max(if c:\n        1\n    else:\n        2\n    )\n"),
        "fn f(c: bool) -> u8 {\n    max(if c {\n        1\n    } else {\n        2\n    })\n}\n"
    );
    assert_eq!(
        rust("fn f(a: u8) -> [u8; 1]:\n    [match a:\n        0 => 1\n        _ => 2\n    ]\n"),
        "fn f(a: u8) -> [u8; 1] {\n    [match a {\n        0 => 1,\n        _ => 2,\n    }]\n}\n"
    );
    assert_eq!(
        rust("fn f(c: bool) -> u8:\n    let v = g(1, if c:\n        1\n    else:\n        2\n    )\n    v\n"),
        "fn f(c: bool) -> u8 {\n    let v = g(1, if c {\n        1\n    } else {\n        2\n    });\n    v\n}\n"
    );
}

#[test]
fn macro_groups_keep_if_and_match_heads_as_written() {
    for tokens in [
        "    vec![match a:\n        0 => 1\n        _ => 2\n    ]\n",
        "    vec![1, match a:\n        0 => 1\n        _ => 2\n    ]\n",
        "    max!(if c:\n        1\n    else:\n        2\n    )\n",
    ] {
        assert_eq!(
            rust(&format!("fn f(a: u8, c: bool) -> u8:\n{tokens}")),
            format!("fn f(a: u8, c: bool) -> u8 {{\n{tokens}}}\n")
        );
    }
}

#[test]
fn one_line_else_branches_close_what_their_chain_head_opened() {
    assert_eq!(
        rust("fn f(c: bool) -> u8:\n    max(if c:\n        1\n    else { 2 }, 3) // )\n"),
        "fn f(c: bool) -> u8 {\n    max(if c {\n        1\n    } else { 2 }, 3) // )\n}\n"
    );
    assert_eq!(
        rust("fn f(c: bool) -> u8:\n    let v = g(if c:\n        1\n    else if C then 3 else 2).min(X)\n    v\n"),
        "fn f(c: bool) -> u8 {\n    let v = g(if c {\n        1\n    } else if C { 3 } else { 2 }).min(X);\n    v\n}\n"
    );
}

#[test]
fn closure_bodies_take_if_heads_and_inline_ifs() {
    assert_eq!(
        rust("fn f(xs: &[u8]) -> Vec[u8]:\n    xs.iter().map(|x| if *x > 0:\n        1\n    else { 2 }).collect()\n"),
        "fn f(xs: &[u8]) -> Vec<u8> {\n    xs.iter().map(|x| if *x > 0 {\n        1\n    } else { 2 }).collect()\n}\n"
    );
    assert_eq!(
        rust("fn f(xs: &[u8]) -> Vec[u8]:\n    xs.iter().map(|X| if X > &0 then 1 else 2).collect()\n"),
        "fn f(xs: &[u8]) -> Vec<u8> {\n    xs.iter().map(|X| if X > &0 { 1 } else { 2 }).collect()\n}\n"
    );
}

#[test]
fn equals_signs_in_header_comments_do_not_start_a_body() {
    assert_eq!(
        rust("fn f[T]()\nwhere\n    // T: A<B, Error = Error>,\n    T: Copy:\n    g()\n"),
        "fn f<T>()\nwhere\n    // T: A<B, Error = Error>,\n    T: Copy,\n{\n    g();\n}\n"
    );
    assert_eq!(
        rust("fn f() -> u8 /* = 2 */ = 1\n"),
        "fn f() -> u8 /* = 2 */ {\n    1\n}\n"
    );
}

#[test]
fn where_clauses_keep_one_comma_before_a_trailing_block_comment() {
    for source in [
        "fn f<T>()\nwhere\n    T: Copy, /* c, */\n{\n}\n",
        "fn f<T, U>() -> u8\nwhere\n    T: Copy,\n    U: Clone, /* ) */ /* d */\n{\n    1\n}\n",
    ] {
        let rusk = rust_to_rusk(source).unwrap();
        assert_eq!(rust(&rusk), source, "{rusk}");
    }
}

#[test]
fn rust_conversion_keeps_unbound_receivers_values() {
    let source = "fn f(point: P) -> u8 {\n    x.y = items.a(|d| d) + point.x;\n    x.y\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    assert_eq!(
        rust(&rusk),
        "fn f(point: P) -> u8 {\n    (x).y = (items).a(|d| d) + point.x;\n    (x).y\n}\n",
        "{rusk}"
    );
    // A bound receiver beside a path ending in its name, as in `foo_x::y`, stays as written; an
    // unbound raw identifier is parenthesized whole; inside a macro body, where parameters are
    // not bindings, only the misread receivers change, not the closure's `state`.
    for (source, expected) in [
        (
            "fn f(x: S) -> u8 {\n    x.y + foo_x::y() + self::x::y()\n}\n",
            "fn f(x: S) -> u8 {\n    x.y + foo_x::y() + self::x::y()\n}\n",
        ),
        (
            "fn f(x: S) -> u8 {\n    g(x.y, \"x::y\") + x::y(x) // x::y\n}\n",
            "fn f(x: S) -> u8 {\n    g(x.y, \"x::y\") + x::y(x) // x::y\n}\n",
        ),
        (
            "fn f() -> usize {\n    r#type.len()\n}\n",
            "fn f() -> usize {\n    (r#type).len()\n}\n",
        ),
        (
            "m! {\n    fn f(o: Option<S>) -> u8 {\n        o.map(|state| state.x)\n    }\n\n    fn g(s: S) -> u8 {\n        s.x\n    }\n}\n",
            "m! {\n    fn f(o: Option<S>) -> u8 {\n        (o).map(|state| state.x)\n    }\n\n    fn g(s: S) -> u8 {\n        (s).x\n    }\n}\n",
        ),
    ] {
        let rusk = rust_to_rusk(source).unwrap();
        assert_eq!(rust(&rusk), expected, "{rusk}");
    }
}

#[test]
fn rust_conversion_closes_empty_value_blocks() {
    let source = "fn f() -> u8 {\n    let g = {\n    };\n    h(g)\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    assert_eq!(rust(&rusk), source, "{rusk}");
}

#[test]
fn rust_conversion_keeps_the_brace_of_called_closures() {
    let source = "fn f() -> u8 {\n    h((|| {\n        1\n    })(), 2)\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    assert_eq!(rust(&rusk), source, "{rusk}");
}

#[test]
fn rust_conversion_reads_let_values_with_blocks_in_their_headers() {
    for source in [
        "fn f(x: u8) -> u8 {\n    let v = if match x {\n        0 => true,\n        _ => false,\n    } {\n        1\n    } else {\n        2\n    };\n    v\n}\n",
        "fn f(s: S) -> u8 {\n    let r = match s(|t| {\n        t\n    }) {\n        _ => 1,\n    };\n    r\n}\n",
        // A statement `if let` whose value holds blocks stays a statement.
        "fn f(a: Option<u8>) -> u8 {\n    if let Some(found) = if let Some(x) = a {\n        Some(x)\n    } else {\n        None\n    } {\n        return found;\n    }\n    0\n}\n",
    ] {
        let rusk = rust_to_rusk(source).unwrap();
        assert_eq!(rust(&rusk), source, "{rusk}");
    }
}

#[test]
fn rust_conversion_reads_continued_trait_constants() {
    let source = "trait T {\n    const K: u8 = A\n        | B;\n}\n";
    let rusk = rust_to_rusk(source).unwrap();
    assert_eq!(rust(&rusk), source, "{rusk}");
}

#[test]
fn rust_conversion_reads_patterns_over_several_header_lines() {
    for source in [
        "fn f(s: S) -> u8 {\n    if let Flags::Coff {\n        c, ..\n    } = s.flags\n    {\n        c\n    } else {\n        0\n    }\n}\n",
        "fn f(s: S) -> u8 {\n    let c = if let Flags::Coff {\n        c, ..\n    } = s.flags\n    {\n        c\n    } else {\n        0\n    };\n    c\n}\n",
        "fn f(s: S) -> u8 {\n    if let Outer::A(Inner {\n        b, ..\n    }) = s.flags\n    {\n        return b;\n    }\n    0\n}\n",
        // Tuple and slice patterns close with `)` or `]`; a call compared over lines is no pattern.
        "fn f(s: S) -> u8 {\n    if let Some(\n        b,\n    ) = s.flags\n    {\n        return b;\n    }\n    0\n}\n",
        "fn f(s: S) -> u8 {\n    while let Outer::A([\n        b, ..\n    ]) = s.flags\n    {\n        return b;\n    }\n    0\n}\n",
        "fn f(s: S) -> u8 {\n    let c = if let [\n        b, ..\n    ] = s.flags\n    {\n        b\n    } else {\n        0\n    };\n    c\n}\n",
        "fn f(s: S) -> u8 {\n    if g(\n        s,\n    ) == s.flags {\n        return 1;\n    }\n    0\n}\n",
    ] {
        let rusk = rust_to_rusk(source).unwrap();
        assert_eq!(rust(&rusk), source, "{rusk}");
    }
}

#[test]
fn tail_value_after_a_statement_on_its_line_keeps_no_semicolon() {
    // The statement after a line's last `;` decides its `;`, past `;` in literals and groups.
    for (source, expected) in [
        (
            "fn f(z: S) -> u8:\n    let s = 1; z.y\n",
            "fn f(z: S) -> u8 {\n    let s = 1; z.y\n}\n",
        ),
        (
            "fn f(z: S) -> usize:\n    let s = \"a;\n    b\"; z.len()\n",
            "fn f(z: S) -> usize {\n    let s = \"a;\n    b\"; z.len()\n}\n",
        ),
        (
            "fn f(z: S) -> u8:\n    z.x = g(|a| { h(a); a }); z.y // z;\n",
            "fn f(z: S) -> u8 {\n    z.x = g(|a| { h(a); a }); z.y // z;\n}\n",
        ),
        (
            "fn f(z: S) -> u8:\n    let s = 1; z.y;\n",
            "fn f(z: S) -> u8 {\n    let s = 1; z.y;\n}\n",
        ),
        (
            "fn f(z: S):\n    g(); let s = 1\n",
            "fn f(z: S) {\n    g(); let s = 1;\n}\n",
        ),
        (
            "fn f(z: S) -> u8:\n    let s = 1; z.y\n    s\n",
            "fn f(z: S) -> u8 {\n    let s = 1; z.y;\n    s\n}\n",
        ),
    ] {
        assert_eq!(rust(source), expected, "{source}");
    }
}
