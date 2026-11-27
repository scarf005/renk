//! The Rust lines and the source map for each node of the Renk block tree.

use super::*;

#[derive(Debug, Default)]
pub(crate) struct Emitter {
    pub(crate) lines: Vec<String>,
    pub(crate) source_map: Vec<SourceMapEntry>,
    pub(crate) source_tree: Vec<SourceMapNode>,
    pub(crate) tree_stack: Vec<SourceMapNode>,
    /// Trailing source comments by generated line, appended once every line is final.
    pub(crate) comments: Vec<(usize, String)>,
    pub(crate) source_lines: Vec<String>,
    /// The last source line a node began on.
    pub(crate) last_node_line: usize,
    /// The source line of the attribute just pushed.
    pub(crate) attribute_line: Option<usize>,
    /// Generated lines, numbered from 0, split from a neighbour: a control header's `{` on a line
    /// of its own, which an empty body's `}` joins, as in `{}`, and a block argument, which joins the
    /// `}` before it, as in `}, |y| {`.
    pub(crate) joining_lines: Vec<usize>,
}

pub(crate) fn transpile_lines(source: &str) -> Result<TranspileOutput, TranspileError> {
    let lines = source_lines(source)?;
    let nodes = parse_source_tree(&lines)?;

    let mut emitter = Emitter {
        source_lines: source.lines().map(str::to_string).collect(),
        ..Emitter::default()
    };
    let modules = module_names(&nodes);
    MODULE_VALUES.set(
        module_value_names(&nodes)
            .into_iter()
            .filter(|name| !modules.contains(name))
            .collect(),
    );
    // A file, like an `impl` or `mod` body, has no tail value, so its last `name!(...)` gets a `;`.
    emit_sequence(&nodes, Context::Root, false, &mut emitter);
    for (line, comment) in &emitter.comments {
        emitter.lines[*line].push_str(comment);
    }
    emitter.join_split_lines();
    Ok(TranspileOutput {
        rust: ensure_trailing_newline(&emitter.lines.join("\n")),
        source_map: emitter.source_map,
        source_tree: emitter.source_tree,
    })
}

fn emit_nodes(nodes: &[Node], context: Context, emitter: &mut Emitter) {
    emit_sequence(nodes, context, true, emitter);
}

/// Emits sibling nodes. An `else` sibling continues the previous braced expression, so the
/// closing suffix of the chain head (such as `);` in `let x = f(if c {`) moves to the last
/// branch of the chain.
pub(crate) fn emit_sequence(
    nodes: &[Node],
    context: Context,
    tail_is_value: bool,
    emitter: &mut Emitter,
) {
    with_value_scope(BLOCK_BINDINGS.take(), || {
        emit_scoped_sequence(nodes, context, tail_is_value, emitter)
    });
}

