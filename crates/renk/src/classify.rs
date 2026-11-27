//! What a Renk line starts or ends with, such as an item header, a control head or a `:`.

use super::*;

pub(crate) fn looks_like_item_header_without_body(text: &str) -> bool {
    is_struct_item(text)
        || is_enum_item(text)
        || is_trait_item(text)
        || is_rust_impl_or_mod_item(text)
        || is_function(text)
}

/// Whether a line of the multi-line header `text` starts a `where` clause.
pub(crate) fn has_where_line(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line == "where" || line.starts_with("where ")
    })
}

pub(crate) fn is_else_branch(text: &str) -> bool {
    text == "else" || text.starts_with("else:") || text.starts_with("else ")
}

/// Whether `text` continues the chain of blocks before it: an `else` branch, or the next block
/// argument after a block argument's closing line, as in `, |y|:` after `f(|x|:`.
pub(crate) fn is_branch(text: &str) -> bool {
    let code = without_trailing_line_comment(text).trim_end();
    is_else_branch(text)
        || (code.starts_with(',') && (has_block_marker(code) || code.ends_with('{')))
}

pub(crate) fn strip_explicit_semicolon(text: &str) -> (&str, bool) {
    let text = text.trim_end();
    text.strip_suffix(';')
        .map(|stripped| (stripped.trim_end(), true))
        .unwrap_or((text, false))
}

pub(crate) fn strip_block_marker(text: &str) -> (&str, bool) {
    let text = text.trim_end();
    text.strip_suffix(':')
        .map(|stripped| (stripped.trim_end(), true))
        .unwrap_or((text, false))
}

pub(crate) fn has_block_marker(text: &str) -> bool {
    strip_block_marker(text).1
}

pub(crate) fn has_explicit_block_opener(text: &str) -> bool {
    let (text, _) = strip_explicit_semicolon(text);
    has_block_marker(text) || text.trim_end().ends_with('{')
}

/// A `//` line or a text that is one block comment, such as a multi-line `/*! ... */`.
pub(crate) fn is_comment(text: &str) -> bool {
    let chars = text.chars().collect::<Vec<_>>();
    text.starts_with("//")
        || (text.starts_with("/*") && opaque_span_end(&chars, 0) == Some(chars.len() - 1))
}

pub(crate) fn is_struct_item(text: &str) -> bool {
    // `union` is a keyword only before the item's name, unlike `union = merge(union)`.
    starts_item(text, "struct")
        || (starts_item(text, "union")
            && without_visibility(text)
                .strip_prefix("union")
                .is_some_and(|rest| rest.trim_start().starts_with(is_ident_start)))
}

pub(crate) fn is_enum_item(text: &str) -> bool {
    starts_item(text, "enum")
}

pub(crate) fn is_trait_item(text: &str) -> bool {
    starts_item(text, "trait") || starts_item(text, "unsafe trait")
}

pub(crate) fn is_impl_or_mod_item(text: &str) -> bool {
    starts_item(text, "impl") || starts_item(text, "unsafe impl") || starts_item(text, "mod")
}

pub(crate) fn is_macro_item(text: &str) -> bool {
    text.starts_with("macro_rules!")
}

/// `text` after the attributes on its line, as `pub fn get(&self)` in
/// `#[inline] pub fn get(&self)`.
pub(crate) fn without_attributes(mut text: &str) -> &str {
    while let Some(open) = ["#[", "#!["]
        .iter()
        .find_map(|start| text.starts_with(start).then_some(start.len() - 1))
        && let Some(close) = matching_delimiter(text, open, '[', ']', false)
    {
        text = text[close + 1..].trim_start();
    }
    text
}

/// Whether `text` starts a function item after its qualifiers, as in
/// `pub(crate) const unsafe extern "C" fn`; `let q = quote! { fn f() {} }` is not one.
pub(crate) fn is_function(text: &str) -> bool {
    let mut rest = without_visibility(without_attributes(text));
    loop {
        if rest.starts_with("fn ") {
            return true;
        }
        let qualified = ["const", "async", "unsafe", "safe", "default", "extern"]
            .iter()
            .find_map(|qualifier| rest.strip_prefix(qualifier))
            .filter(|after| after.starts_with(' '));
        let Some(after) = qualified else {
            return false;
        };
        rest = after.trim_start();
        // The ABI of `extern "C" fn`.
        if let Some(abi) = rest.strip_prefix('"') {
            let Some(end) = abi.find('"') else {
                return false;
            };
            rest = abi[end + 1..].trim_start();
        }
    }
}

pub(crate) fn function_returns_value(signature: &str) -> bool {
    let Some((_, return_type)) = signature.rsplit_once("->") else {
        return false;
    };
    let return_type = return_type.trim();
    !return_type.is_empty() && return_type != "()"
}

