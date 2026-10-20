//! Converts every library source of pinned crates.io releases from Rust to Rusk and back, then
//! checks that each file keeps its comments and each crate still compiles to the same MIR, which
//! nightly emits without spans or source locations so only meaning is compared. Crates come from
//! `cargo vendor`, which checks them against crates.io checksums, rather than git submodules, so
//! nothing large is checked in.
//! Downloads on first run: `cargo test -p rusk --test real_crates -- --ignored`.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rusk::{SourceSyntax, rust_to_rusk, to_rust};

const CRATES: &[(&str, &str)] = &[
    ("anyhow", "1.0.104"),
    ("bitflags", "2.13.2"),
    ("either", "1.19.0"),
    ("hashbrown", "0.17.1"),
    ("indexmap", "2.14.2"),
    ("itertools", "0.15.0"),
    ("log", "0.4.34"),
    ("memchr", "2.8.3"),
    ("once_cell", "1.21.4"),
    ("regex-syntax", "0.8.11"),
    ("semver", "1.0.28"),
    ("serde_json", "1.0.154"),
    ("smallvec", "1.15.1"),
    ("thiserror-impl", "2.0.21"),
    ("unicode-width", "0.2.2"),
];

/// The library builds whose MIR is compared: default features, every feature, and the unit
/// tests with every feature.
const MIR_CONFIGS: &[(&str, &[&str])] = &[
    ("default", &[]),
    ("all-features", &["--all-features"]),
    ("tests", &["--all-features", "--profile", "test"]),
];