fn emit_scoped_sequence(
    nodes: &[Node],
    context: Context,
    tail_is_value: bool,
    emitter: &mut Emitter,
) {
    let mut chain: Option<(String, bool)> = None;
    let mut chain_closures = Vec::new();
    let mut branch_is_value = false;
    let is_cfg = |index: usize| nodes[index].text.trim().starts_with("#[cfg");
    let continues_chain = |index: usize| {
        let text = nodes[index].text.trim();
        is_branch(text) || is_comment(text)
    };
    // Per node, each found in one pass so a long run of comments stays linear: the head of the
    // `else` chain ending there, the last of the branches and comments after it, and whether only
    // comments follow it.
    let mut chain_heads = (0..nodes.len()).collect::<Vec<_>>();
    let mut run_ends = chain_heads.clone();
    let mut only_comments_after = vec![true; nodes.len()];
    for index in 1..nodes.len() {
        if continues_chain(index) {
            chain_heads[index] = chain_heads[index - 1];
        }
    }
    for index in (0..nodes.len().saturating_sub(1)).rev() {
        if continues_chain(index + 1) {
            run_ends[index] = run_ends[index + 1];
        }
        only_comments_after[index] =
            is_comment(nodes[index + 1].text.trim()) && only_comments_after[index + 1];
    }
    let chain_head = |index: usize| chain_heads[index];
    // Comments after the tail leave it the tail.
    let ends_at = |index: usize| only_comments_after[index];
    // Nodes `start..=end` are one of several tails, each under its own `#[cfg(...)]`, when only
    // other alternatives follow them.
    let is_attribute = |index: usize| nodes[index].text.trim().starts_with("#[");
    // Attributes before `index`, as in `#[cfg(unix)]` / `#[allow(unused)]`, hold a `cfg`.
    let has_cfg = |index: usize| {
        (0..index)
            .rev()
            .take_while(|&before| is_attribute(before))
            .any(is_cfg)
    };
    let is_cfg_tail = |start: usize, end: usize| {
        has_cfg(start)
            && (end + 1..nodes.len())
                .find(|&next| !is_comment(nodes[next].text.trim()))
                .is_some_and(is_cfg)
            && (end + 1..nodes.len()).all(|later| {
                let text = nodes[later].text.trim();
                is_attribute(later) || has_cfg(later) || is_comment(text) || is_branch(text)
            })
    };
    // A cfg-selected tail that is not a block-like expression, as in `#[cfg(unix)]` / `v.len()`.
    // A bare block such as `{` / `    v.len()` is one already.
    let is_cfg_expression = |index: usize| {
        let (head, _) = strip_block_marker(nodes[index].text.trim());
        let is_block = head.starts_with(['{', '#']);
        let head = strip_open_brace(head);
        tail_is_value
            && context == Context::Block
            && !looks_like_item_header_without_body(head)
            && !["use", "const", "static", "type", "extern"]
                .iter()
                .any(|keyword| starts_item(head, keyword))
            && !is_macro_item(head)
            && has_cfg(index)
            && (ends_at(index) || is_cfg_tail(index, index))
            && !is_attribute(index)
            && !is_comment(head)
            && !is_control(head)
            && !is_match(head)
            && !is_block
            && !nodes[index + 1..]
                .iter()
                .find(|next| !is_comment(next.text.trim()))
                .is_some_and(|next| is_branch(next.text.trim()))
    };
    for (index, node) in nodes.iter().enumerate() {
        let is_last = tail_is_value && (ends_at(index) || is_cfg_tail(chain_head(index), index));
        let head = node.text.trim();
        if chain.is_some() && is_comment(head) {
            // A comment between `}` and `else` stays inside the chain.
            emit_node(node, context, false, emitter);
            continue;
        }
        if !is_branch(head) {
            // A chain's branches are values when the chain is the tail or its head uses them, as
            // in `let x = if c {`.
            let (head, _) = strip_explicit_semicolon(head);
            let end = run_ends[index];
            branch_is_value = !is_control(strip_block_marker(head).0)
                || (tail_is_value && (ends_at(end) || is_cfg_tail(index, end)));
        }
        let continues = !is_comment(head)
            && nodes[index + 1..]
                .iter()
                .find(|next| !is_comment(next.text.trim()))
                .is_some_and(|next| is_branch(next.text.trim()));
        let in_chain = chain.is_some() || continues;
        // The `else` of an arm such as `p => if c:` is a block branch, not an arm.
        let node_context = if context == Context::Match && is_branch(head) {
            Context::Block
        } else {
            context
        };
        let first = emitter.lines.len();
        let is_value = if in_chain { branch_is_value } else { is_last };
        // An `else` branch stays inside the closures its chain's head opened; a scope of its
        // own would drop the names it binds for later siblings, but a branch binds none.
        if chain.is_some() {
            with_value_scope(chain_closures.clone(), || {
                emit_node(node, node_context, is_value, emitter)
            });
        } else {
            emit_node(node, node_context, is_value, emitter);
        }
        // Rust takes attributes only on a statement or a block, so the expression becomes one.
        if is_cfg_expression(index) && emitter.lines.len() > first {
            let line = &mut emitter.lines[first];
            line.insert_str(raw_indent(line), "{ ");
            if let Some(last) = emitter.lines.last_mut() {
                last.push_str(" }");
            }
        }
        if !continues && chain.is_none() {
            continue;
        }
        let Some(closing) = emitter.lines.last_mut() else {
            continue;
        };
        // A branch ends with a `}` line, or is a single line such as `else if c then a else b`
        // or the value of `let pattern = value` before a `let ... else`.
        let brace = closing
            .find('}')
            .filter(|brace| closing[..*brace].trim().is_empty());
        let end = brace.map_or(closing.trim_end().trim_end_matches(';').len(), |brace| {
            brace + 1
        });
        if continues {
            let suffix = closing.split_off(end);
            if chain.is_none() {
                let head = node.text.trim();
                let (suffix, _) = strip_explicit_semicolon(&suffix);
                chain = Some((suffix.to_string(), is_let(head) || is_assignment(head)));
                chain_closures = open_closure_parameters(strip_block_marker(head).0);
            }
        } else if let Some((suffix, is_statement)) = chain.take() {
            let (suffix, has_explicit_semicolon) = match &node.closing {
                // The tail closes the closures the chain's head opened.
                Some(tail) => {
                    let outer = OPEN_CLOSURES.replace(std::mem::take(&mut chain_closures));
                    let suffix = closing_suffix(tail);
                    OPEN_CLOSURES.set(outer);
                    suffix
                }
                None => (suffix, false),
            };
            closing.truncate(end);
            if context == Context::Match {
                // An arm chain ends with its closing delimiters and `,`.
                closing.push_str(suffix.trim_end_matches(','));
                closing.push(',');
            } else {
                closing.push_str(&suffix);
                // Branches ending with statements leave the chain `()`, a statement already.
                if is_statement || has_explicit_semicolon || (!is_last && branch_is_value) {
                    closing.push(';');
                }
            }
        }
    }
}

fn emit_node(node: &Node, context: Context, is_last: bool, emitter: &mut Emitter) {
    emitter.begin_node(node, classify_node(node, context));
    let head = emitter.lines.len();
    let bindings = node_bindings(&node.text, context, !node.children.is_empty());
    let header = strip_block_marker(node.text.trim()).0;
    // Items such as a nested `fn` do not see the enclosing function's locals, only the const
    // generics of an enclosing `impl` or `trait`.
    let outer = is_function(header).then(|| {
        let generics = ITEM_GENERICS.take();
        (VALUE_SCOPES.replace(vec![generics.clone()]), generics)
    });
    let outer_generics = (is_impl_or_mod_item(header) || is_trait_item(header))
        .then(|| ITEM_GENERICS.replace(bindings.own.clone()));
    let outer_closures = OPEN_CLOSURES.replace(open_closure_parameters(header));
    with_value_scope(bindings.own, || {
        BLOCK_BINDINGS.set(bindings.block);
        emit_node_body(node, context, is_last, emitter);
        BLOCK_BINDINGS.take();
    });
    OPEN_CLOSURES.set(outer_closures);
    if let Some(generics) = outer_generics {
        ITEM_GENERICS.set(generics);
    }
    if let Some((scopes, generics)) = outer {
        VALUE_SCOPES.set(scopes);
        ITEM_GENERICS.set(generics);
    }
    VALUE_SCOPES.with_borrow_mut(|scopes| {
        if let Some(scope) = scopes.last_mut() {
            scope.extend(bindings.siblings);
        }
    });
    if let Some(comment) = &node.comment
        && let Some(last) = emitter.lines.len().checked_sub(1)
    {
        // The comment follows the last physical line of the node's own head.
        let line = (head + node.text.matches('\n').count()).min(last);
        emitter.comments.push((line, comment.clone()));
    }
    // The comment of the line closing the node, after the node's last line even when that line
    // closes a single-line `else` rather than a `}` block.
    if let Some(comment) = &node.closing_comment
        && let Some(last) = emitter.lines.len().checked_sub(1)
    {
        emitter.comments.push((last, comment.clone()));
    }
    emitter.end_node();
}

