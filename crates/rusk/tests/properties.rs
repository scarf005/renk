use proptest::prelude::*;
use rusk::{FormatOptions, format_source, rust_to_rusk, rust_to_rusk_aggressive, transpile};

const FRAGMENTS: &[&str] = &[
    "fn ",
    "pub ",
    "main",
    "f",
    "x",
    "X",
    "Self",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "<",
    ">",
    ":",
    "::",
    ".",
    ",",
    ";",
    "=",
    "==",
    "=>",
    "->",
    "|",
    "||",
    "&",
    "?",
    "!",
    "#[",
    "#![",
    "'a",
    "'",
    "\"",
    "r#\"",
    "\"#",
    "\\",
    "//",
    "/*",
    "*/",
    "0",
    "1.5",
    "if ",
    "then ",
    "else",
    "else if ",
    "match ",
    "let ",
    "loop",
    "while ",
    "for ",
    " in ",
    "return",
    "struct ",
    "enum ",
    "impl ",
    "trait ",
    "mod ",
    "use ",
    "macro_rules! ",
    "$x:expr",
    "$(",
    ")*",
    "async ",
    "move ",
    "unsafe ",
    "where ",
    "dyn ",
    "Vec",
    "Option",
    "println!",
    "format!",
    "é",
    "목록",
    " ",
    "    ",
    "\n",
    "\n    ",
    "\n        ",
    "\r\n",
    "\t",
];

fn rusk_like() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(FRAGMENTS), 0..80).prop_map(|parts| parts.concat())
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|cases| cases.parse().ok())
            .unwrap_or(2048),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn conversions_never_panic_on_token_soup(source in rusk_like()) {
        let _ = transpile(&source);
        let _ = format_source(&source, FormatOptions::default());
        let _ = rust_to_rusk(&source);
        let _ = rust_to_rusk_aggressive(&source);
    }

    #[test]
    fn conversions_never_panic_on_arbitrary_text(source in any::<String>()) {
        let _ = transpile(&source);
        let _ = format_source(&source, FormatOptions::default());
        let _ = rust_to_rusk(&source);
    }

    #[test]
    fn transpile_errors_name_an_existing_line(source in rusk_like()) {
        if let Err(error) = transpile(&source) {
            let lines = source.lines().count().max(1);
            prop_assert!((1..=lines + 1).contains(&error.line), "{error} for {source:?}");
        }
    }

    #[test]
    fn transpile_is_deterministic(source in rusk_like()) {
        prop_assert_eq!(transpile(&source), transpile(&source));
    }

    #[test]
    fn accepted_sources_keep_their_rust_after_formatting(source in rusk_like()) {
        if let Ok(output) = transpile(&source) {
            let formatted = format_source(&source, FormatOptions::default());
            prop_assert!(formatted.is_ok(), "fmt rejected a transpilable source {source:?}: {formatted:?}");
            let formatted = formatted.unwrap();
            prop_assert_eq!(&format_source(&formatted, FormatOptions::default()).unwrap(), &formatted);
            prop_assert_eq!(transpile(&formatted).map(|o| o.rust), Ok(output.rust));
        }
    }
}

#[test]
fn long_groups_convert_in_linear_time() {
    let convert = |rows: usize| {
        let rows = (0..rows)
            .map(|n| format!("    ({n}, {n}),\n"))
            .collect::<String>();
        let rust = format!("pub static TABLE: &[(u32, u32)] = &[\n{rows}];\n");
        let start = std::time::Instant::now();
        transpile(&rust_to_rusk(&rust).unwrap()).unwrap();
        start.elapsed()
    };
    let (short, long) = (convert(5_000), convert(20_000));
    // Four times the rows take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn long_block_argument_chains_convert_in_linear_time() {
    let convert = |links: usize| {
        let links = (0..links)
            .map(|n| format!("        .a(|y| {{\n            y + {n}\n        }})\n"))
            .collect::<String>();
        let rust = format!("fn main() {{\n    x\n{links}        .z();\n}}\n");
        let start = std::time::Instant::now();
        rust_to_rusk(&rust).unwrap();
        start.elapsed()
    };
    let (short, long) = (convert(1_000), convert(4_000));
    // Four times the links take about four times as long, not sixteen.
    assert!(long < short * 8, "{short:?} {long:?}");
}

#[test]
fn deep_closures_convert_in_time_linear_in_their_text() {
    let convert = |depth: usize| {
        let closures = (0..depth).fold("1".to_string(), |body, level| {
            let indent = "    ".repeat(depth - level);
            format!("g(|| {{\n{indent}    {body}\n{indent}}})")
        });
        let rust = format!("fn f() -> u8 {{\n    {closures}\n}}\n");
        let start = std::time::Instant::now();
        rust_to_rusk(&rust).unwrap();
        start.elapsed()
    };
    // Twice the depth is four times the indented text, so it should take about four times as
    // long, not eight.
    let (short, long) = (convert(200), convert(400));
    assert!(long < short * 5, "{short:?} {long:?}");
}

#[test]
fn deep_nesting_converts_on_a_small_caller_stack() {
    let parens = 5_000;
    let parens = format!(
        "fn f() -> u8 {{\n    {}1{}\n}}\n",
        "(".repeat(parens),
        ")".repeat(parens)
    );
    let closures = (0..300).fold("1".to_string(), |body, depth| {
        let indent = "    ".repeat(300 - depth);
        format!("g(|| {{\n{indent}    {body}\n{indent}}})")
    });
    let closures = format!("fn f() -> u8 {{\n    {closures}\n}}\n");
    // The parsers recurse once per level, past the stack of the calling thread.
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            for rust in [parens, closures] {
                let rusk = rust_to_rusk(&rust).unwrap();
                assert_eq!(transpile(&rusk).unwrap().rust, rust);
                format_source(&rusk, FormatOptions::default()).unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
