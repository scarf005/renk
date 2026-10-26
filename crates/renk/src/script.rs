//! Single-file scripts: the cargo-script frontmatter, the `fn main` around top-level lines, and
//! the generated Rust that `renk script` builds and `renk-lsp` hands to rust-analyzer.

use std::{
    ops::Range,
    path::{Path, PathBuf},
};

use similar::{DiffTag, TextDiff};

use crate::{
    SourceSyntax, TranspileError, is_ident_continue, is_ident_start, opaque_span_end, read_ident,
    rust_to_renk, to_rust, transpile, unwrap_script_statements, wrap_script_statements,
};

/// The 0-based line indices of a script's parts: an optional `#!` line, the optional `---`
/// frontmatter with its fence lines, and the body after them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptParts {
    /// The opening and closing fence lines of the frontmatter.
    pub frontmatter: Option<(usize, usize)>,
    pub body_start: usize,
}

/// Finds the parts of a script; a frontmatter fence without its closing line is an error.
pub fn script_parts(source: &str) -> Result<ScriptParts, TranspileError> {
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let shebang = usize::from(
        lines
            .first()
            .is_some_and(|line| line.starts_with("#!") && !line.starts_with("#![")),
    );
    let fence_start = (shebang..lines.len()).find(|&index| !lines[index].trim().is_empty());
    let Some(start) = fence_start.filter(|&index| lines[index].starts_with("---")) else {
        return Ok(ScriptParts {
            frontmatter: None,
            body_start: shebang,
        });
    };
    let fence = &lines[start][..lines[start].len() - lines[start].trim_start_matches('-').len()];
    let Some(end) = (start + 1..lines.len()).find(|&index| lines[index].trim_end() == fence) else {
        return Err(TranspileError {
            line: start + 1,
            message: "unclosed frontmatter".to_string(),
        });
    };
    Ok(ScriptParts {
        frontmatter: Some((start, end)),
        body_start: end + 1,
    })
}

/// The script with its `#!` and frontmatter lines blanked, so items keep their script lines.
pub fn script_body(source: &str) -> Result<String, TranspileError> {
    let parts = script_parts(source)?;
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    Ok("\n".repeat(parts.body_start) + &lines[parts.body_start..].concat())
}

/// A script's generated Rust and, for each 1-based Rust line, the script line it came from and
/// whether renk synthesized it, like a closing brace.
#[derive(Debug)]
pub struct ScriptRust {
    pub rust: String,
    pub source_lines: Vec<(usize, bool)>,
    /// Whether renk named the package, which keeps it apart from other scripts' packages.
    pub named_package: bool,
    pub package: String,
}

/// Transpiles a single-file script, keeping the cargo-script frontmatter in front of the generated
/// Rust. Header lines become blank lines so transpile errors keep the script's line numbers.
pub fn script_rust(
    source: &str,
    syntax: SourceSyntax,
    package: &str,
) -> Result<ScriptRust, TranspileError> {
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let parts = script_parts(source)?;
    let (manifest, mut named_package, mut package_name) = with_package_defaults("", package);
    let mut frontmatter = format!("---\n{manifest}---\n");
    let mut frontmatter_line = 1;
    let mut result = None;
    if let Some((start, end)) = parts.frontmatter {
        let written = lines[start + 1..end].concat();
        result = script_result(&written);
        let manifest;
        (manifest, named_package, package_name) = with_package_defaults(&written, package);
        frontmatter = lines[start].to_string() + &manifest;
        frontmatter += lines[end].trim_end();
        frontmatter.push('\n');
        frontmatter_line = start + 1;
    }
    let body_start = parts.body_start;

    let mut body = "\n".repeat(body_start) + &lines[body_start..].concat();
    let script_lines = body.lines().count();
    let mut header = None;
    if syntax != SourceSyntax::Rust
        && let Some((wrapped, line)) = wrap_script_statements(&body, syntax, result.as_deref())
    {
        body = wrapped;
        header = Some(line);
    }
    // The script line of a line of `body`, or `None` for the `fn main` wrapper's own lines.
    let script_line = |line: usize| match header {
        Some(header) if line == header || line > script_lines + 1 => None,
        Some(header) if line > header => Some(line - 1),
        _ => Some(line),
    };
    let error_line = |mut error: TranspileError| {
        error.line = script_line(error.line).unwrap_or(error.line);
        error
    };
    let (rust, body_lines) = if syntax == SourceSyntax::Renk {
        let output = transpile(&body).map_err(error_line)?;
        let lines = output
            .line_origins()
            .into_iter()
            .map(|(source, synthesized)| match script_line(source) {
                Some(source) => (source, synthesized),
                None => (source.min(script_lines.max(1)), true),
            })
            .collect::<Vec<_>>();
        (output.rust, lines)
    } else {
        let rust = to_rust(&body, syntax).map_err(error_line)?;
        let lines = (1..=rust.lines().count())
            .map(|line| script_line(line).map_or((line, true), |line| (line, false)))
            .collect();
        (rust, lines)
    };
    // A frontmatter renk wrote for a script without one has no script lines.
    let source_lines = (0..frontmatter.lines().count())
        .map(|offset| {
            let line = (frontmatter_line + offset).min(body_start.max(1));
            (line, parts.frontmatter.is_none())
        })
        .chain(body_lines)
        .collect();
    Ok(ScriptRust {
        rust: frontmatter + &rust,
        source_lines,
        named_package,
        package: package_name,
    })
}