fn emit_node_body(node: &Node, context: Context, is_last: bool, emitter: &mut Emitter) {
    let text = node.text.trim();
    if is_comment(text) {
        emitter.push(node, format!("{}{}", spaces(node.indent), text));
        return;
    }
    if let Some(attribute) = lower_attribute(text) {
        emitter.push(node, format!("{}{}", spaces(node.indent), attribute));
        return;
    }

    match context {
        Context::Struct => emit_struct_member(node, emitter),
        Context::Enum => emit_enum_member(node, emitter),
        Context::Trait => emit_trait_member(node, emitter),
        Context::Match => emit_match_arm(node, emitter),
        Context::StructLiteral => emit_struct_literal_field(node, emitter),
        Context::Macro => emit_macro_arm(node, emitter),
        Context::Root | Context::Block => emit_general(node, context, is_last, emitter),
    }
}

fn classify_node(node: &Node, context: Context) -> String {
    let text = node.text.trim();
    if is_comment(text) {
        "comment".to_string()
    } else if lower_attribute(text).is_some() {
        "attribute".to_string()
    } else if context == Context::Match {
        "match_arm".to_string()
    } else if context == Context::StructLiteral {
        "field".to_string()
    } else if context == Context::Macro {
        "macro_arm".to_string()
    } else if matches!(context, Context::Struct | Context::Enum) && !is_function(text) {
        "member".to_string()
    } else if is_struct_item(text) {
        "struct".to_string()
    } else if is_enum_item(text) {
        "enum".to_string()
    } else if is_trait_item(text) {
        "trait".to_string()
    } else if is_macro_item(text) {
        "macro".to_string()
    } else if is_function(text) {
        "function".to_string()
    } else if starts_item(text, "impl") {
        "impl".to_string()
    } else if starts_item(text, "mod") {
        "module".to_string()
    } else if is_match(text) {
        "match".to_string()
    } else if is_inline_if_expression(text) {
        "if_expression".to_string()
    } else if is_control(text) {
        "control".to_string()
    } else if looks_like_struct_literal(node) || is_inline_struct_literal(text) {
        "struct_literal".to_string()
    } else if is_let(text) {
        "let".to_string()
    } else {
        let (text, _) = strip_explicit_semicolon(text);
        classify_expression(text)
    }
}

fn classify_expression(text: &str) -> String {
    if is_inline_if_expression(text) {
        "if_expression".to_string()
    } else if is_inline_struct_literal(text) {
        "struct_literal".to_string()
    } else if is_assignment(text) {
        "assignment".to_string()
    } else if is_jump_statement(text) {
        "jump".to_string()
    } else {
        "expression".to_string()
    }
}

fn emit_general(node: &Node, context: Context, is_last: bool, emitter: &mut Emitter) {
    let text = node.text.trim();
    let (block_text, _) = strip_block_marker(text);
    if is_struct_item(block_text) {
        emit_braced_item(
            node,
            Context::Struct,
            emitter,
            lower_field_or_variant(block_text),
        );
    } else if is_enum_item(block_text) {
        emit_braced_item(node, Context::Enum, emitter, lower_signature(block_text));
    } else if is_trait_item(block_text) {
        emit_braced_item(node, Context::Trait, emitter, lower_signature(block_text));
    } else if is_macro_item(block_text) && node.children.is_empty() {
        // A `macro_rules!` written on its lines, as in `macro_rules! m(...);`, keeps its tokens.
        emitter.push(node, format!("{}{text}", spaces(node.indent)));
    } else if is_macro_item(block_text) {
        emit_braced_item(node, Context::Macro, emitter, lower_signature(block_text));
    } else if is_function(block_text) {
        emit_function(node, emitter);
    } else if starts_item(strip_explicit_semicolon(text).0, "mod")
        && node.children.is_empty()
        && !has_block_marker(text)
        && !text.ends_with('}')
    {
        // A module declared in its own file.
        let (text, _) = strip_explicit_semicolon(text);
        emitter.push(
            node,
            format!("{}{};", spaces(node.indent), lower_signature(text)),
        );
    } else if is_impl_or_mod_item(block_text) {
        let header = lower_signature(block_text);
        emit_braced_item_statement(node, Context::Block, emitter, header, false, false);
    } else {
        let (text, has_explicit_semicolon) = strip_explicit_semicolon(text);
        let (block_text, _) = strip_block_marker(text);
        if is_match(block_text) && node.closing.is_none() {
            emit_braced_item_statement(
                node,
                Context::Match,
                emitter,
                lower_expr(block_text),
                has_explicit_semicolon || (!is_last && !has_unit_arms(node)),
                true,
            );
        } else if is_match(block_text)
            // A brace `match`, as in `let a = match o {`, too.
            || is_trailing_match(strip_open_brace(block_text))
            || (!node.children.is_empty() && is_trailing_if(block_text))
        {
            let block_text = strip_open_brace(block_text);
            let indent = spaces(node.indent);
            emitter.push(node, format!("{}{} {{", indent, lower_expr(block_text)));
            // The `}` lines up with the head's last line, as in `let x =` / `    if c {`, or with
            // the head itself after a `{` on a line of its own.
            let indent = if opens_on_own_line(node) {
                spaces(node.indent)
            } else {
                spaces(raw_indent(node.text.lines().last().unwrap_or_default()).max(node.indent))
            };
            let context = if is_match(block_text) || is_trailing_match(block_text) {
                Context::Match
            } else {
                Context::Block
            };
            emit_nodes(&node.children, context, emitter);
            let is_statement = is_let(block_text) || is_assignment(block_text);
            let (closing, closing_semicolon) = node
                .closing
                .as_deref()
                .map_or_else(|| (closing_delimiters(block_text), false), closing_suffix);
            emitter.push_generated(format!(
                "{}}}{}{}",
                indent,
                closing,
                if is_statement || has_explicit_semicolon || closing_semicolon || !is_last {
                    ";"
                } else {
                    ""
                }
            ));
        } else if is_control(block_text)
            // A one-line `else` an operator continues, as in `else { 0 } + 1`, too.
            && (block_text.ends_with('}') || block_text.starts_with("else {"))
            && node.children.is_empty()
            && open_delimiters(block_text).is_some_and(|open| open.is_empty())
        {
            // A one-line body, as in `while step() {}` or `if c { n += 1 }`, is an expression,
            // a statement unless its blocks end with statements.
            let semicolon = if !is_last && !has_unit_blocks(block_text) {
                ";"
            } else {
                ""
            };
            emitter.push(
                node,
                format!(
                    "{}{}{semicolon}",
                    spaces(node.indent),
                    lower_expr(block_text)
                ),
            );
        } else if is_control(block_text) {
            // Loop bodies and blocks whose value is unused end with a statement.
            emit_braced_item_statement(
                node,
                Context::Block,
                emitter,
                lower_expr(block_text),
                has_explicit_semicolon,
                is_last && !is_loop(block_text),
            );
        } else if looks_like_struct_literal(node) {
            emit_struct_literal_expr(
                node,
                block_text,
                emitter,
                is_last && !has_explicit_semicolon,
            );
        } else {
            emit_statement(node, context, is_last, emitter);
        }
    }
}

