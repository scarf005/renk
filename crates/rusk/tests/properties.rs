use proptest::prelude::*;
use rusk::{
    FormatOptions, format_source, ruk_to_rust, rusk_to_ruk, rust_to_ruk, rust_to_rusk, transpile,
};

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
        let _ = rusk_to_ruk(&source);
        let _ = ruk_to_rust(&source);
        let _ = rust_to_rusk(&source);
        let _ = rust_to_ruk(&source);
    }

    #[test]
    fn conversions_never_panic_on_arbitrary_text(source in any::<String>()) {
        let _ = transpile(&source);
        let _ = format_source(&source, FormatOptions::default());
        let _ = ruk_to_rust(&source);
        let _ = rust_to_rusk(&source);
        let _ = rust_to_ruk(&source);
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