/// What `rust`, the Rust [`script_rust`] wrote for the script `source`, converts back to with
/// [`rust_to_renk`]: without the frontmatter, and with top-level lines taken out of the `fn main`
/// wrapper; and the frontmatter and wrapper header it leaves out, so an edit of them can be told
/// apart. `None` when the Rust does not convert or the wrapper is gone.
pub fn script_renk(source: &str, rust: &str) -> Option<(String, String)> {
    let lines = rust.split_inclusive('\n').collect::<Vec<_>>();
    let body_start = script_parts(rust).ok()?.body_start;
    let frontmatter = lines[..body_start].concat();
    let renk = rust_to_renk(&lines[body_start..].concat()).ok()?;
    if wrap_script_statements(&script_body(source).ok()?, SourceSyntax::Renk, None).is_none() {
        return Some((renk, frontmatter));
    }
    let statements = unwrap_script_statements(&renk)?;
    // The header is the first line the unwrapping changed; the tail it checks is always the same.
    let mut unwrapped = statements.lines();
    let header = renk
        .lines()
        .find(|&line| unwrapped.next() != Some(line))
        .unwrap_or_default();
    Some((statements, frontmatter + header))
}

/// The edition renk's generated Rust is written for, pinned in scripts that do not name one so
/// cargo does not warn about defaulting to the latest edition on every run.
pub const SCRIPT_EDITION: &str = "2024";

/// Adds `package.edition` and `package.name` to a cargo-script manifest that lacks them, and says
/// whether it added the name and what the name is; cargo reports invalid TOML.
fn with_package_defaults(manifest: &str, name: &str) -> (String, bool, String) {
    let Ok(mut document) = manifest.parse::<toml_edit::DocumentMut>() else {
        return (manifest.to_string(), false, String::new());
    };
    let package = document.entry("package").or_insert(toml_edit::table());
    let Some(package) = package.as_table_like_mut() else {
        return (manifest.to_string(), false, String::new());
    };
    if !package.contains_key("edition") {
        package.insert("edition", toml_edit::value(SCRIPT_EDITION));
    }
    let named = !package.contains_key("name");
    if named {
        package.insert("name", toml_edit::value(name));
    }
    let name = package
        .get("name")
        .and_then(|name| name.as_str())
        .unwrap_or_default()
        .to_string();
    (document.to_string(), named, name)
}

/// The `Result` alias of the error crate a script's manifest depends on, such as `anyhow::Result`,
/// named by its dependency key, so a top-level `bail!` returns the error type `fn main` returns.
fn script_result(manifest: &str) -> Option<String> {
    let document = manifest.parse::<toml_edit::DocumentMut>().ok()?;
    let dependencies = document.get("dependencies")?.as_table_like()?;
    dependencies.iter().find_map(|(key, dependency)| {
        let package = dependency
            .get("package")
            .and_then(|package| package.as_str())
            .unwrap_or(key);
        ["anyhow", "eyre", "color-eyre"]
            .contains(&package)
            .then(|| format!("{}::Result", key.replace('-', "_")))
    })
}

/// A package name unique to the script's cache entry. Cargo keys a root package's artifacts by
/// its name and its path inside the workspace, which is the same for scripts with the same file
/// stem, so scripts sharing a build directory without it would run each other's binaries.
pub fn script_package(rust_path: &Path) -> String {
    let stem = rust_path.file_stem().unwrap_or_default().to_string_lossy();
    let hash = rust_path
        .parent()
        .and_then(Path::file_name)
        .unwrap_or_default();
    let mut name = stem.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_");
    // Package names must start with a letter or `_`.
    if !name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        name.insert(0, '_');
    }
    format!("{name}-{}", hash.to_string_lossy())
}