/// The `{` opening the body of `header`: on a line of its own at `indent` when `header` puts it
/// there, as after a `where` clause, else after the header.
fn brace_opener(header: &str, indent: &str) -> String {
    if header.lines().last().is_some_and(|line| line.trim() == "{") {
        format!("\n{indent}{{")
    } else {
        " {".to_string()
    }
}

fn emit_braced_item(node: &Node, child_context: Context, emitter: &mut Emitter, header: String) {
    emit_braced_item_statement(node, child_context, emitter, header, false, true);
}

fn emit_braced_item_statement(
    node: &Node,
    child_context: Context,
    emitter: &mut Emitter,
    header: String,
    has_explicit_semicolon: bool,
    tail_is_value: bool,
) {
    let indent = spaces(node.indent);
    let (header, _) = strip_block_marker(&header);
    let opener = brace_opener(header, &indent);
    let header = strip_open_brace(header);
    if node.children.is_empty()
        && header.ends_with('}')
        && open_delimiters(header).is_some_and(|open| open.is_empty())
    {
        // The fields of a one-line struct and the arms of a one-line `match` are lowered; the
        // body of a one-line item such as `impl[T] Sealed for Vec[T] { fn f() {} }` keeps its
        // Rust tokens.
        let line = match one_line_body(strip_block_marker(node.text.trim()).0) {
            Some((head, body))
                if !matches!(
                    child_context,
                    Context::Struct | Context::Enum | Context::Match
                ) =>
            {
                format!("{}{body}", lower_signature(head))
            }
            _ => header.to_string(),
        };
        let semicolon = if has_explicit_semicolon { ";" } else { "" };
        emitter.push(node, format!("{indent}{line}{semicolon}"));
        return;
    }
    if node.children.is_empty() && matches!(child_context, Context::Struct | Context::Enum) {
        let terminator = if header.ends_with(['}', ';']) {
            ""
        } else {
            ";"
        };
        emitter.push(node, format!("{}{}{}", indent, header, terminator));
        return;
    }
    emitter.push(node, format!("{indent}{header}{opener}"));
    emit_sequence(&node.children, child_context, tail_is_value, emitter);
    emitter.push_generated(format!(
        "{}}}{}",
        indent,
        if has_explicit_semicolon { ";" } else { "" }
    ));
}

fn emit_function(node: &Node, emitter: &mut Emitter) {
    let text = node.text.trim();
    let (text, has_block) = strip_block_marker(text);
    let indent = spaces(node.indent);
    if let Some(body) = inline_block_body(node)
        && let Some((signature, _)) = split_function_body(text)
    {
        let signature = signature.trim();
        emitter.push(node, format!("{}{} {{", indent, lower_signature(signature)));
        emit_function_body(&body, function_returns_value(signature), emitter);
        emitter.push_generated(format!("{}}}", indent));
        return;
    }
    let Some((signature, body)) = split_function_body(text) else {
        // A block comment after the `{`, as in `fn f() { /* } */`, stays after it.
        let (code, comment) = match text.split_at(code_len(text)) {
            (code, comment) if code.ends_with('{') => (code, comment),
            _ => (text, ""),
        };
        if node.children.is_empty() && !has_block && !code.ends_with('{') {
            emitter.push(node, format!("{}{}", indent, lower_signature(text)));
        } else {
            let signature = strip_open_brace(code.trim());
            let returns_value = function_returns_value(signature);
            let opener = brace_opener(code, &indent);
            emitter.push(
                node,
                format!("{}{}{opener}{comment}", indent, lower_signature(signature)),
            );
            emit_function_body(&node.children, returns_value, emitter);
            emitter.push_generated(format!("{}}}", indent));
        }
        return;
    };
    let signature = signature.trim();
    let returns_value = function_returns_value(signature);
    emitter.push(node, format!("{}{} {{", indent, lower_signature(signature)));
    let body = body.trim();
    let (body, has_explicit_semicolon) = strip_explicit_semicolon(body);
    if body.is_empty() {
        emit_function_body(&node.children, returns_value, emitter);
    } else if !node.children.is_empty() && are_continuations(&node.children) {
        emitter.push_generated(format!("{}{}", spaces(node.indent + 4), lower_expr(body)));
        emit_continuations(
            &node.children,
            4,
            returns_value && !has_explicit_semicolon,
            emitter,
        );
    } else {
        emitter.push_generated(format!(
            "{}{}{}",
            spaces(node.indent + 4),
            lower_expr(body),
            if returns_value && !has_explicit_semicolon {
                ""
            } else {
                ";"
            }
        ));
    }
    emitter.push_generated(format!("{}}}", indent));
}

fn emit_function_body(nodes: &[Node], returns_value: bool, emitter: &mut Emitter) {
    emit_sequence(nodes, Context::Block, returns_value, emitter);
}

