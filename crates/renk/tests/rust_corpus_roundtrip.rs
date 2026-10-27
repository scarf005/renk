use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use proc_macro2::{TokenStream, TokenTree};
use renk::{SourceSyntax, rust_to_renk, rust_to_renk_aggressive, to_rust};

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
    ("nested_blocks", include_str!("rust_corpus/nested_blocks.rs")),
    ("macro_matchers", include_str!("rust_corpus/macro_matchers.rs")),
    ("casts", include_str!("rust_corpus/casts.rs")),
    ("one_line_control", include_str!("rust_corpus/one_line_control.rs")),
    ("exact_text", include_str!("rust_corpus/exact_text.rs")),
];

const RENK_CORPUS: &[(&str, &str)] = &[
    ("edge_cases", include_str!("renk_corpus/edge_cases.rk")),
    (
        "trailing_comments",
        include_str!("renk_corpus/trailing_comments.rk"),
    ),
    ("casts", include_str!("renk_corpus/casts.rk")),
    (
        "one_line_control",
        include_str!("renk_corpus/one_line_control.rk"),
    ),
    (
        "cfg_statements",
        include_str!("renk_corpus/cfg_statements.rk"),
    ),
    (
        "closing_lines",
        include_str!("renk_corpus/closing_lines.rk"),
    ),
    ("headers", include_str!("renk_corpus/headers.rk")),
    ("if_let_then", include_str!("renk_corpus/if_let_then.rk")),
    ("format_quotes", include_str!("renk_corpus/format_quotes.rk")),
    ("struct_literal_heads", include_str!("renk_corpus/struct_literal_heads.rk")),
    ("view_blocks", include_str!("renk_corpus/view_blocks.rk")),
];

#[test]
fn rust_corpus_roundtrips_to_compilable_rust() {
    let out_dir = env::temp_dir().join(format!("renk-rust-corpus-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();

    for (name, rust) in CORPUS {
        compile_rust(&out_dir, name, "original", rust);
        let renk = rust_to_renk(rust).unwrap_or_else(|error| {
            panic!("failed to convert corpus case {name} from Rust to Renk: {error}")
        });
        let roundtripped = to_rust(&renk, SourceSyntax::Renk).unwrap_or_else(|error| {
            panic!("failed to convert corpus case {name} back to Rust: {error}\nRenk:\n{renk}")
        });
        compile_rust(&out_dir, name, "roundtrip", &roundtripped);
        assert_eq!(multiline_literals(&roundtripped), multiline_literals(rust), "{name}");

        let second_renk = rust_to_renk(&roundtripped).unwrap_or_else(|error| {
            panic!("failed to convert roundtripped corpus case {name} to Renk: {error}")
        });
        let second_roundtrip = to_rust(&second_renk, SourceSyntax::Renk).unwrap_or_else(|error| {
            panic!("failed second Rust conversion for corpus case {name}: {error}")
        });
        compile_rust(&out_dir, name, "second-roundtrip", &second_roundtrip);

        let aggressive = rust_to_renk_aggressive(rust).unwrap_or_else(|error| {
            panic!("failed to convert corpus case {name} aggressively: {error}")
        });
        let aggressive_rust = to_rust(&aggressive, SourceSyntax::Renk).unwrap_or_else(|error| {
            panic!("failed to convert aggressive corpus case {name} back to Rust: {error}\nRenk:\n{aggressive}")
        });
        assert_eq!(
            tokens(&aggressive_rust),
            tokens(&roundtripped),
            "{name} changed tokens\nRenk:\n{aggressive}"
        );
    }

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn rust_conversion_emits_idiomatic_renk() {
    let rust = include_str!("rust_corpus/idiomatic.rs");
    assert_eq!(
        rust_to_renk(rust).unwrap(),
        include_str!("rust_corpus/idiomatic.rk")
    );
    assert_eq!(
        rust_to_renk_aggressive(rust).unwrap(),
        include_str!("rust_corpus/idiomatic.aggressive.rk")
    );
}

#[test]
fn rust_conversions_keep_the_text_of_rustfmt_shaped_rust() {
    let rust = include_str!("rust_corpus/exact_text.rs");
    let renk = rust_to_renk(rust).unwrap();
    assert_eq!(to_rust(&renk, SourceSyntax::Renk).unwrap(), rust, "via Renk:\n{renk}");
}

fn tokens(rust: &str) -> String {
    rust.parse::<TokenStream>().unwrap().to_string()
}

#[test]
fn renk_corpus_transpiles_to_compilable_rust() {
    let out_dir = env::temp_dir().join(format!("renk-renk-corpus-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();

    for (name, renk) in RENK_CORPUS {
        let rust = to_rust(renk, SourceSyntax::Renk).unwrap_or_else(|error| {
            panic!("failed to convert Renk corpus case {name} to Rust: {error}")
        });
        compile_rust(&out_dir, name, "renk", &rust);
        for literal in multiline_literals(&rust) {
            assert!(renk.contains(&literal), "{name} changed {literal}");
        }
        let second_renk = rust_to_renk(&rustfmt(&rust)).unwrap_or_else(|error| {
            panic!("failed to convert Renk corpus case {name} back to Renk: {error}")
        });
        let roundtripped = to_rust(&second_renk, SourceSyntax::Renk).unwrap_or_else(|error| {
            panic!("failed second Rust conversion for Renk corpus case {name}: {error}\nRenk:\n{second_renk}")
        });
        compile_rust(&out_dir, name, "renk-roundtrip", &roundtripped);
        assert_eq!(multiline_literals(&roundtripped), multiline_literals(&rust), "{name}");
    }

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn docs_examples_transpile_to_compilable_rust() {
    let out_dir = env::temp_dir().join(format!("renk-docs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let mut pages = vec![docs.join("src"), docs.join("ko/src")];
    let mut examples = 0;
    while let Some(path) = pages.pop() {
        if path.is_dir() {
            pages.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
            continue;
        }
        let page = fs::read_to_string(&path).unwrap();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        // Script examples start with a shebang and cargo-script frontmatter.
        for (index, block) in page.split("```rk\n").skip(1).enumerate() {
            let renk = &block[..block.find("```").unwrap()];
            if renk.starts_with("#!") {
                continue;
            }
            let rust = to_rust(renk, SourceSyntax::Renk)
                .unwrap_or_else(|error| panic!("{name} example {index}: {error}\n{renk}"));
            compile_rust(&out_dir, &format!("{name}-{index}"), "docs", &rust);
            examples += 1;
        }
    }
    assert!(examples > 0);
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
