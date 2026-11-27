//! Transpiles Renk to Rust and back.
//!
//! The `cli`, `lsp` and `web` features build the binaries and their dependencies. To use only the
//! library, depend on `renk = { version = "...", default-features = false }`.

use std::fmt;

mod classify;
mod emit;
mod from_rust;
mod lines;
mod lower;
mod macros;
#[cfg(all(not(target_arch = "wasm32"), feature = "project-files"))]
pub mod project;
mod rust_layout;
mod scope;
#[cfg(all(not(target_arch = "wasm32"), feature = "project-files"))]
pub mod script;
mod text;
mod tree;

use classify::*;
use emit::*;
use from_rust::*;
use lines::*;
use lower::*;
use macros::*;
use scope::*;
use text::*;
use tree::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMapEntry {
    pub source_line: usize,
    pub generated_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMapNode {
    pub kind: String,
    pub source_line: usize,
    pub source_column: usize,
    pub source_text: String,
    pub generated_start_line: usize,
    pub generated_end_line: usize,
    pub children: Vec<SourceMapNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranspileOutput {
    pub rust: String,
    pub source_map: Vec<SourceMapEntry>,
    pub source_tree: Vec<SourceMapNode>,
}

impl TranspileOutput {
    /// The source line that produced a 1-based generated line, including inserted closing lines.
    pub fn source_line(&self, generated_line: usize) -> Option<usize> {
        self.source_map
            .iter()
            .find(|entry| entry.generated_line == generated_line)
            .map(|entry| entry.source_line)
            .or_else(|| {
                deepest_source_node(&self.source_tree, generated_line).map(|node| node.source_line)
            })
    }

    /// For each 1-based generated line, its source line and whether renk synthesized it, like a
    /// closing brace. The other lines come from text on their source line, as the body after `=`
    /// of a one-line function does.
    pub fn line_origins(&self) -> Vec<(usize, bool)> {
        self.rust
            .lines()
            .zip(1..)
            .map(|(text, line)| {
                let synthesized = text.trim_start().starts_with('}')
                    && !self
                        .source_map
                        .iter()
                        .any(|entry| entry.generated_line == line);
                (self.source_line(line).unwrap_or(line), synthesized)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranspileError {
    pub line: usize,
    /// The 1-based character column to look at on `line`.
    pub column: usize,
    pub message: String,
}

impl fmt::Display for TranspileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for TranspileError {}

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use wasm_bindgen::prelude::*;

    use super::{TranspileError, format_source, rust_to_renk, source_map_json, transpile};

    #[wasm_bindgen]
    pub fn format_renk(source: &str) -> Result<String, JsValue> {
        format_source(source).map_err(error_to_js_value)
    }

    #[wasm_bindgen]
    pub fn transpile_to_rust(source: &str) -> Result<String, JsValue> {
        transpile(source)
            .map(|output| output.rust)
            .map_err(error_to_js_value)
    }

    #[wasm_bindgen]
    pub fn transpile_syntax_tree_json(source: &str) -> Result<String, JsValue> {
        transpile(source)
            .map(|output| source_map_json(&output))
            .map_err(error_to_js_value)
    }

    #[wasm_bindgen]
    pub fn convert_rust_to_renk(source: &str) -> Result<String, JsValue> {
        rust_to_renk(source).map_err(error_to_js_value)
    }

    fn error_to_js_value(error: TranspileError) -> JsValue {
        JsValue::from_str(&error.to_string())
    }
}

pub fn transpile(source: &str) -> Result<TranspileOutput, TranspileError> {
    let mut output = on_large_stack(|| transpile_source(without_bom(source)))?;
    if source.starts_with('\u{feff}') {
        shift_line_one_columns(&mut output.source_tree);
    }
    Ok(output)
}

/// Adds the byte order mark, one UTF-16 unit, to the columns of the nodes on line 1.
fn shift_line_one_columns(nodes: &mut [SourceMapNode]) {
    for node in nodes.iter_mut().filter(|node| node.source_line == 1) {
        node.source_column += 1;
        shift_line_one_columns(&mut node.children);
    }
}

/// `source` without the byte order mark some editors write at its start.
fn without_bom(source: &str) -> &str {
    source.strip_prefix('\u{feff}').unwrap_or(source)
}

fn transpile_source(source: &str) -> Result<TranspileOutput, TranspileError> {
    with_value_scopes(|| transpile_lines(source))
}

/// `convert` run on a thread with a large stack, as the Rust parsers and the Renk block tree
/// recurse once per nesting level; WebAssembly has no threads and keeps its own stack, and a
/// process limit that refuses the thread leaves `convert` on the caller's stack.
#[cfg(not(target_arch = "wasm32"))]
fn on_large_stack<T: Send>(convert: impl FnOnce() -> T + Send) -> T {
    let convert = std::sync::Mutex::new(Some(convert));
    let take = || convert.lock().unwrap().take().unwrap();
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn_scoped(scope, || take()())
            .map(|thread| {
                thread
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
    })
    .unwrap_or_else(|_| take()())
}

#[cfg(target_arch = "wasm32")]
fn on_large_stack<T>(convert: impl FnOnce() -> T) -> T {
    convert()
}

pub fn format_source(source: &str) -> Result<String, TranspileError> {
    on_large_stack(|| format_lines(without_bom(source)))
}

fn format_lines(source: &str) -> Result<String, TranspileError> {
    let lines = source_lines(source)?;
    parse_source_lines(&lines)?;

    let in_literal = lines_ending_in_literal(source);
    Ok(ensure_trailing_newline(
        source
            .lines()
            .enumerate()
            .map(|(index, line)| {
                if in_literal[index] {
                    line.to_string()
                } else if index > 0 && in_literal[index - 1] {
                    // The line starts inside the literal, so its indentation is literal content.
                    line.trim_end().to_string()
                } else {
                    format_line(line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end_matches('\n'),
    ))
}

/// Formats Renk as `rustfmt` lays out Rust: the generated Rust goes through `rustfmt`, which
/// returns `None` when it cannot format it, and back through [`rust_to_renk`]. The result is kept
/// only when its Rust formats to the same text as the source's, up to inlined format arguments;
/// otherwise each top-level item whose formatted lines fail that check keeps its lines as
/// [`format_source`] writes them. A script keeps its `#!` line and frontmatter, and its top-level
/// statements are formatted in the `fn main` that runs them.
#[cfg(all(not(target_arch = "wasm32"), feature = "project-files"))]
pub fn format_source_with(
    source: &str,
    rustfmt: &dyn Fn(&str) -> Option<String>,
) -> Result<String, TranspileError> {
    let source = without_bom(source);
    let parts = script::script_parts(source)?;
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let body = lines[parts.body_start..].concat();
    let base = format_source(&body).map_err(|mut error| {
        error.line += parts.body_start;
        error
    })?;
    let base = base.trim_start_matches('\n');
    let wrapped = (parts.body_start > 0)
        .then(|| wrap_script_statements(base, SourceSyntax::Renk, None))
        .flatten();
    let formatted = match wrapped {
        Some((wrapped, _)) => unwrap_script_statements(&format_items(&wrapped, rustfmt))
            .filter(|body| {
                wrap_script_statements(body, SourceSyntax::Renk, None)
                    .and_then(|(rewrapped, _)| canonical_rust(&rewrapped, rustfmt))
                    == canonical_rust(&wrapped, rustfmt)
            })
            .unwrap_or_else(|| base.to_string()),
        None => format_items(base, rustfmt),
    };
    // A script's blank lines after its header stay.
    let gap = if parts.body_start > 0 {
        body.lines()
            .take_while(|line| line.trim().is_empty())
            .count()
    } else {
        0
    };
    Ok(lines[..parts.body_start].concat() + &"\n".repeat(gap) + &formatted)
}

/// `rust` formatted by `rustfmt` for edition 2024, run in `dir` to read the `rustfmt.toml` there;
/// `Ok(None)` when rustfmt rejects it, and an error that tells how to install rustfmt when it
/// cannot run.
#[cfg(not(target_arch = "wasm32"))]
pub fn rustfmt(rust: &str, dir: &std::path::Path) -> Result<Option<String>, String> {
    use std::process::{Command, Stdio};
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            format!("cannot run rustfmt ({error}); install it with `rustup component add rustfmt`")
        })?;
    let formatted = || {
        std::io::Write::write_all(&mut child.stdin.take()?, rust.as_bytes()).ok()?;
        let output = child.wait_with_output().ok()?;
        (output.status.success()).then(|| String::from_utf8(output.stdout).ok())?
    };
    Ok(formatted())
}

/// The generated Rust of `source` as `rustfmt` formats it.
#[cfg(all(not(target_arch = "wasm32"), feature = "project-files"))]
fn formatted_rust(source: &str, rustfmt: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    rustfmt(&transpile(source).ok()?.rust)
}

/// [`formatted_rust`] with format arguments inlined, as `"{}", x` and `"{x}"` are the same.
#[cfg(all(not(target_arch = "wasm32"), feature = "project-files"))]
fn canonical_rust(source: &str, rustfmt: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    formatted_rust(source, rustfmt).map(|rust| canonical_format_macros(&rust))
}

/// `source` formatted as a whole, or else item by item; see [`format_source_with`].
#[cfg(all(not(target_arch = "wasm32"), feature = "project-files"))]
fn format_items(source: &str, rustfmt: &dyn Fn(&str) -> Option<String>) -> String {
    let format = |text: &str| rust_to_renk(&formatted_rust(text, rustfmt)?).ok();
    let expected = canonical_rust(source, rustfmt);
    if let Some(formatted) = format(source)
        && expected.is_some()
        && canonical_rust(&formatted, rustfmt) == expected
    {
        return formatted;
    }
    // An item starts at a line without indentation after a blank line, and keeps the blank lines
    // after it.
    let lines = source.lines().collect::<Vec<_>>();
    let in_literal = lines_ending_in_literal(source);
    let mut items = Vec::<(String, String)>::new();
    for (index, line) in lines.iter().enumerate() {
        let starts = index > 0
            && !in_literal[index - 1]
            && lines[index - 1].is_empty()
            && !line.is_empty()
            && raw_indent(line) == 0;
        match items.last_mut() {
            Some((_, gap)) if line.is_empty() && !in_literal[index - 1] => gap.push('\n'),
            Some((item, gap)) if !starts => {
                item.push_str(&std::mem::take(gap));
                item.push_str(line);
                item.push('\n');
            }
            _ => items.push((format!("{line}\n"), String::new())),
        }
    }
    let join = |items: &[(String, String)]| {
        items
            .iter()
            .map(|(item, gap)| format!("{item}{gap}"))
            .collect::<String>()
    };
    for index in 0..items.len() {
        let Some(formatted) = format(&items[index].0) else {
            continue;
        };
        let mut trial = items.clone();
        trial[index].0 = formatted;
        let same = match &expected {
            Some(_) => canonical_rust(&join(&trial), rustfmt) == expected,
            // Without Rust for the whole source, the item is compared alone.
            None => canonical_rust(&items[index].0, rustfmt)
                .is_some_and(|rust| canonical_rust(&trial[index].0, rustfmt) == Some(rust)),
        };
        if same {
            items = trial;
        }
    }
    ensure_trailing_newline(join(&items).trim_end_matches('\n'))
}

/// The top-level lines of a script that [`wrap_script_statements`] wrapped, from its formatted
/// form; `None` when the wrapper is no longer there.
#[cfg(all(not(target_arch = "wasm32"), feature = "project-files"))]
pub(crate) fn unwrap_script_statements(wrapped: &str) -> Option<String> {
    let lines = wrapped.lines().collect::<Vec<_>>();
    let in_literal = lines_ending_in_literal(wrapped);
    let starts_code = |index: usize| index == 0 || !in_literal[index - 1];
    let header = (0..lines.len())
        .find(|&index| starts_code(index) && lines[index].starts_with("fn main()"))?;
    let end = lines.len().checked_sub(1)?;
    if end <= header || lines[end] != "    Ok(())" {
        return None;
    }
    let body = (header + 1..end)
        .map(|index| match lines[index] {
            line if !starts_code(index) || line.is_empty() => Some(line),
            line => line.strip_prefix("    "),
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ensure_trailing_newline(
        &lines[..header]
            .iter()
            .copied()
            .chain(body)
            .collect::<Vec<_>>()
            .join("\n"),
    ))
}

/// A script without a top-level `fn main` with its lines wrapped in a `fn main` returning
/// `Result`, so top-level statements run in order and can use `?`, and the 1-based line of the
/// inserted header. `result` names a `Result` alias with a default error type, such as
/// `anyhow::Result`, to return instead of `Result<(), Box<dyn std::error::Error>>`. Leading blank,
/// comment, and `#![...]` lines stay in front of the header.
pub fn wrap_script_statements(
    source: &str,
    syntax: SourceSyntax,
    result: Option<&str>,
) -> Option<(String, usize)> {
    let lines = source.lines().collect::<Vec<_>>();
    let in_literal = lines_ending_in_literal(source);
    let starts_code = |index: usize| index == 0 || !in_literal[index - 1];
    let declares_main = |line: &str| {
        let line = without_visibility(line);
        let line = line.strip_prefix("async ").unwrap_or(line);
        line.strip_prefix("fn main")
            .is_some_and(|rest| rest.trim_start().starts_with('('))
    };
    if (0..lines.len()).any(|index| starts_code(index) && declares_main(lines[index])) {
        return None;
    }
    let header = (0..lines.len())
        .find(|&index| {
            let line = lines[index].trim();
            !(line.is_empty() || line.starts_with("//") || line.starts_with("#!["))
        })
        .unwrap_or(lines.len());
    let (left, right, open, close) = match syntax {
        SourceSyntax::Renk => ("[", "]", ":", ""),
        _ => ("<", ">", " {", "\n}"),
    };
    let returns = result.map_or_else(
        || format!("Result{left}(), Box{left}dyn std::error::Error{right}{right}"),
        |result| format!("{result}{left}(){right}"),
    );
    let open = format!("fn main() -> {returns}{open}");
    let body = (header..lines.len()).map(|index| {
        if starts_code(index) && !lines[index].is_empty() {
            format!("    {}", lines[index])
        } else {
            lines[index].to_string()
        }
    });
    let wrapped = lines[..header]
        .iter()
        .map(|line| line.to_string())
        .chain([open])
        .chain(body)
        .chain([format!("    Ok(()){close}")])
        .collect::<Vec<_>>()
        .join("\n");
    Some((ensure_trailing_newline(&wrapped), header + 1))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSyntax {
    Renk,
    Rust,
}

pub fn to_rust(source: &str, syntax: SourceSyntax) -> Result<String, TranspileError> {
    match syntax {
        SourceSyntax::Renk => transpile(source).map(|output| output.rust),
        SourceSyntax::Rust => Ok(ensure_trailing_newline(source)),
    }
}

pub fn to_renk(source: &str, syntax: SourceSyntax) -> Result<String, TranspileError> {
    match syntax {
        SourceSyntax::Renk => Ok(ensure_trailing_newline(source)),
        SourceSyntax::Rust => rust_to_renk(source),
    }
}

pub fn rust_to_renk(source: &str) -> Result<String, TranspileError> {
    convert_rust_to_renk(source, false)
}

/// Like [`rust_to_renk`], but also keeps idiomatic rewrites that change only the layout of the
/// generated Rust, so converting back no longer reproduces the original line structure.
pub fn rust_to_renk_aggressive(source: &str) -> Result<String, TranspileError> {
    convert_rust_to_renk(source, true)
}

pub fn source_map_json(output: &TranspileOutput) -> String {
    format!(
        "{{\n  \"version\": 2,\n  \"format\": \"renk.syntax-tree\",\n  \"tree\": [\n{}\n  ]\n}}\n",
        source_map_nodes_json(&output.source_tree, 2)
    )
}

fn source_map_nodes_json(nodes: &[SourceMapNode], indent: usize) -> String {
    nodes
        .iter()
        .map(|node| source_map_node_json(node, indent))
        .collect::<Vec<_>>()
        .join(",\n")
}

fn source_map_node_json(node: &SourceMapNode, indent: usize) -> String {
    let pad = spaces(indent * 2);
    let child_pad = spaces((indent + 1) * 2);
    let children = if node.children.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[\n{}\n{child_pad}]",
            source_map_nodes_json(&node.children, indent + 2)
        )
    };

    format!(
        "{pad}{{\n{child_pad}\"kind\": \"{}\",\n{child_pad}\"source\": {{ \"line\": {}, \"column\": {}, \"text\": \"{}\" }},\n{child_pad}\"generated\": {{ \"start_line\": {}, \"end_line\": {} }},\n{child_pad}\"children\": {}\n{pad}}}",
        escape_json(&node.kind),
        node.source_line,
        node.source_column,
        escape_json(&node.source_text),
        node.generated_start_line,
        node.generated_end_line,
        children
    )
}

fn escape_json(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests;
