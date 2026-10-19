use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use proc_macro2::{TokenStream, TokenTree};
use rusk::{
    SourceSyntax, ruk_to_rust, rust_to_ruk, rust_to_rusk, rust_to_rusk_aggressive, to_rust,
};

const CORPUS: &[(&str, &str)] = &[
    ("basic_items", include_str!("rust_corpus/basic_items.rs")),
    ("control_flow", include_str!("rust_corpus/control_flow.rs")),
    (
        "enums_matches",
        include_str!("rust_corpus/enums_matches.rs"),
    ),
    (
        "generics_traits",
        include_str!("rust_corpus/generics_traits.rs"),
    ),
    (
        "macros_closures",
        include_str!("rust_corpus/macros_closures.rs"),
    ),
    ("wrapped_lines", include_str!("rust_corpus/wrapped_lines.rs")),
    ("edge_cases", include_str!("rust_corpus/edge_cases.rs")),
    ("macro_rules", include_str!("rust_corpus/macro_rules.rs")),
    ("value_blocks", include_str!("rust_corpus/value_blocks.rs")),
    ("idiomatic", include_str!("rust_corpus/idiomatic.rs")),
    ("format_args", include_str!("rust_corpus/format_args.rs")),
    ("where_clauses", include_str!("rust_corpus/where_clauses.rs")),
    ("long_conditions", include_str!("rust_corpus/long_conditions.rs")),
    ("match_arm_blocks", include_str!("rust_corpus/match_arm_blocks.rs")),
    ("bare_blocks", include_str!("rust_corpus/bare_blocks.rs")),
    ("wrapped_groups", include_str!("rust_corpus/wrapped_groups.rs")),
    ("item_tails", include_str!("rust_corpus/item_tails.rs")),
    ("block_comments", include_str!("rust_corpus/block_comments.rs")),
    ("one_line_items", include_str!("rust_corpus/one_line_items.rs")),
    ("split_continuations", include_str!("rust_corpus/split_continuations.rs")),
    ("real_crate_forms", include_str!("rust_corpus/real_crate_forms.rs")),
];

const RUSK_CORPUS: &[(&str, &str)] = &[
    ("edge_cases", include_str!("rusk_corpus/edge_cases.rsk")),
    (
        "trailing_comments",
        include_str!("rusk_corpus/trailing_comments.rsk"),
    ),
];