fn emit_statement(node: &Node, _context: Context, is_last: bool, emitter: &mut Emitter) {
    let indent = spaces(node.indent);
    let (text, has_explicit_semicolon) = strip_explicit_semicolon(node.text.trim());
    let (block_text, _) = strip_block_marker(text);
    // A head ending with `{` or `:` owns statements, such as `(O).init(f)` or `*x = 1`.
    if !node.children.is_empty() && !text.ends_with(['{', ':']) && are_continuations(&node.children)
    {
        emitter.push(node, format!("{}{}", indent, lower_expr(text)));
        // A `let` or an item such as `const K: u8 = A` / `    | B` ends with `;` even last.
        emit_continuations(
            &node.children,
            0,
            is_last && !has_explicit_semicolon && !is_let(text) && !is_assignment(text),
            emitter,
        );
    } else if !node.children.is_empty() && is_multiline_closure_head(text) {
        emit_multiline_closure_expr(node, text, is_last && !has_explicit_semicolon, emitter);
    } else if !node.children.is_empty()
        && let Some((lhs, rhs)) = split_once_top_level(text, '=')
        && is_multiline_closure_head(rhs.trim())
    {
        emit_multiline_closure_assignment(node, lhs.trim(), rhs.trim(), emitter);
    } else if (!node.children.is_empty() || text.ends_with('{')) && is_raw_braced_expr_head(text) {
        // An empty block, as in `let g = {` / `};`, closes on a line of its own too.
        let is_value = is_last && !has_explicit_semicolon && !is_let(text);
        emit_raw_braced_expr(node, text, is_value, emitter);
    } else if !node.children.is_empty()
        && let Some(head) = let_else_head(text)
    {
        emit_raw_braced_expr(node, &format!("{head} else {{"), false, emitter);
    } else if starts_raw_item(text) && text.ends_with('}') {
        // An item kept as Rust, as in `default impl T for X {` ... `}`, takes no `;`.
        emitter.push(node, format!("{indent}{text}"));
    } else if is_use_or_extern_crate(text) || starts_item(text, "type") {
        emitter.push(node, format!("{}{};", indent, lower_signature(text)));
    } else if let tail = last_statement(text)
        && (is_let(tail) || is_assignment(tail) || is_jump_statement(tail))
    {
        emitter.push(node, format!("{}{};", indent, lower_expr(text)));
    } else if !node.children.is_empty() && is_control(block_text) {
        emit_braced_item_statement(
            node,
            Context::Block,
            emitter,
            lower_expr(block_text),
            has_explicit_semicolon,
            is_last && !is_loop(block_text),
        );
    } else if !node.children.is_empty() && looks_like_struct_literal(node) {
        emit_struct_literal_expr(
            node,
            block_text,
            emitter,
            is_last && !has_explicit_semicolon,
        );
    } else {
        // A `name! { ... }` statement, which may also be an item, needs no `;`.
        let suffix = if has_explicit_semicolon {
            ";"
        } else if is_last || is_brace_macro_invocation(text) {
            ""
        } else {
            ";"
        };
        emitter.push(node, format!("{}{}{}", indent, lower_expr(text), suffix));
    }
}

/// The statement after the last top-level `;` of `text`, as `s.len()` in `let s = 1; s.len()`;
/// a `;` in a literal, group, or macro body separates none.
fn last_statement(text: &str) -> &str {
    let chars = text.chars().collect::<Vec<_>>();
    let mut start = 0;
    while let Some(end) = top_level_position(&chars, start, |at| chars[at] == ';') {
        start = end + 1;
    }
    let at = text
        .char_indices()
        .nth(start)
        .map_or(text.len(), |(at, _)| at);
    text[at..].trim()
}

/// `text` split before the top-level `{ ... }` that ends it, as in `impl A { fn f() {} }`.
pub(crate) fn one_line_body(text: &str) -> Option<(&str, &str)> {
    let chars = text.chars().collect::<Vec<_>>();
    let open = top_level_position(&chars, 0, |at| chars[at] == '{')?;
    (matching_bracket(&chars, open, '{', '}') == Some(chars.len() - 1)).then(|| {
        let at = text.char_indices().nth(open).map_or(0, |(at, _)| at);
        text.split_at(at)
    })
}

fn is_brace_macro_invocation(text: &str) -> bool {
    let chars = text.chars().collect::<Vec<_>>();
    text.ends_with('}') && macro_invocation_end(&chars, 0) == Some(chars.len() - 1)
}

fn emit_multiline_closure_expr(node: &Node, head: &str, is_last: bool, emitter: &mut Emitter) {
    let indent = spaces(node.indent);
    emitter.push(
        node,
        format!("{}{} {{", indent, lower_expr(strip_block_opener(head))),
    );
    emit_nodes(&node.children, Context::Block, emitter);
    emitter.push_generated(format!("{}}}{}", indent, if is_last { "" } else { ";" }));
}

fn emit_multiline_closure_assignment(node: &Node, lhs: &str, rhs: &str, emitter: &mut Emitter) {
    let indent = spaces(node.indent);
    emitter.push(
        node,
        format!(
            "{}{} = {} {{",
            indent,
            lower_expr(lhs),
            lower_expr(strip_block_opener(rhs))
        ),
    );
    emit_nodes(&node.children, Context::Block, emitter);
    emitter.push_generated(format!("{}}};", indent));
}

fn emit_raw_braced_expr(node: &Node, head: &str, is_last: bool, emitter: &mut Emitter) {
    let indent = spaces(node.indent);
    let head = strip_block_opener(head);
    let lowered = lower_expr(head);
    // A bare block is a `{` line.
    let space = if lowered.is_empty() { "" } else { " " };
    emitter.push(node, format!("{indent}{lowered}{space}{{"));
    emit_nodes(&node.children, Context::Block, emitter);
    let (closing, has_explicit_semicolon) = node
        .closing
        .as_deref()
        .map_or_else(|| (closing_delimiters(head), false), closing_suffix);
    emitter.push_generated(format!(
        "{}}}{}{}",
        indent,
        closing,
        // A bare block is a statement without `;`.
        if (is_last || lowered.is_empty()) && !has_explicit_semicolon {
            ""
        } else {
            ";"
        }
    ));
}

