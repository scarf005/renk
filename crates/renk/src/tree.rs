//! The Renk block tree: logical lines nested by indent, and the check of each node in its context.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) line: usize,
    pub(crate) indent: usize,
    /// Source line and byte offset `text` starts at, which `line` and `indent` leave when a
    /// body is lowered or follows a multi-line attribute.
    pub(crate) start: (usize, usize),
    pub(crate) text: String,
    pub(crate) children: Vec<Node>,
    /// Source closing line of a `{` head after its `}`, such as `)?` in `})?`.
    pub(crate) closing: Option<String>,
    pub(crate) comment: Option<String>,
    pub(crate) closing_comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    Root,
    Block,
    Struct,
    Enum,
    Trait,
    Match,
    StructLiteral,
    Macro,
}

pub(crate) fn parse_source_lines(lines: &[SourceLine]) -> Result<(), TranspileError> {
    parse_source_tree(lines).map(|_| ())
}

pub(crate) fn parse_source_tree(lines: &[SourceLine]) -> Result<Vec<Node>, TranspileError> {
    let (nodes, index) = parse_nodes(lines, 0, 0)?;
    if index != lines.len() {
        return Err(TranspileError {
            line: lines[index].line,
            column: lines[index].indent + 1,
            message: "unexpected indentation".to_string(),
        });
    }
    let nodes = absorb_function_branches(nodes);
    validate_nodes(&nodes, Context::Root)?;
    Ok(nodes)
}

/// Moves the `else` branches after a shorthand body that opens a block, as in
/// `fn f(c: bool) -> u8 = if c:` / `    1` / `else:` / `    2`, into the function's children at
/// its indent, where `inline_block_body` takes them.
fn absorb_function_branches(nodes: Vec<Node>) -> Vec<Node> {
    let mut nodes = nodes.into_iter().peekable();
    let mut absorbed = Vec::new();
    while let Some(mut node) = nodes.next() {
        node.children = absorb_function_branches(std::mem::take(&mut node.children));
        if !is_function(strip_block_marker(node.text.trim()).0)
            || inline_block_body(&node).is_none()
        {
            absorbed.push(node);
            continue;
        }
        let mut branches = Vec::new();
        while let Some(next) = nodes.next_if(|next| {
            let next = next.text.trim();
            is_branch(next) || is_comment(next)
        }) {
            branches.push(next);
        }
        // Comments after the last branch stay after the function.
        let trailing = branches
            .iter()
            .rev()
            .take_while(|branch| is_comment(branch.text.trim()))
            .count();
        let after = branches.split_off(branches.len() - trailing);
        node.children.extend(absorb_function_branches(branches));
        absorbed.push(node);
        absorbed.extend(after);
    }
    absorbed
}

