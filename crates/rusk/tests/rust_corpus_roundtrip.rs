use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use rusk::{SourceSyntax, rust_to_rusk, to_rust};

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
];

const RUSK_CORPUS: &[(&str, &str)] = &[("edge_cases", include_str!("rusk_corpus/edge_cases.rsk"))];

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

        let second_rusk = rust_to_rusk(&roundtripped).unwrap_or_else(|error| {
            panic!("failed to convert roundtripped corpus case {name} to Rusk: {error}")
        });
        let second_roundtrip = to_rust(&second_rusk, SourceSyntax::Rusk).unwrap_or_else(|error| {
            panic!("failed second Rust conversion for corpus case {name}: {error}")
        });
        compile_rust(&out_dir, name, "second-roundtrip", &second_roundtrip);
    }

    let _ = fs::remove_dir_all(out_dir);
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
        let second_rusk = rust_to_rusk(&rustfmt(&rust)).unwrap_or_else(|error| {
            panic!("failed to convert Rusk corpus case {name} back to Rusk: {error}")
        });
        let roundtripped = to_rust(&second_rusk, SourceSyntax::Rusk).unwrap_or_else(|error| {
            panic!("failed second Rust conversion for Rusk corpus case {name}: {error}\nRusk:\n{second_rusk}")
        });
        compile_rust(&out_dir, name, "rusk-roundtrip", &roundtripped);
    }

    let _ = fs::remove_dir_all(out_dir);
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
