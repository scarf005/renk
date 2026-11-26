use std::{fs, path::Path};

struct Block {
    lang: String,
    line: usize,
    body: String,
    /// Non-blank lines of prose between the previous block and this one.
    prose_before: usize,
}

fn blocks(page: &str) -> Vec<Block> {
    let mut blocks = vec![];
    let mut open: Option<Block> = None;
    let mut prose = 0;
    for (index, line) in page.lines().enumerate() {
        match (&mut open, line.strip_prefix("```")) {
            (None, Some(lang)) => {
                let block = Block {
                    lang: lang.to_string(),
                    line: index + 1,
                    body: String::new(),
                    prose_before: prose,
                };
                open = Some(block);
                prose = 0;
            }
            (Some(_), Some(_)) => blocks.extend(open.take()),
            (Some(block), None) => {
                block.body.push_str(line);
                block.body.push('\n');
            }
            (None, None) => prose += usize::from(!line.is_empty()),
        }
    }
    blocks
}

/// An `rk` or `rk,ignore` block followed by a `rust` block with at most one line of prose between
/// them ("becomes", "Generated Rust:" or the Korean forms) must transpile to that `rust` block.
#[test]
fn documented_rust_is_the_transpiler_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut pages = vec![
        root.join("README.md"),
        root.join("docs/src"),
        root.join("docs/ko/src"),
    ];
    let (mut pairs, mut mismatches) = (0, vec![]);
    while let Some(path) = pages.pop() {
        if path.is_dir() {
            pages.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if path.extension().is_some_and(|extension| extension == "md") {
            let page = fs::read_to_string(&path).unwrap();
            let blocks = blocks(&page);
            for pair in blocks.windows(2) {
                let [renk, rust] = pair else { unreachable!() };
                if !matches!(renk.lang.as_str(), "rk" | "rk,ignore")
                    || rust.lang != "rust"
                    || rust.prose_before > 1
                {
                    continue;
                }
                pairs += 1;
                let actual = renk::transpile(&renk.body)
                    .unwrap_or_else(|error| panic!("{}:{}: {error}", path.display(), renk.line))
                    .rust;
                if actual.trim_end() != rust.body.trim_end() {
                    mismatches.push(format!(
                        "{}:{}\n--- documented\n{}--- actual\n{actual}",
                        path.display(),
                        renk.line,
                        rust.body
                    ));
                }
            }
        }
    }
    assert!(pairs >= 7, "found only {pairs} pairs");
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