/// renk's cache directory, `$XDG_CACHE_HOME/renk` or `~/.cache/renk`.
pub fn cache_root() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .map(|cache| cache.join("renk"))
}

/// The generated Rust file of the script at `source_path` under `cache_root`.
pub fn script_cache_path(cache_root: &Path, source_path: &Path) -> PathBuf {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    source_path.hash(&mut hasher);
    let mut file_name = source_path.file_stem().unwrap_or_default().to_owned();
    file_name.push(".rs");
    cache_root
        .join(format!("{:016x}", hasher.finish()))
        .join(file_name)
}

/// The column of `script` that the character at `column` of `rust` came from: the same character
/// where the lines match, or where renk inserted or rewrote text, the script column it replaces.
pub fn script_column(script: &str, rust: &str, column: usize) -> usize {
    TextDiff::from_chars(script, rust)
        .ops()
        .iter()
        .map(similar::DiffOp::as_tag_tuple)
        .find(|(_, _, generated)| generated.contains(&column))
        .map_or(
            script.chars().count(),
            |(tag, written, generated)| match tag {
                DiffTag::Equal => written.start + column - generated.start,
                _ => written.start,
            },
        )
}

/// A `.` between two names in Renk code, as in `a.b`, and how renk read it. Ranges count the
/// characters of the Renk line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dot {
    pub receiver: Range<usize>,
    pub member: Range<usize>,
    /// Whether renk wrote the path `a::b` rather than keeping `a.b`, a member of the value `a`.
    pub path: bool,
    /// Whether the receiver is a name of its own, not the last segment of `x.a` or `x::a`.
    pub plain: bool,
}

impl Dot {
    /// How to write the other reading of the dot on the Renk line `renk`. A member of a receiver
    /// that is not a plain name has no other reading to suggest.
    pub fn hint(&self, renk: &str) -> Option<String> {
        let text = |range: &Range<usize>| -> String {
            renk.chars().skip(range.start).take(range.len()).collect()
        };
        let (receiver, member) = (text(&self.receiver), text(&self.member));
        if self.path {
            return Some(format!(
                "renk read `{receiver}.{member}` as the path `{receiver}::{member}`; write `({receiver}).{member}` for a value"
            ));
        }
        self.plain.then(|| {
            format!(
                "renk read `{receiver}.{member}` as a member of the value `{receiver}`; write `{receiver}::{member}` for a path"
            )
        })
    }
}

/// The dots between two names in the code of the Renk line `renk`, outside strings and comments,
/// read as paths where `rust`, the Rust line renk wrote for it, has `::` in their place by a
/// character diff of the two lines. Dots the diff shows neither kept nor turned into `::` are left
/// out.
pub fn line_dots(renk: &str, rust: &str) -> Vec<Dot> {
    let chars = renk.chars().collect::<Vec<_>>();
    let generated = rust.chars().collect::<Vec<_>>();
    let diff = TextDiff::from_chars(renk, rust);
    let reading = |dot: usize| {
        let (tag, _, range) = diff
            .ops()
            .iter()
            .map(similar::DiffOp::as_tag_tuple)
            .find(|(_, written, _)| written.contains(&dot))?;
        match tag {
            DiffTag::Equal => Some(false),
            _ => generated[range]
                .windows(2)
                .any(|pair| pair == [':', ':'])
                .then_some(true),
        }
    };
    let mut dots = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            index = end + 1;
        } else if chars[index].is_ascii_digit() {
            // A number such as `1.5` or `2.0f64` is no receiver.
            index += 1;
            while index < chars.len()
                && (is_ident_continue(chars[index])
                    || (chars[index] == '.'
                        && chars.get(index + 1).is_some_and(char::is_ascii_digit)))
            {
                index += 1;
            }
        } else if is_ident_start(chars[index]) {
            let start = index;
            index = read_ident(&chars, index);
            if chars.get(index) == Some(&'.')
                && chars.get(index + 1).copied().is_some_and(is_ident_start)
                && let Some(path) = reading(index)
            {
                dots.push(Dot {
                    receiver: start..index,
                    member: index + 1..read_ident(&chars, index + 1),
                    path,
                    plain: !matches!(chars[..start].last(), Some('.' | ':')),
                });
            }
        } else {
            index += 1;
        }
    }
    dots
}

