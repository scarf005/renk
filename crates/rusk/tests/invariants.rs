use std::{
    fs,
    path::{Path, PathBuf},
};

use rusk::{FormatOptions, SourceMapNode, format_source, rust_to_rusk, transpile};

fn sources(dirs: &[&str], extension: &str) -> Vec<(PathBuf, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = dirs
        .iter()
        .flat_map(|dir| fs::read_dir(root.join(dir)).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty(), "no .{extension} files in {dirs:?}");
    paths
        .into_iter()
        .map(|path| {
            let source = fs::read_to_string(&path).unwrap();
            (path, source)
        })
        .collect()
}

fn rusk_sources() -> Vec<(PathBuf, String)> {
    sources(&["../../examples", "tests/rusk_corpus"], "rsk")
}

fn rust_sources() -> Vec<(PathBuf, String)> {
    sources(&["tests/rust_corpus"], "rs")
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

#[test]
fn formatting_is_idempotent_and_keeps_the_generated_rust() {
    for (path, source) in rusk_sources() {
        let once = format_source(&source, FormatOptions::default()).unwrap();
        let twice = format_source(&once, FormatOptions::default()).unwrap();
        assert_eq!(once, twice, "{}", path.display());
        assert_eq!(
            transpile(&once).unwrap().rust,
            transpile(&source).unwrap().rust,
            "{}",
            path.display()
        );
    }
}

#[test]
fn generated_rust_ends_with_exactly_one_newline() {
    for (path, source) in rusk_sources() {
        let rust = transpile(&source).unwrap().rust;
        assert!(
            rust.ends_with('\n') && !rust.ends_with("\n\n"),
            "{}",
            path.display()
        );
    }
}

#[test]
fn line_endings_and_a_missing_final_newline_do_not_change_the_output() {
    for (path, source) in rusk_sources() {
        let expected = transpile(&source).unwrap().rust;
        let crlf = source.replace('\n', "\r\n");
        assert_eq!(
            transpile(&crlf).unwrap().rust,
            expected,
            "CRLF {}",
            path.display()
        );
        let unterminated = source.trim_end_matches('\n');
        assert_eq!(
            transpile(unterminated).unwrap().rust,
            expected,
            "no final newline {}",
            path.display()
        );
    }
}

fn assert_node_inside(
    node: &SourceMapNode,
    source_lines: usize,
    rust_lines: usize,
    parent: (usize, usize),
    path: &Path,
) {
    let context = format!(
        "{} {:?} at line {}",
        path.display(),
        node.kind,
        node.source_line
    );
    assert!((1..=source_lines).contains(&node.source_line), "{context}");
    assert!(node.generated_start_line >= 1, "{context}");
    assert!(
        node.generated_start_line <= node.generated_end_line,
        "{context}"
    );
    assert!(node.generated_end_line <= rust_lines, "{context}");
    assert!(
        parent.0 <= node.generated_start_line && node.generated_end_line <= parent.1,
        "{context} escapes its parent {parent:?}"
    );
    for child in &node.children {
        assert_node_inside(
            child,
            source_lines,
            rust_lines,
            (node.generated_start_line, node.generated_end_line),
            path,
        );
    }
}

#[test]
fn source_maps_point_inside_both_files() {
    for (path, source) in rusk_sources() {
        let output = transpile(&source).unwrap();
        let (source_lines, rust_lines) = (line_count(&source), line_count(&output.rust));
        assert!(!output.source_map.is_empty(), "{}", path.display());
        for entry in &output.source_map {
            assert!(
                (1..=source_lines).contains(&entry.source_line)
                    && (1..=rust_lines).contains(&entry.generated_line),
                "{} {entry:?}",
                path.display()
            );
        }
        for node in &output.source_tree {
            assert_node_inside(node, source_lines, rust_lines, (1, rust_lines), &path);
        }
    }
}

#[test]
fn source_map_lines_carry_their_source_text() {
    for (path, source) in rusk_sources() {
        let output = transpile(&source).unwrap();
        let lines = source.lines().collect::<Vec<_>>();
        let mut stack = output.source_tree.iter().collect::<Vec<_>>();
        while let Some(node) = stack.pop() {
            let line = lines[node.source_line - 1];
            let text = node.source_text.lines().next().unwrap_or_default().trim();
            assert!(
                !text.is_empty() && line.contains(text),
                "{} line {}: {line:?} does not contain {text:?}",
                path.display(),
                node.source_line
            );
            stack.extend(&node.children);
        }
    }
}

#[test]
fn rust_round_trips_reach_a_fixed_point_after_one_pass() {
    let round_trip = |rust: &str| transpile(&rust_to_rusk(rust).unwrap()).unwrap().rust;
    let rust = rust_sources().into_iter().chain(
        rusk_sources()
            .into_iter()
            .map(|(path, source)| (path, transpile(&source).unwrap().rust)),
    );
    for (path, rust) in rust {
        let once = round_trip(&rust);
        assert_eq!(round_trip(&once), once, "{}", path.display());
    }
}

#[test]
fn malformed_sources_fail_at_the_offending_line() {
    let cases = [
        ("fn main():\n\tlet x = 1\n", 2),
        ("fn main():\n    let x = 1\n      let y = 2\n", 2),
        ("struct A:\n    x: i32\n        y: i32\n", 2),
        ("fn f() = g(x):\n    1\n", 1),
        ("fn main():\n    let x = (\n", 2),
        ("fn main():\n    let s = \"open\n", 2),
    ];
    for (source, line) in cases {
        let error = transpile(source).expect_err(source);
        assert_eq!(error.line, line, "{source:?}: {error}");
        assert!(!error.message.is_empty(), "{source:?}");
        let formatted = format_source(source, FormatOptions::default()).expect_err(source);
        assert_eq!(formatted, error, "fmt and transpile disagree on {source:?}");
    }
}
