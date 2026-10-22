//! Single-file scripts: the cargo-script frontmatter, the `fn main` around top-level lines, and
//! the generated Rust that `rusk script` builds and `rusk-lsp` hands to rust-analyzer.

use std::path::{Path, PathBuf};

use similar::{DiffTag, TextDiff};

use crate::{SourceSyntax, TranspileError, to_rust, transpile, wrap_script_statements};

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
/// whether rusk synthesized it, like a closing brace.
#[derive(Debug)]
pub struct ScriptRust {
    pub rust: String,
    pub source_lines: Vec<(usize, bool)>,
    /// Whether rusk named the package, which keeps it apart from other scripts' packages.
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
    let (rust, body_lines) = if syntax == SourceSyntax::Rusk {
        let output = transpile(&body).map_err(error_line)?;
        let lines = (1..=output.rust.lines().count())
            .map(|line| {
                let written = output
                    .source_map
                    .iter()
                    .any(|entry| entry.generated_line == line);
                let source = output.source_line(line).unwrap_or(line);
                match script_line(source) {
                    Some(source) => (source, !written),
                    None => (source, true),
                }
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
    let source_lines = (0..frontmatter.lines().count())
        .map(|offset| ((frontmatter_line + offset).min(body_start.max(1)), false))
        .chain(body_lines)
        .collect();
    Ok(ScriptRust {
        rust: frontmatter + &rust,
        source_lines,
        named_package,
        package: package_name,
    })
}

/// The edition rusk's generated Rust is written for, pinned in scripts that do not name one so
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
/// where the lines match, or where rusk inserted or rewrote text, the script column it replaces.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_rust_keeps_frontmatter_first_and_drops_the_shebang() {
        let source = "#!/usr/bin/env -S rusk script\n---\n[dependencies]\nregex = \"1\"\n---\n\nfn main():\n    println!(\"hi\")\n";

        let rust = script_rust(source, SourceSyntax::Rusk, "tool")
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
            let rust = script_rust(&source, SourceSyntax::Rusk, "tool")
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
    fn script_rust_treats_inner_attributes_and_later_dashes_as_body() {
        let source = "#![allow(unused)]\nfn main():\n    let s = \"\n---\n\"\n    print!(\"{s}\")\n";

        let rust = script_rust(source, SourceSyntax::Rusk, "tool")
            .unwrap()
            .rust;
        assert!(rust.starts_with(
            "---\n[package]\nedition = \"2024\"\nname = \"tool\"\n---\n#![allow(unused)]"
        ));
        assert!(rust.contains("\n---\n"));
    }

    #[test]
    fn script_rust_accepts_infostrings_and_longer_fences() {
        let source = "#!/usr/bin/env -S rusk script\n----cargo\n[package]\nedition = \"2024\"\n---\n----\nfn main() = ()\n";

        let rust = script_rust(source, SourceSyntax::Rusk, "tool")
            .unwrap()
            .rust;
        assert!(rust.starts_with("----cargo\n[package]\nedition = \"2024\"\n---\n----\n"));
        assert!(rust.contains("fn main()"));
    }

    #[test]
    fn script_rust_rejects_unclosed_frontmatter() {
        let error = script_rust(
            "---\n[dependencies]\nfn main() = ()\n",
            SourceSyntax::Rusk,
            "tool",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("unclosed frontmatter"), "{error}");
    }

    #[test]
    fn script_errors_report_the_physical_script_line() {
        let source = "#!/usr/bin/env -S rusk script\n---\n[package]\n---\nfn main():\n\tprintln!(\"tab\")\n";

        let error = script_rust(source, SourceSyntax::Rusk, "tool")
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 6"), "{error}");
    }

    #[test]
    fn script_columns_follow_the_characters_rusk_wrote_them_from() {
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
        let first = script_cache_path(root, Path::new("/a/tool.rsk"));
        let second = script_cache_path(root, Path::new("/b/tool.rsk"));

        assert_ne!(first, second);
        assert_eq!(first.file_name().unwrap(), "tool.rs");
        assert!(first.starts_with("/cache"));
        assert_eq!(first, script_cache_path(root, Path::new("/a/tool.rsk")));
        let dotted = script_cache_path(root, Path::new("/a/setup-itch.io.rsk"));
        assert_eq!(dotted.file_name().unwrap(), "setup-itch.io.rs");
    }
}