fn parse_nodes(
    lines: &[SourceLine],
    mut index: usize,
    indent: usize,
) -> Result<(Vec<Node>, usize), TranspileError> {
    let mut nodes = Vec::<Node>::new();
    while index < lines.len() {
        let line = &lines[index];
        if line.indent < indent {
            break;
        }
        // An `else` continues the `if` head of `let x =` / `    if c:` at that head's indent.
        if line.indent > indent
            && !(is_branch(line.text.trim())
                && nodes
                    .iter()
                    .rev()
                    .find(|node| !is_branch(node.text.trim()))
                    .is_some_and(|head| {
                        let last = head.text.lines().last().unwrap_or_default();
                        head.text.contains('\n')
                            && raw_indent(last) == line.indent
                            && is_trailing_if(strip_block_marker(head.text.trim()).0)
                    }))
        {
            return Err(TranspileError {
                line: line.line,
                column: line.indent + 1,
                message: "indentation must belong to the previous line".to_string(),
            });
        }
        let indent = line.indent;

        index += 1;
        let mut children = Vec::new();
        if index < lines.len() && lines[index].indent > indent {
            let child_indent = lines[index].indent;
            let parsed = parse_nodes(lines, index, child_indent)?;
            children = parsed.0;
            index = parsed.1;
        }
        if !line.raw && line.text.trim() == ":" {
            return Err(TranspileError {
                line: line.line,
                column: line.indent + 1,
                message: "a multi-line header ends its last line with `:`, as in `    T: Debug:`"
                    .to_string(),
            });
        }
        // A block argument such as `, |y|:` continues a chain whose head leaves a group open.
        let argument = is_branch(line.text.trim())
            && nodes
                .iter()
                .rev()
                .find(|node| !is_branch(node.text.trim()) && !is_comment(node.text.trim()))
                .is_some_and(|head| {
                    open_delimiters(strip_block_marker(head.text.trim()).0).is_some_and(|open| {
                        open.iter().any(|delimiter| matches!(delimiter, '(' | '['))
                    })
                });
        if !line.raw && line.text.starts_with([')', ']', ',']) && !argument {
            return Err(TranspileError {
                line: line.line,
                column: line.indent + 1,
                message: "a closing line must follow a block whose head leaves `(` or `[` open, at the head's indent".to_string(),
            });
        }
        let mut closing = None;
        let mut closing_comment = line.closing_comment.clone();
        if let Some(next) = lines.get(index)
            && next.indent == indent
            && !next.raw
            && let Some(tail) = closing_tail(&next.text)
        {
            if !accepts_closing_line(&line.text, &nodes) && !opens_group_block(lines, line) {
                return Err(TranspileError {
                    line: next.line,
                    column: next.indent + 1,
                    message: "a `}` closing line must follow a line ending with `{`, a trailing `match` head, or the last `else` of a `{` chain".to_string(),
                });
            }
            closing = Some(tail.to_string());
            closing_comment = closing_comment.or_else(|| next.comment.clone());
            index += 1;
        }
        let continues_chain = lines
            .get(index)
            .is_some_and(|next| next.indent == indent && is_branch(next.text.trim()));
        if !line.raw
            && (has_explicit_block_opener(&line.text) || is_branch(line.text.trim()))
            && opens_group_block(lines, line)
            && closing.is_none()
            && !line.closed
            && !continues_chain
        {
            return Err(TranspileError {
                line: line.line,
                column: line.indent + 1,
                message: "a block whose head leaves `(` or `[` open ends with a line closing them at the head's indent, such as `)` or `})`".to_string(),
            });
        }
        // Comments indented under a statement, as in `f(x); // a` / `       // b`, follow it.
        let comments = if !has_explicit_block_opener(&line.text)
            && !line.text.trim_end().ends_with("=>")
            && children
                .iter()
                .all(|child| child.children.is_empty() && is_comment(child.text.trim()))
        {
            std::mem::take(&mut children)
        } else {
            Vec::new()
        };
        nodes.push(Node {
            line: line.line,
            indent: line.indent,
            start: line.start,
            text: line.text.clone(),
            children,
            closing,
            comment: line.comment.clone(),
            closing_comment,
        });
        nodes.extend(comments);
    }
    Ok((nodes, index))
}

/// Whether a `}` closing line may follow `text`: a `{` head, a trailing `match` head, or the last
/// `else` branch of a chain whose head (the last non-`else` of `siblings`) ends with `{`.
fn accepts_closing_line(text: &str, siblings: &[Node]) -> bool {
    let text = text.trim();
    text.ends_with('{')
        || is_match(strip_block_marker(text).0)
        || is_trailing_match(strip_block_marker(text).0)
        || is_trailing_if(strip_block_marker(text).0)
        || (is_branch(text)
            && siblings
                .iter()
                .rev()
                .find(|node| !is_branch(node.text.trim()))
                .is_some_and(|head| {
                    let head = strip_block_marker(head.text.trim()).0;
                    head.ends_with('{') || is_trailing_if(head)
                }))
}

/// The part after `}` of a closing line that continues the expression, such as `)?` in `})?`,
/// `, b)` in `}, b)`, `)}.a` in `})}.a`, `)()` in `})()`, or `)` and the `.method()` lines after
/// `})`.
pub(crate) fn closing_tail(text: &str) -> Option<&str> {
    let tail = text.trim().strip_prefix('}')?;
    let rest = tail.trim_start_matches([')', ']', '}']).trim_start();
    // `}) && b` and `} + 1` continue with an operand, and `})()` calls the closure it closes.
    (rest.starts_with(['?', '.'])
        || (tail.starts_with(')') && rest.starts_with(['(', '[']))
        || starts_with_binary_operator(rest)
        || rest
            .strip_prefix(',')
            .is_some_and(|arguments| !arguments.trim().is_empty()))
    .then_some(tail)
}

/// Whether `text` is a closing line such as `})` that the `next` line continues with a chain.
pub(crate) fn closes_before_chain(text: &str, next: Option<&str>, indent: usize) -> bool {
    text.strip_prefix('}')
        .is_some_and(|tail| tail.chars().all(|character| matches!(character, ')' | ']')))
        && next.is_some_and(|next| continues_chain(next, indent))
}

