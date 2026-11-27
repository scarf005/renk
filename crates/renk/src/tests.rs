use super::*;

#[test]
fn deep_block_argument_chains_convert_on_a_small_stack() {
    let links = (0..2_000)
        .map(|n| format!("        .a(|y| {{\n            y + {n}\n        }})\n"))
        .collect::<String>();
    let rust = format!("fn main() {{\n    x\n{links}        .z();\n}}\n");
    // WebAssembly runs on a 1 MiB stack without the conversion thread; a recursive walk or
    // drop of the tree overflows it.
    std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || with_value_scopes(|| convert_rust_lines(&rust, false)).unwrap())
        .unwrap()
        .join()
        .unwrap();
}

fn rust(source: &str) -> String {
    transpile(source).unwrap().rust
}

#[test]
fn whole_file_checks_stay_as_many_for_four_times_the_functions() {
    // Each function wraps an `assert!` over six lines. Inlining its format arguments puts it
    // on one line, so the Rust line count changes.
    let checks = |functions: usize, aggressive: bool| {
        let rust = (0..functions)
            .map(|n| {
                format!(
                    "fn check_{n}(cmd: &Command, value: usize) {{\n    assert!(\n        \
                     value > {n},\n        \"Command {{}}: value {{}} is too small\",\n        \
                     cmd.get_name(),\n        value,\n    );\n    let total = value + {n};\n    \
                     println!(\"{{}}\", total);\n}}\n\n"
                )
            })
            .collect::<String>();
        FILE_TRANSPILES.set(0);
        with_value_scopes(|| convert_rust_lines(&rust, aggressive)).unwrap();
        FILE_TRANSPILES.get()
    };
    for aggressive in [false, true] {
        assert_eq!(
            checks(10, aggressive),
            checks(40, aggressive),
            "{aggressive}"
        );
    }
}

#[test]
fn item_checks_give_the_renk_of_whole_file_checks() {
    // Each item wraps an `assert!`, so its check takes more than one round. The items read a
    // function of another item as a value, hold a string with a blank line and a line that
    // starts with `fn`, and have comments, attributes, a macro and a module between them.
    let wrapped = "    assert!(\n        a > 1,\n        \"{}\",\n        a,\n    );\n";
    let neighbours = format!(
        "fn parse_args(a: usize) -> usize {{\n{wrapped}    a\n}}\n\n\
         fn run(a: usize) -> usize {{\n{wrapped}    parse_args.call((a,))\n}}\n\n\
         // A comment.\n\n#[inline]\nfn help(a: usize) -> &'static str {{\n{wrapped}    \"a\n\n\
         fn b\n\n#[c]\"\n}}\n\n\
         macro_rules! check {{\n    ($a:expr) => {{\n{wrapped}    }};\n}}\n\n\
         mod inner {{\n    fn f(a: usize) {{\n    {wrapped}    }}\n\n    fn g() {{}}\n}}\n"
    );
    let corpus = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rust_corpus"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| {
            (
                path.display().to_string(),
                std::fs::read_to_string(path).unwrap(),
            )
        });
    for (name, rust) in corpus.chain([("neighbours".to_string(), neighbours)]) {
        for aggressive in [false, true] {
            let convert = |whole_file| {
                on_large_stack(|| {
                    WHOLE_FILE_CHECKS.set(whole_file);
                    with_value_scopes(|| convert_rust_lines(&rust, aggressive))
                })
            };
            assert_eq!(convert(false), convert(true), "{name} {aggressive}");
        }
    }
}

#[test]
fn lowers_struct_impl_and_inline_functions() {
    let source = r#"
#[derive(Debug, Clone)]
pub struct User:
    pub id: u64
    pub name: String

impl User:
    pub fn new(id: u64, name: String) -> Self = Self{ id, name }

    pub fn display_name(&self) -> &str = &self.name
"#;

    assert_eq!(
        rust(source),
        r#"#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
}

impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self{ id, name }
    }

    pub fn display_name(&self) -> &str {
        &self.name
    }
}
"#
    );
}

#[test]
fn lowers_explicit_semicolons_and_match_arms() {
    let source = r#"
fn parse(line: &str) -> Result[i32, String]:
    match line.parse[i32]():
        Ok(value):
            println!("{}", value);
            Ok(value)
        Err(error) => Err(error.to_string())
"#;

    assert_eq!(
        rust(source),
        r#"fn parse(line: &str) -> Result<i32, String> {
    match line.parse::<i32>() {
        Ok(value) => {
            println!("{}", value);
            Ok(value)
        }
        Err(error) => Err(error.to_string()),
    }
}
"#
    );
}