pub(crate) fn is_match(text: &str) -> bool {
    text.starts_with("match ")
}

/// A `match` ending a larger expression, such as `let x = match y`, `return match y`, or
/// `.filter(|x| match y`; it owns match arms and closes the delimiters its head left open. The
/// scrutinee may be a block over several lines, as in `let v = match unsafe {` ... `}`.
pub(crate) fn is_trailing_match(text: &str) -> bool {
    !is_match(text)
        && text.match_indices("match ").any(|(at, _)| {
            let before = text[..at].trim_end();
            !text[..at].ends_with(is_ident_continue)
                // An operand, as in `= match`, `&mut match`, `!match`, or `lo..match`.
                && (before.ends_with(['=', '(', '[', '|', ',', '>', '&', '*', '!', '-', '+'])
                    || before.ends_with("..")
                    || ["return", "&mut"].iter().any(|keyword| {
                        before
                            .strip_suffix(keyword)
                            .is_some_and(|rest| !rest.ends_with(is_ident_continue))
                    }))
                && (!has_top_level_brace(&text[at..])
                    || (text[at..].contains('\n')
                        && !is_control(text)
                        && ["match {", "match unsafe {"]
                            .iter()
                            .any(|head| text[at..].starts_with(head))))
                && is_balanced_tail(text, at)
        })
}

/// Whether `text[at..]` neither leaves a group open nor closes one opened before it, as the `]`
/// of a macro group joined into one line with its arms, as in `vec![match a:` / `    ]`.
fn is_balanced_tail(text: &str, at: usize) -> bool {
    open_delimiters(&text[at..]).is_some_and(|open| open.is_empty())
        && open_delimiters(text) == open_delimiters(&text[..at])
}

/// Whether `text` has a `{` or `}` outside parentheses and brackets, unlike the struct literal
/// in `match f(Key { k }):`.
fn has_top_level_brace(text: &str) -> bool {
    let chars = text.chars().collect::<Vec<_>>();
    let mut depth = 0usize;
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            index = end + 1;
            continue;
        }
        match chars[index] {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '{' | '}' if depth == 0 => return true,
            _ => {}
        }
        index += 1;
    }
    false
}

/// Whether `text` ends in a block-owning `if` head after a binding, `return`, an open group, or
/// closure parameters, such as `let x = if c`, `return if c`, `max(if c`, or `.map(|x| if c`;
/// `if c then a else b` and `if c { a } else { b }` stay inline, unlike a header over several
/// lines such as `let v = if match x {` ... `}`.
pub(crate) fn is_trailing_if(text: &str) -> bool {
    text.match_indices("if ").any(|(at, _)| {
        let before = text[..at].trim_end();
        !text[..at].ends_with(is_ident_continue)
            && (before.ends_with(['=', '(', '[', ',', '|'])
                || before
                    .strip_suffix("return")
                    .is_some_and(|rest| !rest.ends_with(is_ident_continue)))
            && (!has_top_level_brace(&text[at..])
                || (text[at..].contains('\n') && !is_control(text)))
            && !is_inline_if_expression(&text[at..])
            && is_balanced_tail(text, at)
    })
}

pub(crate) fn is_control(text: &str) -> bool {
    let text = strip_control_label(text);
    (text.starts_with("if ") && !is_inline_if_expression(text))
        || (text.starts_with("else if ") && !is_inline_if_expression(text))
        || (text.starts_with("else") && !text.starts_with("else if "))
        || text == "loop"
        || text.starts_with("while ")
        || text.starts_with("for ")
        || text == "unsafe"
        || text == "async"
}

pub(crate) fn is_loop(text: &str) -> bool {
    let text = strip_control_label(text);
    text == "loop" || text.starts_with("while ") || text.starts_with("for ")
}

pub(crate) fn strip_control_label(text: &str) -> &str {
    let text = text.trim_start();
    let Some(rest) = text.strip_prefix('\'') else {
        return text;
    };
    let Some(colon) = rest.find(':') else {
        return text;
    };
    let label = &rest[..colon];
    if label.is_empty() || !label.chars().all(is_ident_continue) {
        return text;
    }
    rest[colon + 1..].trim_start()
}

pub(crate) fn is_inline_if_expression(text: &str) -> bool {
    let Some(rest) = text
        .strip_prefix("if ")
        .or_else(|| text.strip_prefix("else if "))
    else {
        return false;
    };
    let Some(then_index) = find_top_level_keyword(rest, "then") else {
        return false;
    };
    !rest[then_index + "then".len()..].trim().is_empty()
}

/// Whether `text` starts with `keyword` after any visibility, as in `pub(in crate.a) mod m`.
pub(crate) fn starts_item(text: &str, keyword: &str) -> bool {
    let text = without_visibility(text);
    text == keyword
        || text.starts_with(&format!("{keyword} "))
        || text.starts_with(&format!("{keyword}["))
}