#[test]
fn rust_corpus_roundtrips_to_compilable_rust() {
    let out_dir = env::temp_dir().join(format!("rusk-rust-corpus-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();

    for (name, rust) in CORPUS {
        compile_rust(&out_dir, name, "original", rust);
        let rusk = rust_to_rusk(rust).unwrap_or_else(|error| {
            panic!("failed to convert corpus case {name} from Rust to Rusk: {error}")
        });
        let roundtripped = to_rust(&rusk, SourceSyntax::Rusk).unwrap_or_else(|error| {
            panic!("failed to convert corpus case {name} back to Rust: {error}\nRusk:\n{rusk}")
        });
        compile_rust(&out_dir, name, "roundtrip", &roundtripped);
        assert_eq!(multiline_literals(&roundtripped), multiline_literals(rust), "{name}");

        let second_rusk = rust_to_rusk(&roundtripped).unwrap_or_else(|error| {
            panic!("failed to convert roundtripped corpus case {name} to Rusk: {error}")
        });
        let second_roundtrip = to_rust(&second_rusk, SourceSyntax::Rusk).unwrap_or_else(|error| {
            panic!("failed second Rust conversion for corpus case {name}: {error}")
        });
        compile_rust(&out_dir, name, "second-roundtrip", &second_roundtrip);

        let aggressive = rust_to_rusk_aggressive(rust).unwrap_or_else(|error| {
            panic!("failed to convert corpus case {name} aggressively: {error}")
        });
        let aggressive_rust = to_rust(&aggressive, SourceSyntax::Rusk).unwrap_or_else(|error| {
            panic!("failed to convert aggressive corpus case {name} back to Rust: {error}\nRusk:\n{aggressive}")
        });
        assert_eq!(
            tokens(&aggressive_rust),
            tokens(&roundtripped),
            "{name} changed tokens\nRusk:\n{aggressive}"
        );

        let ruk = rust_to_ruk(rust).unwrap_or_else(|error| {
            panic!("failed to convert corpus case {name} from Rust to Ruk: {error}")
        });
        let ruk_rust = ruk_to_rust(&ruk).unwrap_or_else(|error| {
            panic!("failed to convert corpus case {name} back from Ruk: {error}\nRuk:\n{ruk}")
        });
        compile_rust(&out_dir, name, "ruk-roundtrip", &ruk_rust);
        assert_eq!(multiline_literals(&ruk_rust), multiline_literals(rust), "{name} via Ruk");
    }

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn rust_conversion_emits_idiomatic_rusk() {
    let rust = include_str!("rust_corpus/idiomatic.rs");
    assert_eq!(
        rust_to_rusk(rust).unwrap(),
        include_str!("rust_corpus/idiomatic.rsk")
    );
    assert_eq!(
        rust_to_rusk_aggressive(rust).unwrap(),
        include_str!("rust_corpus/idiomatic.aggressive.rsk")
    );
}

fn tokens(rust: &str) -> String {
    rust.parse::<TokenStream>().unwrap().to_string()
}

#[test]
fn rusk_corpus_transpiles_to_compilable_rust() {
    let out_dir = env::temp_dir().join(format!("rusk-rusk-corpus-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();

    for (name, rusk) in RUSK_CORPUS {
        let rust = to_rust(rusk, SourceSyntax::Rusk).unwrap_or_else(|error| {
            panic!("failed to convert Rusk corpus case {name} to Rust: {error}")
        });
        compile_rust(&out_dir, name, "rusk", &rust);
        for literal in multiline_literals(&rust) {
            assert!(rusk.contains(&literal), "{name} changed {literal}");
        }
        let second_rusk = rust_to_rusk(&rustfmt(&rust)).unwrap_or_else(|error| {
            panic!("failed to convert Rusk corpus case {name} back to Rusk: {error}")
        });
        let roundtripped = to_rust(&second_rusk, SourceSyntax::Rusk).unwrap_or_else(|error| {
            panic!("failed second Rust conversion for Rusk corpus case {name}: {error}\nRusk:\n{second_rusk}")
        });
        compile_rust(&out_dir, name, "rusk-roundtrip", &roundtripped);
        assert_eq!(multiline_literals(&roundtripped), multiline_literals(&rust), "{name}");
    }

    let _ = fs::remove_dir_all(out_dir);
}

/// Multi-line string literals, which every conversion must keep byte for byte.
fn multiline_literals(source: &str) -> Vec<String> {
    fn collect(tokens: TokenStream, literals: &mut Vec<String>) {
        for token in tokens {
            match token {
                TokenTree::Group(group) => collect(group.stream(), literals),
                TokenTree::Literal(literal) => {
                    let text = literal.to_string();
                    if text.contains('\n') {
                        literals.push(text);
                    }
                }
                _ => {}
            }
        }
    }
    let mut literals = Vec::new();
    collect(source.parse().unwrap(), &mut literals);
    literals.sort();
    literals
}

fn compile_rust(out_dir: &Path, name: &str, phase: &str, source: &str) {
    let rust_path = out_dir.join(format!("{name}-{phase}.rs"));
    fs::write(&rust_path, source).unwrap();
    let output = Command::new("rustc")
        .arg("--edition=2024")
        .arg("--crate-type=lib")
        .arg("--emit=metadata")
        .arg("-o")
        .arg(out_dir.join(format!("{name}-{phase}.rmeta")))
        .arg(&rust_path)
        .output()
        .unwrap_or_else(|error| panic!("failed to run rustc for corpus case {name}: {error}"));
    assert!(
        output.status.success(),
        "rustc failed for corpus case {name} during {phase}\nstdout:\n{}\nstderr:\n{}\nsource:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        source
    );
}

fn rustfmt(source: &str) -> String {
    let mut child = Command::new("rustfmt")
        .args(["--edition=2024", "--emit=stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to run rustfmt: {error}"));
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "rustfmt failed\nstderr:\n{}\nsource:\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