#[test]
fn lowers_generic_impl_and_inline_semicolon_match_arm() {
    let source = r#"
pub struct Boxed[T]:
    pub value: T

impl[T] Boxed[T]:
    pub fn new(value: T) -> Self = Self{ value }

fn log(value: Result[i32, String]):
    match value:
        Ok(number) => println!("{}", number);
        Err(error) => error
"#;

    assert_eq!(
        rust(source),
        r#"pub struct Boxed<T> {
    pub value: T,
}

impl<T> Boxed<T> {
    pub fn new(value: T) -> Self {
        Self{ value }
    }
}

fn log(value: Result<i32, String>) {
    match value {
        Ok(number) => {
            println!("{}", number);
        }
        Err(error) => error,
    };
}
"#
    );
}

#[test]
fn preserves_value_dots_and_lowers_path_dots() {
    let source = r#"
fn test(iter: Iter):
    Foo.new();
    Foo::bar();
    iter.collect::<Vec<_>>();
    std.io.read();
"#;

    assert_eq!(
        rust(source),
        r#"fn test(iter: Iter) {
    Foo::new();
    Foo::bar();
    iter.collect::<Vec<_>>();
    std::io::read();
}
"#
    );
}

#[test]
fn keeps_index_expressions_numeric() {
    let source = r#"
fn example(xs: &[i32], index: usize) -> i32:
    let a = [Foo]
    let b = [3]
    let c = xs[3]
    let d = xs[index]
    let repeated = [d; 2]
    c + repeated[1]
"#;

    assert_eq!(
        rust(source),
        r#"fn example(xs: &[i32], index: usize) -> i32 {
    let a = [Foo];
    let b = [3];
    let c = xs[3];
    let d = xs[index];
    let repeated = [d; 2];
    c + repeated[1]
}
"#
    );
}

#[test]
fn lowers_if_then_else_expression() {
    let source = r#"
fn clamp(value: i32, min: i32, max: i32) -> i32:
    if value < min then min else if value > max then max else value
"#;

    assert_eq!(
        rust(source),
        r#"fn clamp(value: i32, min: i32, max: i32) -> i32 {
    if value < min { min } else if value > max { max } else { value }
}
"#
    );

    assert_eq!(
        rust("fn choose(a: bool, b: bool) -> i32 = if a then if b then 1 else 2 else 3\n"),
        r#"fn choose(a: bool, b: bool) -> i32 {
    if a { if b { 1 } else { 2 } } else { 3 }
}
"#
    );
}

#[test]
fn infers_unit_function_tail_statement() {
    let source = r#"
pub fn main():
    println!("hello")

pub fn id(value: i32) -> i32:
    value
"#;

    assert_eq!(
        rust(source),
        r#"pub fn main() {
    println!("hello");
}

pub fn id(value: i32) -> i32 {
    value
}
"#
    );
}

#[test]
fn preserves_comment_indentation() {
    let source = r#"
fn demo():
    // A trailing semicolon explicitly discards the value.
    println!("hi");
"#;

    assert_eq!(
        rust(source),
        r#"fn demo() {
    // A trailing semicolon explicitly discards the value.
    println!("hi");
}
"#
    );
}

#[test]
fn explicit_semicolon_discards_tail_expression() {
    let source = r#"
fn bump(value: i32) -> i32 = value + 1

fn discarded_units(values: &[i32]) -> Vec[()] = values
    .iter()
    .map(|value| {
        bump(*value);
    })
    .collect[Vec[()]]()
"#;

    assert_eq!(
        rust(source),
        r#"fn bump(value: i32) -> i32 {
    value + 1
}

fn discarded_units(values: &[i32]) -> Vec<()> {
    values
        .iter()
        .map(|value| {
            bump(*value);
        })
        .collect::<Vec<()>>()
}
"#
    );
}

#[test]
fn explicit_semicolon_discards_multiline_block_expression() {
    let source = r#"
struct Foo:
    x: i32

fn demo(value: Option[i32]):
    match value:;
        Some(number) => number
        None => 0
    Foo:;
        x = 1
"#;

    assert_eq!(
        rust(source),
        r#"struct Foo {
    x: i32,
}

fn demo(value: Option<i32>) {
    match value {
        Some(number) => number,
        None => 0,
    };
    Foo {
        x: 1,
    };
}
"#
    );
}

#[test]
fn lowers_multiline_closures() {
    let source = r#"
pub fn normalized_names(lines: Vec[String]) -> Vec[String] = lines
    .into_iter()
    .filter_map(|line| {
        let trimmed = line.trim()
        if trimmed.is_empty() then None else Some(trimmed.to_ascii_lowercase())
    })
    .collect[Vec[String]]()
"#;

    assert_eq!(
        rust(source),
        r#"pub fn normalized_names(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_ascii_lowercase()) }
        })
        .collect::<Vec<String>>()
}
"#
    );
}