/// The lowered suffix of a `})?` closing line and whether it ends with an explicit `;`.
/// Parameters of the closures whose bodies `header` leaves open, as `env` in `.map(|env| {`.
fn open_closure_parameters(header: &str) -> Vec<String> {
    let chars = header.chars().collect::<Vec<_>>();
    closures(&chars)
        .into_iter()
        .filter(|closure| closure.body.end == chars.len())
        .flat_map(|closure| closure.parameters)
        .collect()
}

fn closing_suffix(tail: &str) -> (String, bool) {
    let (tail, has_explicit_semicolon) = strip_explicit_semicolon(tail);
    // The tail binds its own names, as `x` in `}, |x| x.len())`.
    let bindings = node_bindings(tail, Context::Block, false).own;
    // The closures the head opened end here, so their parameters no longer bind.
    let closed = OPEN_CLOSURES.with_borrow(|names| {
        VALUE_SCOPES.with_borrow_mut(|scopes| {
            names
                .iter()
                .filter_map(|name| {
                    let scope = scopes.iter().rposition(|scope| scope.contains(name))?;
                    let at = scopes[scope].iter().position(|bound| bound == name)?;
                    Some((scope, scopes[scope].remove(at)))
                })
                .collect::<Vec<_>>()
        })
    });
    let lowered = with_value_scope(bindings, || lower_expr(tail));
    VALUE_SCOPES.with_borrow_mut(|scopes| {
        for (scope, name) in closed {
            scopes[scope].push(name);
        }
    });
    (lowered, has_explicit_semicolon)
}

fn emit_continuations(
    nodes: &[Node],
    indent_offset: usize,
    last_is_tail_value: bool,
    emitter: &mut Emitter,
) {
    for (index, node) in nodes.iter().enumerate() {
        let adjusted = offset_node_indent(node, indent_offset);
        let is_last = index + 1 == nodes.len();
        emit_node(
            &adjusted,
            Context::Block,
            !is_last || last_is_tail_value,
            emitter,
        );
    }
}

pub(crate) fn offset_node_indent(node: &Node, offset: usize) -> Node {
    Node {
        line: node.line,
        indent: node.indent + offset,
        start: node.start,
        text: node.text.clone(),
        closing: node.closing.clone(),
        comment: node.comment.clone(),
        closing_comment: node.closing_comment.clone(),
        children: node
            .children
            .iter()
            .map(|child| offset_node_indent(child, offset))
            .collect(),
    }
}

/// A field or variant without the Rust `,` it may be written with.
fn strip_member_comma(text: &str) -> &str {
    text.strip_suffix(',').map_or(text, str::trim_end)
}

fn emit_struct_member(node: &Node, emitter: &mut Emitter) {
    let text = strip_member_comma(node.text.trim());
    if is_function(text) || is_impl_or_mod_item(text) {
        emit_general(node, Context::Block, true, emitter);
    } else {
        emitter.push(
            node,
            format!("{}{},", spaces(node.indent), lower_field_or_variant(text)),
        );
    }
}

fn emit_enum_member(node: &Node, emitter: &mut Emitter) {
    let (text, _) = strip_block_marker(strip_member_comma(node.text.trim()));
    if node.children.is_empty() {
        emitter.push(
            node,
            format!("{}{},", spaces(node.indent), lower_field_or_variant(text)),
        );
    } else {
        emitter.push(
            node,
            format!("{}{} {{", spaces(node.indent), lower_field_or_variant(text)),
        );
        emit_nodes(&node.children, Context::Struct, emitter);
        emitter.push_generated(format!("{}}},", spaces(node.indent)));
    }
}

fn emit_trait_member(node: &Node, emitter: &mut Emitter) {
    let text = node.text.trim();
    let (block_text, _) = strip_block_marker(text);
    let (signature, _) = strip_explicit_semicolon(block_text);
    // A default body is `= value`, an indented block, a one-line `{ ... }`, or an empty `{` / `}`.
    if is_function(signature)
        && (split_function_body(signature).is_some()
            || signature[..code_len(signature)].ends_with(['{', '}'])
            || !node.children.is_empty())
    {
        emit_function(node, emitter);
    } else if is_function(signature) {
        emitter.push(
            node,
            format!("{}{};", spaces(node.indent), lower_signature(signature)),
        );
    } else {
        emit_general(node, Context::Trait, true, emitter);
    }
}

fn emit_match_arm(node: &Node, emitter: &mut Emitter) {
    let indent = spaces(node.indent);
    let (text, _) = strip_block_marker(node.text.trim());
    if let Some((pattern, expr)) = split_arrow(text) {
        let pattern = lower_expr(pattern.trim());
        let expr = expr.trim();
        let (expr, has_explicit_semicolon) = strip_explicit_semicolon(expr);
        // The Rust `,` of an arm inside a brace `match`, as in `Some(x) => x,`.
        let expr = expr.strip_suffix(',').map_or(expr, str::trim_end);
        if expr.is_empty() {
            emitter.push(node, format!("{}{} => {{", indent, pattern));
            emit_nodes(&node.children, Context::Block, emitter);
            emitter.push_generated(format!("{}}}", indent));
        } else if has_explicit_block_opener(node.text.trim()) {
            // A body opening a block, as `x => match y:`, `x => unsafe:`, or `x => f(|v| {`.
            let expr = strip_open_brace(expr).trim_end();
            let space = if expr.is_empty() || expr.ends_with(['(', '[']) {
                ""
            } else {
                " "
            };
            emitter.push(
                node,
                format!("{indent}{pattern} => {}{space}{{", lower_expr(expr)),
            );
            emit_nodes(&node.children, arm_block_context(expr), emitter);
            let (closing, _) = node
                .closing
                .as_deref()
                .map_or_else(|| (closing_delimiters(expr), false), closing_suffix);
            let comma = if closing.is_empty() { "" } else { "," };
            emitter.push_generated(format!("{indent}}}{closing}{comma}"));
        } else if !node.children.is_empty() {
            emitter.push(
                node,
                format!("{}{} => {}", indent, pattern, lower_expr(expr)),
            );
            emit_continuations(&node.children, 0, true, emitter);
            if let Some(last) = emitter.lines.last_mut() {
                last.push(if has_explicit_semicolon { ';' } else { ',' });
            }
        } else if has_explicit_semicolon {
            emitter.push(node, format!("{}{} => {{", indent, pattern));
            emitter.push_generated(format!("{}{};", spaces(node.indent + 4), lower_expr(expr)));
            emitter.push_generated(format!("{}}}", indent));
        } else {
            // A block body, as in `None => {}`, needs no comma.
            let comma = if is_brace_group(expr) { "" } else { "," };
            emitter.push(
                node,
                format!("{}{} => {}{comma}", indent, pattern, lower_expr(expr)),
            );
        }
    } else if node.children.is_empty() {
        let (text, _) = strip_explicit_semicolon(node.text.trim());
        emitter.push(node, format!("{}{},", indent, lower_expr(text)));
    } else {
        emitter.push(node, format!("{}{} => {{", indent, lower_expr(text)));
        emit_nodes(&node.children, Context::Block, emitter);
        emitter.push_generated(format!("{}}}", indent));
    }
}