/// Whether the `next` line continues a chain at `indent` with `.method()` or `?`.
pub(crate) fn continues_chain(next: &str, indent: usize) -> bool {
    let chained = next.trim_start();
    raw_indent(next) == indent
        && (chained.starts_with('?') || (chained.starts_with('.') && !chained.starts_with("..")))
}

fn validate_nodes(nodes: &[Node], context: Context) -> Result<(), TranspileError> {
    nodes
        .iter()
        .try_for_each(|node| validate_node(node, context))
}

fn validate_node(node: &Node, context: Context) -> Result<(), TranspileError> {
    let text = node.text.trim();
    if is_comment(text) || lower_attribute(text).is_some() {
        return reject_children(node, "attributes and comments cannot own indented blocks");
    }

    match context {
        Context::Struct => validate_struct_member(node),
        Context::Enum => validate_enum_member(node),
        Context::Trait => validate_trait_member(node),
        Context::Match => validate_match_arm(node),
        Context::StructLiteral => validate_struct_literal_field(node),
        Context::Macro => validate_macro_arm(node),
        Context::Root | Context::Block => validate_general_node(node),
    }
}

fn validate_general_node(node: &Node) -> Result<(), TranspileError> {
    let text = node.text.trim();
    let (text_without_semicolon, _) = strip_explicit_semicolon(text);
    let (block_text, _) = strip_block_marker(text_without_semicolon);
    if is_struct_item(block_text) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::Struct)
    } else if is_enum_item(block_text) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::Enum)
    } else if is_trait_item(block_text) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::Trait)
    } else if is_macro_item(block_text) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::Macro)
    } else if is_function(block_text) {
        validate_function_node(node)
    } else if is_impl_or_mod_item(block_text) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::Block)
    } else if is_match(block_text) || is_trailing_match(strip_open_brace(block_text)) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::Match)
    } else if is_control(block_text) || is_trailing_if(block_text) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::Block)
    } else if looks_like_struct_literal(node) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::StructLiteral)
    } else {
        validate_statement_node(node)
    }
}

fn require_block_opener(node: &Node) -> Result<(), TranspileError> {
    if node.children.is_empty() || has_explicit_block_opener(node.text.trim()) {
        Ok(())
    } else {
        Err(TranspileError {
            line: node.line,
            column: node.indent + 1,
            message: "indented blocks require `:`".to_string(),
        })
    }
}

/// The source position of byte `at` of `text`, which starts at `start` and may span lines.
pub(crate) fn text_start(start: (usize, usize), text: &str, at: usize) -> (usize, usize) {
    match text[..at].rfind('\n') {
        Some(newline) => (start.0 + text[..at].matches('\n').count(), at - newline - 1),
        None => (start.0, start.1 + at),
    }
}

/// `fn f() = match x:` owns its block like `fn f():` whose only body line is `match x:`.
pub(crate) fn inline_block_body(node: &Node) -> Option<Vec<Node>> {
    let (text, has_block) = strip_block_marker(node.text.trim());
    let (_, body) = split_function_body(text)?;
    let body = body.trim();
    let at = (body.as_ptr() as usize)
        .checked_sub(node.text.as_ptr() as usize)
        .filter(|&at| {
            node.text
                .get(at..)
                .is_some_and(|rest| rest.starts_with(body))
        })
        .unwrap_or_default();
    let start = text_start(node.start, &node.text, at);
    // Children at the function's indent are the body's `else` branches.
    let split = node
        .children
        .iter()
        .position(|child| child.indent == node.indent)
        .unwrap_or(node.children.len());
    let (children, branches) = node.children.split_at(split);
    (has_block && !body.is_empty() && !children.is_empty() && !are_continuations(children)).then(
        || {
            std::iter::once(Node {
                line: node.line,
                indent: node.indent + 4,
                start,
                text: format!("{body}:"),
                children: children
                    .iter()
                    .map(|child| offset_node_indent(child, 4))
                    .collect(),
                closing: node.closing.clone(),
                comment: None,
                closing_comment: None,
            })
            .chain(branches.iter().map(|branch| offset_node_indent(branch, 4)))
            .collect()
        },
    )
}