#[test]
fn lowers_macro_rules_items() {
    let source = r#"
macro_rules! make_message:
    ($name:expr) => format!("hello {}", $name)

pub fn demo() -> String:
    make_message!("Ada")
"#;

    assert_eq!(
        rust(source),
        r#"macro_rules! make_message {
    ($name:expr) => { format!("hello {}", $name) };
}

pub fn demo() -> String {
    make_message!("Ada")
}
"#
    );
}

#[test]
fn lowers_inline_expressions_in_format_macros() {
    let source = r#"
struct User:
    name: String

impl User:
    fn display_name(&self) -> &str = &self.name

fn render(user: &User, suffix: &str) -> String:
    format!("{} {user.display_name()} {user.display_name():?}", suffix)

fn show(user: &User):
    println!("{user.display_name()}")
    println!("{{user}} {user.display_name()}")
"#;

    assert_eq!(
        rust(source),
        r#"struct User {
    name: String,
}

impl User {
    fn display_name(&self) -> &str {
        &self.name
    }
}

fn render(user: &User, suffix: &str) -> String {
    format!("{} {1} {2:?}", suffix, user.display_name(), user.display_name())
}

fn show(user: &User) {
    println!("{}", user.display_name());
    println!("{{user}} {}", user.display_name());
}
"#
    );
}

#[test]
fn lowers_inline_expression_with_string_literal_argument() {
    let source = r#"
fn swap(pair: (String, usize)) -> (usize, String) = (pair.1, pair.0)

fn main():
    println!("{swap(("items".to_string(), 3)):?}")
"#;

    assert_eq!(
        rust(source),
        r#"fn swap(pair: (String, usize)) -> (usize, String) {
    (pair.1, pair.0)
}

fn main() {
    println!("{:?}", swap(("items".to_string(), 3)));
}
"#
    );
}

#[test]
fn converts_rust_to_renk() {
    let source = r#"
#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
}

impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }

    pub fn display_name(&self) -> &str {
        &self.name
    }
}

pub fn main() {
    let user = User::new(1, "Ada".to_string());
    println!("{}", user.display_name());
}
"#;

    assert_eq!(
        rust_to_renk(source).unwrap(),
        r#"#[derive(Debug, Clone)]
pub struct User:
    pub id: u64
    pub name: String

impl User:
    pub fn new(id: u64, name: String) -> Self = Self { id, name }

    pub fn display_name(&self) -> &str = &self.name

pub fn main():
    let user = User.new(1, "Ada".to_string())
    println!("{user.display_name()}")
"#
    );
}

#[test]
fn preserves_inline_rust_blocks_during_rust_to_renk() {
    assert_eq!(
        rust_to_renk("fn main() { println!(\"hi\"); }\n").unwrap(),
        "fn main() { println!(\"hi\"); }\n"
    );
}

#[test]
fn transpiles_multiline_rust_function_blocks() {
    assert_eq!(
        rust("fn main() {\n    println!(\"hi\");\n}\n"),
        "fn main() {\n    println!(\"hi\");\n}\n"
    );
}

#[test]
fn converts_rust_tail_discard_semicolons() {
    let source = r#"
fn bump(value: i32) -> i32 {
    value + 1
}

fn discarded_units(values: &[i32]) -> Vec<()> {
    values
        .iter()
        .map(|value| {
            bump(*value);
        })
        .collect::<Vec<()>>()
}
"#;

    let renk = rust_to_renk(source).unwrap();
    assert!(renk.contains("bump(*value);"));
    assert!(transpile(&renk).unwrap().rust.contains("bump(*value);"));
}

#[test]
fn converts_rust_generics_turbofish_and_inline_if() {
    let source = r#"
pub struct Boxed<T> {
    pub value: T,
}

impl<T> Boxed<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

pub fn parse(raw: &str) -> Option<(i32, i32)> {
    let (left, right) = raw.split_once(',')?;
    let left = left.trim().parse::<i32>().ok()?;
    let right = right.trim().parse::<i32>().ok()?;
    if left < right { Some((left, right)) } else { None }
}
"#;

    assert_eq!(
        rust_to_renk(source).unwrap(),
        r#"pub struct Boxed[T]:
    pub value: T

impl[T] Boxed[T]:
    pub fn new(value: T) -> Self = Self { value }

pub fn parse(raw: &str) -> Option[(i32, i32)]:
    let (left, right) = raw.split_once(',')?
    let left = left.trim().parse[i32]().ok()?
    let right = right.trim().parse[i32]().ok()?
    if left < right then Some((left, right)) else None
"#
    );
}