/// Whether each `{...}` group of the one-line `text` outside other groups is empty or ends with a
/// statement, as in `if c { f(); } else {}`, so `text` is `()`.
fn has_unit_blocks(text: &str) -> bool {
    let mut rest = text;
    let mut found = false;
    while let Some(open) = rest.find('{') {
        let Some(close) = matching_delimiter(rest, open, '{', '}', false) else {
            return false;
        };
        let inner = rest[open + 1..close].trim();
        if !inner.is_empty() && !inner.ends_with(';') {
            return false;
        }
        found = true;
        rest = &rest[close + 1..];
    }
    found
}

/// Whether `text` is one `{...}` group, as in `{}` or `{ f(); }`.
fn is_brace_group(text: &str) -> bool {
    text.starts_with('{') && matching_delimiter(text, 0, '{', '}', false) == Some(text.len() - 1)
}

/// Whether each arm of the `match` at `node` is a block ending with a statement, as in
/// `None => {}` or `Some(x) =>:` / `v.push(x);`, so the `match` is `()` and needs no `;`.
/// Whether `node` is a statement, or a control block such as `if c:` / `else:` whose body ends
/// with one; a `loop` takes its value from `break`.
fn ends_with_statement(node: &Node) -> bool {
    let (text, explicit) = strip_explicit_semicolon(node.text.trim());
    let head = strip_block_marker(text).0;
    explicit
        || is_let(text)
        || is_assignment(text)
        || is_jump_statement(text)
        || head.starts_with("while ")
        || head.starts_with("for ")
        || (is_control(head)
            && head != "loop"
            && node
                .children
                .iter()
                .rfind(|child| !is_comment(child.text.trim()))
                .is_some_and(ends_with_statement))
}

fn has_unit_arms(node: &Node) -> bool {
    let arms = node
        .children
        .iter()
        .filter(|arm| !is_comment(arm.text.trim()))
        .collect::<Vec<_>>();
    !arms.is_empty()
        && arms.iter().all(|arm| {
            let text = strip_block_marker(arm.text.trim()).0;
            let Some((_, expr)) = split_arrow(text) else {
                return false;
            };
            let expr = expr.trim();
            match arm
                .children
                .iter()
                .rfind(|child| !is_comment(child.text.trim()))
            {
                Some(last) if expr.is_empty() => ends_with_statement(last),
                Some(_) => false,
                None => {
                    strip_explicit_semicolon(expr).1
                        || is_brace_group(expr) && {
                            let inner = expr[1..expr.len() - 1].trim();
                            inner.is_empty() || inner.ends_with(';')
                        }
                }
            }
        })
}

/// The context of the block a match arm body such as `match y:` or `unsafe:` opens.
pub(crate) fn arm_block_context(expr: &str) -> Context {
    let expr = strip_open_brace(expr.trim());
    if is_match(expr) || is_trailing_match(expr) {
        Context::Match
    } else {
        Context::Block
    }
}

fn emit_macro_arm(node: &Node, emitter: &mut Emitter) {
    let indent = spaces(node.indent);
    let (text, _) = strip_block_marker(node.text.trim());
    if let Some((pattern, expr)) = split_arrow(text) {
        let pattern = pattern.trim();
        let expr = expr.trim();
        let (expr, _) = strip_explicit_semicolon(expr);
        if expr.is_empty() {
            emitter.push(node, format!("{}{} => {{", indent, pattern));
            emit_raw_macro_nodes(&node.children, emitter);
            emitter.push_generated(format!("{}}};", indent));
        } else if expr.trim_start().starts_with('{') && expr.trim_end().ends_with('}') {
            emitter.push(node, format!("{}{} => {};", indent, pattern, expr));
        } else {
            emitter.push(node, format!("{}{} => {{ {} }};", indent, pattern, expr));
        }
    } else {
        emitter.push(node, format!("{}{};", indent, text));
    }
}

fn emit_raw_macro_nodes(nodes: &[Node], emitter: &mut Emitter) {
    for node in nodes {
        emitter.push(
            node,
            format!("{}{}", spaces(node.indent), node.text.trim_end()),
        );
        emit_raw_macro_nodes(&node.children, emitter);
    }
}

fn emit_struct_literal_expr(node: &Node, text: &str, emitter: &mut Emitter, is_last: bool) {
    let indent = spaces(node.indent);
    let (text, _) = strip_block_marker(text);
    // A generic head, as in `Pair[i32]:`, takes its generics as a turbofish.
    let chars = text.trim_end().chars().collect::<Vec<_>>();
    let head = (0..chars.len())
        .find(|&open| {
            chars[open] == '['
                && matching_bracket(&chars, open, '[', ']')
                    .is_some_and(|close| close + 1 == chars.len())
        })
        .map_or_else(
            || lower_expr(text),
            |open| {
                let name = chars[..open].iter().collect::<String>();
                let content = chars[open + 1..chars.len() - 1].iter().collect::<String>();
                format!(
                    "{}::<{}>",
                    lower_expr(&name),
                    lower_generic_content(&content)
                )
            },
        );
    emitter.push(node, format!("{indent}{head} {{"));
    emit_nodes(&node.children, Context::StructLiteral, emitter);
    emitter.push_generated(format!("{}}}{}", indent, if is_last { "" } else { ";" }));
}

