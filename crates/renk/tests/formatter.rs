use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use renk::{format_source_with, transpile};

fn rustfmt(rust: &str) -> Option<String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("rustfmt runs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(rust.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).unwrap())
}

fn fmt(source: &str) -> String {
    format_source_with(source, &rustfmt).unwrap()
}

fn formatted_rust(source: &str) -> String {
    rustfmt(&transpile(source).unwrap().rust).unwrap()
}

#[test]
fn examples_and_corpus_format_idempotently_to_the_same_rust() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Formatting may only inline a format argument that Rust captures the same way.
    let inlined = [(
        "async_functions.rk",
        "println!(\"result: {}\", result);",
        "println!(\"result: {result}\");",
    )];
    let mut files = 0;
    for dir in [root.join("../../examples"), root.join("tests/renk_corpus")] {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| extension != "rk") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let source = fs::read_to_string(&path).unwrap();
            let once = fmt(&source);
            let expected = inlined
                .iter()
                .filter(|(file, _, _)| *file == name)
                .fold(formatted_rust(&source), |rust, (_, from, to)| {
                    rust.replace(from, to)
                });
            assert_eq!(formatted_rust(&once), expected, "{name}\n{once}");
            assert_eq!(fmt(&once), once, "{name} is not idempotent");
            files += 1;
        }
    }
    assert!(files > 30, "{files}");
}

#[test]
fn normalizes_indentation_and_line_breaks_like_rustfmt() {
    let source = "\
fn main():
  let values = vec![1, 2, 3]
  for value in values.iter():
        if *value > 1:
           println!(\"{value}\")
  let total: i32 = values.iter().map(|value| value * 2).filter(|value| *value > 2).sum::<i32>() + 100
  println!(\"{total}\")
";
    assert_eq!(
        fmt(source),
        "\
fn main():
    let values = vec![1, 2, 3]
    for value in values.iter():
        if *value > 1:
            println!(\"{value}\")
    let total: i32 = values
        .iter()
        .map(|value| value * 2)
        .filter(|value| *value > 2)
        .sum[i32]()
        + 100
    println!(\"{total}\")
"
    );
}

#[test]
fn keeps_comments_blank_lines_and_multi_line_strings() {
    let source = "\
// Leading comment.
fn  text()  -> &'static str:
    // Inside.
    let raw = r#\"
  keep   this
\"#

    let plain = \"a
    b  \"  // trailing
    if raw.len() > plain.len() then raw else plain
";
    let formatted = fmt(source);
    assert_eq!(
        formatted,
        "\
// Leading comment.
fn text() -> &'static str:
    // Inside.
    let raw = r#\"
  keep   this
\"#

    let plain = \"a
    b  \" // trailing
    if raw.len() > plain.len() then raw else plain
"
    );
}

#[test]
fn formats_a_script_body_and_keeps_its_header() {
    let source = "\
#!/usr/bin/env -S renk script
---
[dependencies]
regex  =  \"1\"
---

// Top-level statements.
let   words = vec![\"a\", \"b\"]
for word in words.iter():
      println!(\"{word}\")
let text = \"line
  kept\"
println!(\"{text}\")
";
    let formatted = fmt(source);
    assert_eq!(
        formatted,
        "\
#!/usr/bin/env -S renk script
---
[dependencies]
regex  =  \"1\"
---

// Top-level statements.
let words = vec![\"a\", \"b\"]
for word in words.iter():
    println!(\"{word}\")
let text = \"line
  kept\"
println!(\"{text}\")
"
    );
    assert_eq!(fmt(&formatted), formatted);
}

#[test]
fn keeps_the_lines_of_items_whose_formatting_changes_the_rust() {
    // Formatting twice adds another `bad();`, so the item with it never formats to the same Rust.
    let doubling = |rust: &str| Some(rust.replacen("bad();", "bad(); bad();", 1));
    let source = "fn one() -> i32:\n    1\n\nfn two():   \n    bad()\n";
    assert_eq!(
        format_source_with(source, &doubling).unwrap(),
        "fn one() -> i32 = 1\n\nfn two():\n    bad()\n"
    );
}

#[test]
fn formats_items_alone_when_the_whole_file_does_not_format() {
    let failing = |rust: &str| (!rust.contains("broken")).then(|| rust.to_string());
    let source = "fn one() -> i32:\n    1\n\nfn two():\n    broken()  \n";
    assert_eq!(
        format_source_with(source, &failing).unwrap(),
        "fn one() -> i32 = 1\n\nfn two():\n    broken()\n"
    );
    assert_eq!(
        format_source_with(source, &|_| None).unwrap(),
        "fn one() -> i32:\n    1\n\nfn two():\n    broken()\n"
    );
}

#[test]
fn reports_parse_errors_at_script_lines() {
    let script = "#!/usr/bin/env -S renk script\n---\n---\nfn f():\n\tx\n";
    assert_eq!(format_source_with(script, &rustfmt).unwrap_err().line, 5);
}