#[test]
fn converts_rust_multiline_closure_chains() {
    let source = r#"
pub fn normalized_names(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_ascii_lowercase()) }
        })
        .collect::<Vec<String>>()
}
"#;

    let renk = rust_to_renk(source).unwrap();
    assert!(renk.contains(".filter_map(|line| {"));
    assert!(!renk.contains("        )\n"));
    assert!(
        transpile(&renk)
            .unwrap()
            .rust
            .contains(".collect::<Vec<String>>()")
    );
}

#[test]
fn formats_without_reflowing_line_break_style() {
    let inline = "pub fn even_squares(values: Vec[i32]) -> Vec[i32]:\n    values.into_iter().filter(|value| value % 2 == 0).map(|value| value * value).collect[Vec[i32]]()\n";
    let broken = "pub fn even_squares(values: Vec[i32]) -> Vec[i32] = values\n    .into_iter()\n    .filter(|value| value % 2 == 0)\n    .map(|value| value * value)\n    .collect[Vec[i32]]()\n";

    assert_eq!(format_source(inline).unwrap(), inline);
    assert_eq!(format_source(broken).unwrap(), broken);
}

#[test]
fn examples_transpile_to_compilable_rust() {
    use std::{env, fs, path::PathBuf, process::Command};

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let examples_dir = manifest_dir.join("../../examples");
    let mut examples = fs::read_dir(&examples_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension == "rk")
        })
        .collect::<Vec<_>>();
    examples.sort();
    assert!(!examples.is_empty());

    let out_dir = env::temp_dir().join(format!("renk-example-tests-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();

    for example in examples {
        let source = fs::read_to_string(&example).unwrap();
        let rust = transpile(&source)
            .unwrap_or_else(|error| panic!("failed to transpile {}: {error}", example.display()))
            .rust;
        if example
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem == "async_functions")
        {
            assert!(rust.contains("trpl::block_on"));
            continue;
        }
        let rust_path = out_dir.join(format!(
            "{}.rs",
            example
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .replace('-', "_")
        ));
        fs::write(&rust_path, rust).unwrap();

        let status = Command::new("rustc")
            .arg("--edition=2024")
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg("-o")
            .arg(out_dir.join("out.rmeta"))
            .arg(&rust_path)
            .status()
            .unwrap_or_else(|error| {
                panic!("failed to run rustc for {}: {error}", example.display())
            });
        assert!(status.success(), "rustc failed for {}", example.display());
    }

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn rejects_unsupported_indented_expression_blocks() {
    let error = transpile("fn main():\n    println!(\"first\")\n        println!(\"nested\")\n")
        .unwrap_err();
    assert_eq!(error.line, 2);
    assert_eq!(error.message, "indented blocks require `:`");
}

#[test]
fn allows_inline_function_body_with_operator_continuation() {
    assert_eq!(
        transpile("fn id(value: i32) -> i32 = value\n    + 1\n")
            .unwrap()
            .rust,
        "fn id(value: i32) -> i32 {\n    value\n        + 1\n}\n"
    );
}

#[test]
fn rejects_inline_match_arm_with_children() {
    let error = transpile("fn id(value: i32) -> i32:\n    match value:\n        0 => 1\n            println!(\"ignored\")\n        _ => value\n")
        .unwrap_err();
    assert_eq!(error.line, 3);
    assert_eq!(
        error.message,
        "match arms with inline bodies cannot own indented blocks"
    );
}

#[test]
fn format_rejects_unsupported_blocks() {
    let error =
        format_source("fn main():\n    println!(\"first\")\n        println!(\"nested\")\n")
            .unwrap_err();
    assert_eq!(error.line, 2);
}

#[test]
fn canonical_format_macros_skip_a_quote_in_a_comment_or_a_lifetime() {
    // A `'` that opens no char literal must not hide the code after it up to the next `'`.
    let call = "fn f(s: &S) {\n    format!(\"{}\", s.token);\n}\n";
    let inlined = canonical_format_macros(call);
    assert!(inlined.contains("format!(\"{s.token}\")"), "{inlined}");
    for before in [
        "// the literal's span\n",
        "/* it's */\n",
        "fn g(x: &'static str) {}\n",
        "const Q: char = '\\'';\n",
        "const R: &str = r#\"a\"b\"#;\n",
    ] {
        let text = format!("{before}{call}");
        assert_eq!(canonical_format_macros(&text), format!("{before}{inlined}"));
    }
}

#[test]
fn emits_source_map_json() {
    let output = transpile("fn id(x: i32) -> i32 = x\n").unwrap();
    assert_eq!(output.source_map[0].source_line, 1);
    assert_eq!(output.source_tree[0].kind, "function");
    let json = source_map_json(&output);
    assert!(json.contains("\"version\": 2"));
    assert!(json.contains("\"kind\": \"function\""));
    assert!(json.contains("\"generated\": { \"start_line\": 1, \"end_line\": 3 }"));
}