fn emit_struct_literal_field(node: &Node, emitter: &mut Emitter) {
    let text = node.text.trim();
    if let Some((field, expr)) = split_once_top_level(text, '=') {
        emitter.push(
            node,
            format!(
                "{}{}: {},",
                spaces(node.indent),
                field.trim(),
                lower_expr(expr.trim())
            ),
        );
    } else {
        emitter.push(
            node,
            format!("{}{},", spaces(node.indent), lower_expr(text)),
        );
    }
}

impl Emitter {
    /// Starts `node`, after the blank source line before it.
    fn begin_node(&mut self, node: &Node, kind: String) {
        // Only the first node on a line, not the body an `= match x:` head lowers to.
        if node.line > self.last_node_line
            && node.line > 1
            && self.source_lines[node.line - 2].trim().is_empty()
            && self
                .lines
                .last()
                .is_some_and(|last| !last.trim().is_empty())
        {
            self.lines.push(String::new());
        }
        self.last_node_line = self.last_node_line.max(node.line);
        let source_column = self.source_column(node);
        self.tree_stack.push(SourceMapNode {
            kind,
            source_line: node.start.0,
            source_column,
            source_text: node.text.clone(),
            generated_start_line: self.lines.len() + 1,
            generated_end_line: self.lines.len(),
            children: Vec::new(),
        });
    }

    /// The UTF-16 column `node`'s text starts at in its source line.
    fn source_column(&self, node: &Node) -> usize {
        let (line, at) = node.start;
        self.source_lines[line - 1][..at].encode_utf16().count()
    }

    fn end_node(&mut self) {
        let Some(mut node) = self.tree_stack.pop() else {
            return;
        };
        node.generated_end_line = self.lines.len();
        if let Some(parent) = self.tree_stack.last_mut() {
            parent.children.push(node);
        } else {
            self.source_tree.push(node);
        }
    }

    /// Pushes a possibly multi-line logical line, mapping each physical line to its source line.
    /// A header whose `:` ends a more indented continuation line opens its `{` on a line of its
    /// own at the header's indent, as rustfmt lays it out.
    fn push(&mut self, node: &Node, mut line: String) {
        // The line after an attribute from the same source line joins it, as in `#[inline] fn`.
        if self.attribute_line == Some(node.line)
            && let Some(last) = self.lines.last_mut()
            && last.trim_start().starts_with("#[")
        {
            last.push(' ');
            line = format!("{last}{}", line.trim_start());
            self.lines.pop();
        }
        self.attribute_line = node
            .text
            .trim_start()
            .starts_with("#[")
            .then_some(node.line);
        if opens_on_own_line(node)
            && line.split('\n').count() == node.text.lines().count()
            && line.ends_with(" {")
        {
            line.truncate(line.len() - 2);
            // rustfmt ends a `where` clause's last bound with `,`.
            if has_where_line(&node.text) && !line[..code_len(&line)].ends_with(',') {
                line.push(',');
            }
            line.push_str(&format!("\n{}{{", spaces(node.indent)));
            // rustfmt keeps an empty body after a `where` clause on two lines.
            if !has_where_line(&node.text) {
                self.joining_lines
                    .push(self.lines.len() + line.split('\n').count() - 1);
            }
        }
        if !is_else_branch(node.text.trim()) && is_branch(node.text.trim()) {
            self.joining_lines.push(self.lines.len());
        }
        for (offset, part) in line.split('\n').enumerate() {
            self.source_map.push(SourceMapEntry {
                source_line: node.line + offset,
                generated_line: self.lines.len() + 1,
            });
            self.lines.push(part.to_string());
        }
    }

    /// Writes each `else` after the `}` line before it, as in `} else {`, and an empty block's
    /// `}` after its `{` line of its own, as in `{}`, renumbering the maps.
    pub(crate) fn join_split_lines(&mut self) {
        let in_literal = lines_ending_in_literal(&self.lines.join("\n"));
        let mut joined = Vec::new();
        for index in (1..self.lines.len()).rev() {
            if in_literal[index - 1] {
                continue;
            }
            let (before, after) = self.lines.split_at_mut(index);
            let (close, branch) = (&mut before[index - 1], &after[0]);
            let joins = self.joining_lines.contains(&index);
            if close.trim() == "}"
                && raw_indent(close) == raw_indent(branch)
                && (is_else_branch(branch.trim()) || joins)
            {
                if is_else_branch(branch.trim()) {
                    close.push(' ');
                }
                close.push_str(branch.trim_start());
                self.lines.remove(index);
                joined.push(index);
            } else if close.trim() == "{"
                && self.joining_lines.contains(&(index - 1))
                && raw_indent(close) == raw_indent(branch)
                && matches!(branch.trim(), "}" | "};" | "},")
            {
                close.push_str(branch.trim_start());
                self.lines.remove(index);
                joined.push(index);
            }
        }
        // A generated line number counts from 1, so line `index + 1` joins line `index`.
        let renumber = |line: usize| line - joined.iter().filter(|&&index| index < line).count();
        for entry in &mut self.source_map {
            entry.generated_line = renumber(entry.generated_line);
        }
        fn renumber_tree(nodes: &mut [SourceMapNode], renumber: &dyn Fn(usize) -> usize) {
            for node in nodes {
                node.generated_start_line = renumber(node.generated_start_line);
                node.generated_end_line = renumber(node.generated_end_line);
                renumber_tree(&mut node.children, renumber);
            }
        }
        renumber_tree(&mut self.source_tree, &renumber);
    }

    fn push_generated(&mut self, line: String) {
        self.lines.extend(line.split('\n').map(str::to_string));
    }
}