/// The codes of errors about names that do not resolve, where a dot renk read the other way is a
/// likely cause: rustc's, and rust-analyzer's own.
pub const RESOLUTION_ERRORS: &[&str] = &[
    "E0412",
    "E0423",
    "E0425",
    "E0433",
    "E0531",
    "E0532",
    "E0573",
    "E0574",
    "E0599",
    "E0609",
    "E0610",
    "E0615",
    "E0616",
    "unresolved-assoc-item",
    "unresolved-field",
    "unresolved-ident",
    "unresolved-method",
];

/// The [`Dot::hint`] of the first dot of the Renk line `renk` that the character range
/// `columns` of it touches, given `rust`, the Rust line renk wrote for it.
pub fn dot_hint(renk: &str, rust: &str, columns: Range<usize>) -> Option<String> {
    line_dots(renk, rust)
        .iter()
        .filter(|dot| dot.receiver.start <= columns.end && columns.start <= dot.member.end)
        .find_map(|dot| dot.hint(renk))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_dots_tell_paths_from_members_outside_strings_and_comments() {
        let dot = |receiver: Range<usize>, member: Range<usize>, path, plain| Dot {
            receiver,
            member,
            path,
            plain,
        };
        assert_eq!(
            line_dots("    let s = String.nope()", "    let s = String::nope();"),
            [dot(12..18, 19..23, true, true)]
        );
        assert_eq!(
            line_dots("    let n = args.len()", "    let n = args.len();"),
            [dot(12..16, 17..20, false, true)]
        );
        assert_eq!(
            line_dots(
                "let a = std.env.args().skip(1)",
                "let a = std::env::args().skip(1);"
            ),
            [
                dot(8..11, 12..15, true, true),
                dot(12..15, 16..20, true, false)
            ]
        );
        assert_eq!(
            line_dots(
                "    println!(\"{a.b}\", 'x', 1.5, 2.0f64.abs()) // c.d",
                "    println!(\"{}\", a.b, 'x', 1.5, 2.0f64.abs()); // c.d"
            ),
            []
        );
        assert_eq!(
            line_dots("let r = 0..n.len()", "let r = 0..n.len();").len(),
            1
        );
    }

    #[test]
    fn dot_hints_suggest_the_other_reading() {
        let line = "let s = String.nope() + cfg.name.len()";
        let rust = "let s = String::nope() + cfg.name.len();";
        let hints = line_dots(line, rust)
            .iter()
            .map(|dot| dot.hint(line))
            .collect::<Vec<_>>();
        assert_eq!(
            hints,
            [
                Some(
                    "renk read `String.nope` as the path `String::nope`; write `(String).nope` for a value"
                        .to_string()
                ),
                Some(
                    "renk read `cfg.name` as a member of the value `cfg`; write `cfg::name` for a path"
                        .to_string()
                ),
                None,
            ]
        );
    }

    #[test]
    fn script_body_blanks_the_shebang_and_frontmatter_but_keeps_their_lines() {
        let source = "#!/usr/bin/env -S renk script\n---\n[package]\n---\nfn helper() = 1\n";
        assert_eq!(script_body(source).unwrap(), "\n\n\n\nfn helper() = 1\n");
        let output = transpile(&script_body(source).unwrap()).unwrap();
        assert_eq!(output.source_tree[0].source_line, 5);
        assert_eq!(script_body("let x = 1\n").unwrap(), "let x = 1\n");
    }

    #[test]
    fn script_rust_keeps_frontmatter_first_and_drops_the_shebang() {
        let source = "#!/usr/bin/env -S renk script\n---\n[dependencies]\nregex = \"1\"\n---\n\nfn main():\n    println!(\"hi\")\n";

        let rust = script_rust(source, SourceSyntax::Renk, "tool")
            .unwrap()
            .rust;
        assert!(rust.starts_with(
            "---\n[dependencies]\nregex = \"1\"\n\n[package]\nedition = \"2024\"\nname = \"tool\"\n---\n"
        ));
        assert!(!rust.contains("#!/usr/bin/env"));
        assert!(rust.contains("println!(\"hi\");"));
    }

    #[test]
    fn script_top_level_main_returns_the_result_of_the_error_crate_it_depends_on() {
        let main = |dependencies: &str| {
            let source = format!("---\n[dependencies]\n{dependencies}---\nanyhow::bail!(\"no\")\n");
            let rust = script_rust(&source, SourceSyntax::Renk, "tool")
                .unwrap()
                .rust;
            rust.lines()
                .find(|line| line.starts_with("fn main()"))
                .unwrap()
                .to_string()
        };

        assert_eq!(main("anyhow = \"1\"\n"), "fn main() -> anyhow::Result<()> {");
        assert_eq!(main("eyre = \"0.6\"\n"), "fn main() -> eyre::Result<()> {");
        assert_eq!(
            main("color-eyre = \"0.6\"\n"),
            "fn main() -> color_eyre::Result<()> {"
        );
        assert_eq!(
            main("oops = { package = \"anyhow\", version = \"1\" }\n"),
            "fn main() -> oops::Result<()> {"
        );
        assert_eq!(
            main("anyhow-ext = \"1\"\nregex = \"1\"\n"),
            "fn main() -> Result<(), Box<dyn std::error::Error>> {"
        );
        assert_eq!(
            main("[dev-dependencies]\nanyhow = \"1\"\n"),
            "fn main() -> Result<(), Box<dyn std::error::Error>> {"
        );
    }

    #[test]
    fn script_rust_maps_the_main_wrapper_lines_inside_the_script() {
        let source = "#!/usr/bin/env -S renk script\nlet x = 1\nprintln!(\"{x}\")\n";
        let script = script_rust(source, SourceSyntax::Renk, "tool").unwrap();
        assert!(script.rust.contains("Ok(())"), "{}", script.rust);
        assert!(
            script.source_lines.iter().all(|&(line, _)| (1..=3).contains(&line)),
            "{:?}",
            script.source_lines
        );
    }

    #[test]
    fn script_rust_treats_inner_attributes_and_later_dashes_as_body() {
        let source = "#![allow(unused)]\nfn main():\n    let s = \"\n---\n\"\n    print!(\"{s}\")\n";

        let rust = script_rust(source, SourceSyntax::Renk, "tool")
            .unwrap()
            .rust;
        assert!(rust.starts_with(
            "---\n[package]\nedition = \"2024\"\nname = \"tool\"\n---\n#![allow(unused)]"
        ));
        assert!(rust.contains("\n---\n"));
    }

    #[test]
    fn script_rust_accepts_infostrings_and_longer_fences() {
        let source = "#!/usr/bin/env -S renk script\n----cargo\n[package]\nedition = \"2024\"\n---\n----\nfn main() = ()\n";

        let rust = script_rust(source, SourceSyntax::Renk, "tool")
            .unwrap()
            .rust;
        assert!(rust.starts_with("----cargo\n[package]\nedition = \"2024\"\n---\n----\n"));
        assert!(rust.contains("fn main()"));
    }

    #[test]
    fn script_rust_rejects_unclosed_frontmatter() {
        let error = script_rust(
            "---\n[dependencies]\nfn main() = ()\n",
            SourceSyntax::Renk,
            "tool",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("unclosed frontmatter"), "{error}");
    }

    #[test]
    fn script_errors_report_the_physical_script_line() {
        let source = "#!/usr/bin/env -S renk script\n---\n[package]\n---\nfn main():\n\tprintln!(\"tab\")\n";

        let error = script_rust(source, SourceSyntax::Renk, "tool")
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 6"), "{error}");
    }

    #[test]
    fn script_columns_follow_the_characters_renk_wrote_them_from() {
        let script = "    let s = String.nope()";
        let rust = "    let s = String::nope();";
        assert_eq!(script_column(script, rust, 4), 4);
        assert_eq!(script_column(script, rust, rust.find("nope").unwrap()), 19);
        assert_eq!(
            script_column(script, rust, rust.find("::").unwrap() + 1),
            18
        );
        assert_eq!(script_column(script, rust, rust.len() - 1), script.len());

        let script = "    println!(\"{p.nope()} {v.len()}\")";
        let rust = "    println!(\"{} {}\", p.nope(), v.len());";
        let column = script_column(script, rust, rust.find("nope").unwrap());
        assert_eq!(column, script.find("nope").unwrap());
        assert_eq!(script_column("", "}", 0), 0);
    }

    #[test]
    fn script_cache_is_per_source_path_and_keeps_the_stem() {
        let root = Path::new("/cache");
        let first = script_cache_path(root, Path::new("/a/tool.rk"));
        let second = script_cache_path(root, Path::new("/b/tool.rk"));

        assert_ne!(first, second);
        assert_eq!(first.file_name().unwrap(), "tool.rs");
        assert!(first.starts_with("/cache"));
        assert_eq!(first, script_cache_path(root, Path::new("/a/tool.rk")));
        let dotted = script_cache_path(root, Path::new("/a/setup-itch.io.rk"));
        assert_eq!(dotted.file_name().unwrap(), "setup-itch.io.rs");
    }
}