fn validate_function_node(node: &Node) -> Result<(), TranspileError> {
    if let Some(body) = inline_block_body(node) {
        // The lowered body is indented 4 columns past the function.
        return validate_nodes(&body, Context::Block).map_err(|error| TranspileError {
            column: error.column - 4,
            ..error
        });
    }
    let (text, _) = strip_block_marker(node.text.trim());
    let Some((_, body)) = split_function_body(text) else {
        require_block_opener(node)?;
        return validate_nodes(&node.children, Context::Block);
    };
    let body = body.trim();
    if node.children.is_empty() {
        return Ok(());
    }
    if body.is_empty() {
        require_block_opener(node)?;
        return validate_nodes(&node.children, Context::Block);
    }
    if are_continuations(&node.children) {
        return validate_nodes(&node.children, Context::Block);
    }
    Err(TranspileError {
        line: node.line,
        column: node.indent + 1,
        message: "inline function bodies cannot own indented blocks; move the body below `:` or use method-chain continuations".to_string(),
    })
}

fn validate_statement_node(node: &Node) -> Result<(), TranspileError> {
    if node.children.is_empty() {
        return Ok(());
    }

    let (text, _) = strip_explicit_semicolon(node.text.trim());
    let (block_text, _) = strip_block_marker(text);
    if are_continuations(&node.children) || is_multiline_closure_head(text) {
        validate_nodes(&node.children, Context::Block)
    } else if let Some((_, rhs)) = split_once_top_level(text, '=')
        && is_multiline_closure_head(rhs.trim())
    {
        validate_nodes(&node.children, Context::Block)
    } else if is_raw_braced_expr_head(text) || let_else_head(text).is_some() {
        validate_nodes(&node.children, Context::Block)
    } else if looks_like_struct_literal(node) {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::StructLiteral)
    } else if has_block_marker(text) {
        Err(TranspileError {
            line: node.line,
            column: node.indent + 1,
            message: format!("unsupported indented block after `{block_text}`"),
        })
    } else {
        Err(TranspileError {
            line: node.line,
            column: node.indent + 1,
            message: "indented blocks require `:`".to_string(),
        })
    }
}

fn validate_struct_member(node: &Node) -> Result<(), TranspileError> {
    let (text, _) = strip_block_marker(node.text.trim());
    if is_function(text) || is_impl_or_mod_item(text) {
        validate_general_node(node)
    } else {
        reject_children(node, "struct fields cannot own indented blocks")
    }
}

fn validate_enum_member(node: &Node) -> Result<(), TranspileError> {
    require_block_opener(node)?;
    validate_nodes(&node.children, Context::Struct)
}

fn validate_trait_member(node: &Node) -> Result<(), TranspileError> {
    let (text, _) = strip_block_marker(node.text.trim());
    if is_function(text) {
        validate_function_node(node)
    } else if !node.children.is_empty() && are_continuations(&node.children) {
        // A value continued on more indented lines, as in `const K: u8 = A` / `    | B`.
        Ok(())
    } else {
        reject_children(
            node,
            "trait items without bodies cannot own indented blocks",
        )
    }
}

fn validate_match_arm(node: &Node) -> Result<(), TranspileError> {
    let (text, _) = strip_block_marker(node.text.trim());
    if let Some((_, expr)) = split_arrow(text) {
        if expr.trim().is_empty() {
            require_block_opener(node)?;
            validate_nodes(&node.children, Context::Block)
        } else if node.children.is_empty() {
            Ok(())
        } else if has_explicit_block_opener(node.text.trim()) {
            validate_nodes(&node.children, arm_block_context(expr))
        } else if are_continuations(&node.children) {
            validate_nodes(&node.children, Context::Block)
        } else {
            Err(TranspileError {
                line: node.line,
                column: node.indent + 1,
                message: "match arms with inline bodies cannot own indented blocks".to_string(),
            })
        }
    } else {
        require_block_opener(node)?;
        validate_nodes(&node.children, Context::Block)
    }
}

fn validate_macro_arm(node: &Node) -> Result<(), TranspileError> {
    let (text, _) = strip_block_marker(node.text.trim());
    if let Some((_, expr)) = split_arrow(text) {
        if expr.trim().is_empty() {
            // The body is Rust tokens, emitted line by line.
            require_block_opener(node)
        } else if node.children.is_empty() {
            Ok(())
        } else {
            Err(TranspileError {
                line: node.line,
                column: node.indent + 1,
                message: "macro arms with inline bodies cannot own indented blocks".to_string(),
            })
        }
    } else {
        reject_children(node, "macro arms without `=>` cannot own indented blocks")
    }
}

fn validate_struct_literal_field(node: &Node) -> Result<(), TranspileError> {
    reject_children(node, "struct literal fields cannot own indented blocks")
}

fn reject_children(node: &Node, message: &str) -> Result<(), TranspileError> {
    if node.children.is_empty() {
        Ok(())
    } else {
        Err(TranspileError {
            line: node.line,
            column: node.indent + 1,
            message: message.to_string(),
        })
    }
}