pub(crate) fn is_let(text: &str) -> bool {
    text.starts_with("let ")
}

/// Whether `text` starts an item with a body Renk keeps as Rust, as in `default fn`,
/// `auto trait`, `extern "C" {`, or `extern crate a; fn f() {`, unlike
/// `default const K: S = S {};` or `extern "C" fn f() {`.
pub(crate) fn starts_raw_item(text: &str) -> bool {
    let text = without_visibility(without_attributes(text));
    let text = text.strip_prefix("unsafe ").unwrap_or(text);
    text.starts_with("auto trait ")
        || (text.starts_with("extern ") || text.starts_with("extern{"))
            && !text.starts_with("extern crate ")
            && !is_function(text)
        || text
            .strip_prefix("extern crate ")
            .and_then(|rest| rest.split_once(';'))
            .is_some_and(|(_, rest)| {
                let rest = rest.trim_start();
                starts_raw_item(rest) || looks_like_item_header_without_body(rest)
            })
        || text.strip_prefix("default ").is_some_and(|rest| {
            ["fn ", "impl", "unsafe ", "async ", "extern "]
                .iter()
                .any(|keyword| rest.starts_with(keyword))
        })
}

pub(crate) fn is_use_or_extern_crate(text: &str) -> bool {
    let text = without_visibility(text);
    text.starts_with("use ") || text.starts_with("extern crate ")
}

/// `text` without a leading `pub` or `pub(...)`.
pub(crate) fn without_visibility(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("pub") else {
        return text;
    };
    let rest = rest
        .strip_prefix('(')
        .and_then(|scoped| Some(&scoped[scoped.find(')')? + 1..]))
        .unwrap_or(rest);
    if rest.starts_with(' ') {
        rest.trim_start()
    } else {
        text
    }
}

pub(crate) fn are_continuations(nodes: &[Node]) -> bool {
    !nodes.is_empty()
        && nodes
            .iter()
            .all(|node| is_expression_continuation(node.text.trim_start()))
}

fn is_expression_continuation(text: &str) -> bool {
    [
        ".", "::", "(", "[", "?", "+", "-", "*", "/", "%", "&&", "||", "&", "|", "^", "<<", ">>",
        "==", "!=", "<=", ">=", "<", ">",
    ]
    .iter()
    .any(|prefix| text.starts_with(prefix))
}

pub(crate) fn is_jump_statement(text: &str) -> bool {
    text == "return"
        || text.starts_with("return ")
        || text == "break"
        || text.starts_with("break ")
        || text == "continue"
        || text.starts_with("continue ")
}

pub(crate) fn is_assignment(text: &str) -> bool {
    // An operator may end the line, as in `q +=` / `{`.
    let text = &format!("{} ", text.trim_end());
    let chars = text.chars().collect::<Vec<_>>();
    // At the top level, unlike the `=` of `}, |x| { let y = x; y })`.
    top_level_position(&chars, 0, |at| {
        [
            " += ", " -= ", " *= ", " /= ", " %= ", " |= ", " &= ", " ^= ", " <<= ", " >>= ", " = ",
        ]
            .iter()
            .any(|operator| chars[at..].starts_with(&operator.chars().collect::<Vec<_>>()))
    })
    .is_some()
        && !text.contains(" == ")
        && !text.contains(" != ")
        && !text.contains(" <= ")
        && !text.contains(" >= ")
        && !text.contains(" => ")
        // The `=` of `if let` / `while let` binds a pattern in a condition, also in a one-line
        // `if let P = e then a else b`.
        && !is_control(text)
        && !strip_control_label(text).starts_with("if ")
}

pub(crate) fn looks_like_struct_literal(node: &Node) -> bool {
    // `TABLE.with(|slot| {` and `Foo.new().map(|a|:` open a closure body rather than fields, and
    // `a == 2` or `let b = a` is no `field = value`.
    let head = strip_block_marker(node.text.trim()).0;
    !node.children.is_empty()
        && !node.text.trim_end().ends_with('{')
        && head.chars().next().is_some_and(char::is_uppercase)
        && !head.contains('|')
        && node.children.iter().all(|child| {
            split_once_top_level(child.text.trim(), '=').is_some_and(|(field, value)| {
                let field = field.trim();
                let field = field.strip_prefix("r#").unwrap_or(field);
                // A tuple struct's fields are numbered, as in `0 = 1`.
                !value.starts_with('=')
                    && field.starts_with(|c: char| is_ident_start(c) || c.is_ascii_digit())
                    && field.chars().all(is_ident_continue)
            })
        })
}

pub(crate) fn is_inline_struct_literal(text: &str) -> bool {
    let Some(open_index) = text.find('{') else {
        return false;
    };
    text.ends_with('}')
        && text[..open_index]
            .trim_end()
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
}