#[test]
#[ignore = "downloads crates from crates.io"]
fn real_crates_roundtrip_with_their_comments_and_mir() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("real-crates");
    let vendor = vendor(&root);
    let work = root.join("work");
    let _ = fs::remove_dir_all(&work);

    let mut failures = Vec::new();
    for (name, version) in CRATES {
        let original = vendor.join(format!("{name}-{version}"));
        let converted = work.join(format!("{name}-{version}"));
        copy_crate(&original, &converted);
        for file in rust_files(&converted.join("src")) {
            let rust = fs::read_to_string(&file).unwrap();
            let shown = file.strip_prefix(&work).unwrap().display().to_string();
            match rust_to_rusk(&rust)
                .map_err(|error| format!("to Rusk: {error}"))
                .and_then(|rusk| {
                    to_rust(&rusk, SourceSyntax::Rusk).map_err(|error| format!("to Rust: {error}"))
                }) {
                Ok(roundtripped) => {
                    let (before, after) = (comments(&rust), comments(&roundtripped));
                    if let Some(index) = first_difference(&before, &after) {
                        failures.push(format!(
                            "{shown}: comment {index} changed: {:?} -> {:?}",
                            before.get(index),
                            after.get(index)
                        ));
                    }
                    fs::write(&file, roundtripped).unwrap();
                }
                Err(error) => failures.push(format!("{shown}: {error}")),
            }
        }
        let baseline = work.join(format!("{name}-{version}-original"));
        copy_crate(&original, &baseline);
        if let Err(errors) = cargo_check(&converted, &root) {
            match cargo_check(&baseline, &root) {
                Ok(()) => failures.push(format!("{name}-{version} does not compile:\n{errors}")),
                Err(errors) => panic!("{name}-{version} does not compile unconverted:\n{errors}"),
            }
            continue;
        }
        for (config, args) in MIR_CONFIGS {
            let mir_dir = |variant: &str| {
                root.join("mir")
                    .join(format!("{name}-{version}-{config}-{variant}"))
            };
            // Features or tests some crates leave for other toolchains or platforms.
            let Ok(before) = mir(&baseline, &mir_dir("original"), args) else {
                assert!(
                    *config != "default",
                    "{name}-{version} has no MIR unconverted"
                );
                continue;
            };
            match mir(&converted, &mir_dir("converted"), args) {
                Ok(after) => {
                    let (before, after) = (mir_items(&before), mir_items(&after));
                    let (before, after): (Vec<_>, Vec<_>) = (
                        before.iter().map(String::as_str).collect(),
                        after.iter().map(String::as_str).collect(),
                    );
                    if let Some(index) = first_difference(&before, &after) {
                        let (old, new): (Vec<_>, Vec<_>) = (
                            before.get(index).map_or("", |item| item).lines().collect(),
                            after.get(index).map_or("", |item| item).lines().collect(),
                        );
                        let line = first_difference(&old, &new).unwrap_or_default();
                        failures.push(format!(
                            "{name}-{version} {config} MIR of {:?} changed: {:?} -> {:?}",
                            old.first(),
                            old.get(line),
                            new.get(line)
                        ));
                    }
                }
                Err(errors) => {
                    failures.push(format!("{name}-{version} has no {config} MIR:\n{errors}"))
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn mir_items_ignore_layout_but_not_code() {
    let mir = |blocks: &str, allocations: &str| {
        format!("fn f(_1: bool) -> u8 {{\n    let mut _0: u8;\n{blocks}}}\n\n{allocations}")
    };
    let entry = "    bb0: {\n        switchInt(copy _1) -> [0: bb2, otherwise: bb1];\n    }\n";
    let one = "    bb1: {\n        _0 = const 1_u8;\n        return;\n    }\n";
    let two = "    bb2: {\n        _0 = const 2_u8;\n        return;\n    }\n";
    let (a, b) = (
        "alloc3 (size: 1, align: 1) {\n    61\n}\n",
        "alloc4 (size: 1, align: 1) {\n    62\n}\n",
    );
    let original = mir_items(&mir(&format!("{entry}{one}{two}"), &format!("{a}{b}")));
    let reordered = mir(
        &format!(
            "{}{}{}",
            entry
                .replace("bb2", "bb9")
                .replace("bb1", "bb2")
                .replace("bb9", "bb1"),
            two.replace("bb2", "bb1"),
            one.replace("bb1", "bb2")
        ),
        &format!("{b}{a}"),
    );
    assert_eq!(mir_items(&reordered), original);
    let unit = mir(
        &format!(
            "    let _5: ();\n{}",
            entry.replace(
                "        switchInt",
                "        _5 = const ();\n        switchInt"
            )
        ),
        "",
    );
    assert_eq!(mir_items(&unit)[0], mir_items(&mir(entry, ""))[0]);
    let never = |binding: &str| mir(&format!("    let {binding}_5: !;\n{entry}"), "");
    assert_eq!(mir_items(&never("mut "))[0], mir_items(&never(""))[0]);
    let test = |line: u32, value: u8| {
        mir(
            &entry.replace(
                "        switchInt",
                &format!(
                    "        _1 = TestDesc {{ start_line: const {line}_usize, start_col: const 4_usize, end_line: const {line}_usize, end_col: const 9_usize, value: const {value}_u8 }};\n        switchInt"
                ),
            ),
            "",
        )
    };
    assert_eq!(mir_items(&test(3, 1))[0], mir_items(&test(4, 1))[0]);
    let dump = |name: &str, padding: &str| {
        mir(
            entry,
            &format!(
                "{name} (size: 8, align: 8) {{\n    ╾──{name}<imm>{padding}╼ │ ╾──────╼\n}}\n"
            ),
        )
    };
    assert_eq!(
        mir_items(&dump("alloc12", "──")),
        mir_items(&dump("alloc3", "───"))
    );
    assert_ne!(mir_items(&test(3, 1))[0], mir_items(&test(3, 2))[0]);
    let swapped = mir(
        &format!(
            "{entry}{}{}",
            one.replace("bb1", "bb2"),
            two.replace("bb2", "bb1")
        ),
        "",
    );
    assert_ne!(
        mir_items(&swapped)[0],
        mir_items(&mir(&format!("{entry}{one}{two}"), ""))[0]
    );
    let uses = |first: &str, second: &str| {
        mir(
            &format!(
                "{entry}{}{}",
                one.replace("const 1_u8", &format!("const {{{first}: &u8}}")),
                two.replace("const 2_u8", &format!("const {{{second}: &u8}}"))
            ),
            &format!("{a}{b}"),
        )
    };
    assert_eq!(
        mir_items(&uses("alloc3", "alloc4")),
        mir_items(
            &uses("alloc4", "alloc3")
                .replace("alloc3 (", "alloc9 (")
                .replace("alloc4 (", "alloc3 (")
                .replace("alloc9 (", "alloc4 (")
        )
    );
    assert_ne!(
        mir_items(&uses("alloc3", "alloc4")),
        mir_items(&uses("alloc4", "alloc3"))
    );
    let text = |text: &str| {
        mir(
            &one.replace("const 1_u8", &format!("const \"x@a.rs:1 {text}}}\"")),
            "",
        )
    };
    assert_ne!(mir_items(&text("one")), mir_items(&text("two")));
}

/// The vendored crate sources, downloaded once into `root`.
fn vendor(root: &Path) -> PathBuf {
    let vendor = root.join("vendor");
    let fetch = root.join("fetch");
    let manifest = CRATES.iter().fold(
        "[package]\nname = \"fetch\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\n"
            .to_string(),
        |manifest, (name, version)| manifest + &format!("{name} = \"={version}\"\n"),
    );
    // Written after vendoring, so an interrupted download is retried.
    let vendored = fetch.join("vendored.toml");
    if fs::read_to_string(&vendored).is_ok_and(|existing| existing == manifest) {
        return vendor;
    }
    fs::create_dir_all(fetch.join("src")).unwrap();
    fs::write(fetch.join("src/lib.rs"), "").unwrap();
    fs::write(fetch.join("Cargo.toml"), &manifest).unwrap();
    let status = Command::new("cargo")
        .args(["vendor", "--quiet", "--versioned-dirs", "--manifest-path"])
        .arg(fetch.join("Cargo.toml"))
        .arg(&vendor)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "cargo vendor failed");
    fs::write(vendored, manifest).unwrap();
    vendor
}

/// `cargo check` of a crate's library, or its compiler errors.
fn cargo_check(crate_dir: &Path, root: &Path) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["check", "--lib", "--quiet", "--message-format=short"])
        .current_dir(crate_dir)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors = stderr
        .lines()
        .filter(|line| line.contains("error"))
        .collect::<Vec<_>>();
    Err(errors[..errors.len().min(10)].join("\n"))
}

/// The MIR of a crate's library, built with nightly into `target`.
fn mir(crate_dir: &Path, target: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("cargo")
        .args([
            "+nightly",
            "rustc",
            "--lib",
            "--quiet",
            "--message-format=short",
        ])
        .args(args)
        .arg("--")
        .args(["--emit=mir", "-Cdebuginfo=0", "-Zlocation-detail=none"])
        .arg("-Zmir-include-spans=off")
        .current_dir(crate_dir)
        .env("CARGO_TARGET_DIR", target)
        // Incremental state from an earlier conversion renumbers allocations.
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    // A build with other flags, which cargo keeps beside this one, writes its MIR under another
    // hash, and a fresh build writes none.
    fs::read_dir(target.join("debug/deps"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mir"))
        .max_by_key(|path| fs::metadata(path).unwrap().modified().unwrap())
        .map(|file| fs::read_to_string(file).unwrap())
        .ok_or_else(|| format!("no .mir file in {}", target.display()))
}

/// The items of `mir`, each a function with the allocations and promoted constants after it,
/// without what the layout moves or a `;` after a unit expression adds: source locations, unit
/// temporaries, the numbering of locals, basic blocks, and allocations, and the order basic
/// blocks and allocations print in.
fn mir_items(mir: &str) -> Vec<String> {
    // A pointer in an allocation's dump, as in `╾──alloc3<imm>──╼`, pads to its width, which
    // depends on the allocation's number.
    let mir = without_locations(mir)
        .split('─')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("─");
    // Top-level definitions: a function, an allocation, or a promoted constant.
    let mut definitions = Vec::<Vec<&str>>::new();
    for line in mir.lines() {
        match definitions.last_mut() {
            Some(definition) if line.is_empty() || line.starts_with([' ', '}']) => {
                definition.push(line)
            }
            _ => definitions.push(vec![line]),
        }
    }
    let mut items = Vec::<(String, Vec<String>)>::new();
    for definition in &definitions {
        let text = canonical_body(definition);
        match items.last_mut() {
            Some((_, attached)) if !definition[0].starts_with("fn ") => attached.push(text),
            _ => items.push((text, Vec::new())),
        }
    }
    items
        .into_iter()
        .map(|(function, attached)| {
            // Allocations print in interning order, so they follow in the order the function,
            // its promoted constants, and earlier allocations first name them.
            let (allocations, mut promoted) = attached
                .into_iter()
                .partition::<Vec<_>, _>(|definition| definition.starts_with("alloc"));
            promoted.sort_by_cached_key(|definition| renumber(definition, "alloc", &[]));
            let mut texts = vec![function];
            texts.extend(promoted);
            let mut named = Vec::<String>::new();
            let mut scanned = 0;
            while scanned < texts.len() {
                for name in renumber_with_names(&texts[scanned], "alloc", &[]).1 {
                    if !named.contains(&name) {
                        named.push(name.clone());
                        texts.extend(
                            allocations
                                .iter()
                                .find(|definition| definition.split(' ').next() == Some(&name))
                                .cloned(),
                        );
                    }
                }
                scanned += 1;
            }
            let mut unnamed = allocations
                .into_iter()
                .filter(|definition| !texts.contains(definition))
                .collect::<Vec<_>>();
            unnamed.sort_by_cached_key(|definition| renumber(definition, "alloc", &[]));
            texts.extend(unnamed);
            renumber(&texts.join("\n"), "alloc", &[])
        })
        .collect()
}

/// A definition's lines without unit temporaries, with its basic blocks in depth-first order from
/// `bb0`, and with blocks and locals numbered by first appearance.
fn canonical_body(lines: &[&str]) -> String {
    let units = lines
        .iter()
        .filter_map(|line| {
            let local = line.trim().strip_prefix("let ")?;
            let local = local.strip_prefix("mut ").unwrap_or(local);
            local.strip_suffix(": ();")
        })
        .collect::<Vec<_>>();
    // A `;` after a diverging expression, as in `panic!();`, leaves its `!` local immutable.
    let lines = lines
        .iter()
        .filter(|line| {
            let line = line.trim();
            !units.iter().any(|unit| {
                line == format!("let {unit}: ();")
                    || line == format!("let mut {unit}: ();")
                    || line == format!("{unit} = const ();")
            })
        })
        .map(|line| match line.strip_prefix("    let mut ") {
            Some(local) if local.ends_with(": !;") => format!("    let {local}"),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>();
    let lines = lines.iter().map(String::as_str).collect::<Vec<_>>();
    let is_block = |line: &&str| line.starts_with("    bb") && line.ends_with(": {");
    let start = lines.iter().position(is_block).unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| *line == "}")
        .unwrap_or(lines.len())
        .max(start);
    let mut blocks = Vec::<(&str, Vec<&str>)>::new();
    for line in &lines[start..end] {
        match blocks.last_mut() {
            Some((_, block)) if !is_block(line) => block.push(line),
            _ => blocks.push((
                line.trim_start().split([':', ' ']).next().unwrap(),
                vec![line],
            )),
        }
    }
    let mut order = Vec::<usize>::new();
    let mut stack = vec![0];
    while let Some(index) = stack.pop() {
        if index >= blocks.len() || order.contains(&index) {
            continue;
        }
        order.push(index);
        let successors = blocks[index].1[1..]
            .iter()
            .flat_map(|line| line.split(|c: char| !c.is_alphanumeric() && c != '_'))
            .filter_map(|word| blocks.iter().position(|(name, _)| *name == word))
            .collect::<Vec<_>>();
        stack.extend(successors.into_iter().rev());
    }
    let unreached = (0..blocks.len())
        .filter(|index| !order.contains(index))
        .collect::<Vec<_>>();
    order.extend(unreached);
    let text = lines[..start]
        .iter()
        .copied()
        .chain(
            order
                .iter()
                .flat_map(|&index| blocks[index].1.iter().copied()),
        )
        .chain(lines[end..].iter().copied())
        .collect::<Vec<_>>()
        .join("\n");
    renumber(&renumber(&text, "bb", &[]), "_", &units)
}

/// `text` with each `{prefix}N` numbered by first appearance, and the `units` as `_unit`.
fn renumber(text: &str, prefix: &str, units: &[&str]) -> String {
    renumber_with_names(text, prefix, units).0
}

/// [`renumber`]'s output, and the original names in order of first appearance.
fn renumber_with_names(text: &str, prefix: &str, units: &[&str]) -> (String, Vec<String>) {
    let mut names = Vec::<String>::new();
    let mut output = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(prefix) {
        let (head, tail) = rest.split_at(at);
        output.push_str(head);
        let digits = tail[prefix.len()..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(tail.len() - prefix.len());
        let name = &tail[..prefix.len() + digits];
        let standalone = digits > 0
            && !head.ends_with(|c: char| c.is_alphanumeric() || c == '_')
            && !tail[name.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_');
        if !standalone {
            output.push_str(&tail[..prefix.len()]);
            rest = &tail[prefix.len()..];
            continue;
        }
        if units.contains(&name) {
            output.push_str("_unit");
        } else {
            let number = names
                .iter()
                .position(|seen| seen == name)
                .unwrap_or_else(|| {
                    names.push(name.to_string());
                    names.len() - 1
                });
            output.push_str(&format!("{prefix}{number}"));
        }
        rest = &tail[name.len()..];
    }
    output.push_str(rest);
    (output, names)
}

/// `mir` without the lines and columns that name impls and closures, as in
/// `<impl at src/a.rs:18:1: 18:23>` or `{closure@src/a.rs:4:9: 4:12}`, which move with the layout.
fn without_locations(mir: &str) -> String {
    let mut output = String::new();
    let mut rest = mir;
    while let Some(at) = ["<impl at ", "@"]
        .iter()
        .filter_map(|marker| rest.find(marker).map(|at| at + marker.len()))
        .min()
    {
        let (head, tail) = rest.split_at(at);
        output.push_str(head);
        let path_end = tail.find([':', '>', '}']).unwrap_or(tail.len());
        output.push_str(&tail[..path_end]);
        rest = &tail[path_end..];
        // Only `:line:column: line:column` before the `>` or `}`, unlike `"x@a.rs:1 one}"`.
        if let Some(location) = rest.strip_prefix(':')
            && let Some(end) = location.find(['>', '}'])
            && location[..end]
                .split([':', ' '])
                .filter(|part| !part.is_empty())
                .count()
                == 4
            && location[..end]
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, ':' | ' '))
        {
            rest = &location[end..];
        }
    }
    output.push_str(rest);
    // A test's `TestDesc` holds its source lines and columns as constants.
    for key in ["start_line", "start_col", "end_line", "end_col"] {
        let marker = format!("{key}: const ");
        let mut rest = output.as_str();
        let mut normalized = String::new();
        while let Some(at) = rest.find(&marker) {
            let (head, tail) = rest.split_at(at + marker.len());
            normalized.push_str(head);
            rest = tail.trim_start_matches(|c: char| c.is_ascii_digit());
        }
        normalized.push_str(rest);
        output = normalized;
    }
    output
}

/// The comments of a Rust source, in order.
fn comments(rust: &str) -> Vec<&str> {
    let mut offset = 0;
    tokenize(rust, FrontmatterAllowed::No)
        .filter_map(|token| {
            let text = &rust[offset..offset + token.len as usize];
            offset += token.len as usize;
            matches!(
                token.kind,
                TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
            )
            .then_some(text.trim_end())
        })
        .collect()
}

/// The index of the first item that differs between `before` and `after`.
fn first_difference(before: &[&str], after: &[&str]) -> Option<usize> {
    (0..before.len().max(after.len())).find(|&index| before.get(index) != after.get(index))
}

/// Copies a vendored crate as its own workspace.
fn copy_crate(from: &Path, to: &Path) {
    copy_dir(from, to);
    let manifest = to.join("Cargo.toml");
    let mut document = fs::read_to_string(&manifest)
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    document.insert("workspace", toml_edit::table());
    fs::write(manifest, document.to_string()).unwrap();
}

fn copy_dir(from: &Path, to: &Path) {
    let _ = fs::remove_dir_all(to);
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}
