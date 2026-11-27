//! Renk syntax in one line lowered to Rust: attributes, signatures, expressions, generics, paths.

use super::*;

pub(crate) fn lower_attribute(text: &str) -> Option<String> {
    if let Some(rest) = text.strip_prefix("#!") {
        if rest.starts_with('[') {
            Some(format!("#!{}", process_attribute_body(rest)))
        } else {
            Some(format!("#![{}]", process_attribute_body(rest)))
        }
    } else if let Some(rest) = text.strip_prefix('#') {
        if rest.starts_with('[') {
            Some(process_attribute_body(text))
        } else {
            Some(format!("#[{}]", process_attribute_body(rest)))
        }
    } else {
        None
    }
}

fn process_attribute_body(text: &str) -> String {
    // Attribute payloads are proc-macro/token-tree input. Keep them as Rust
    // tokens instead of guessing whether punctuation is Renk syntax.
    text.to_string()
}

pub(crate) fn lower_signature(text: &str) -> String {
    if without_visibility(text).starts_with("use ") {
        return lower_use_tree(text);
    }
    // A const expression, as in `[u8; (TABLE)[N].len()]`, reads receivers as values too.
    unwrap_path_receivers(&replace_dotted_paths(
        &replace_square_generics(text, GenericMode::Type),
        PathMode::Type,
    ))
}

/// Every `.` between use-tree segments, including before `*` and `{...}` groups, is `::`.
fn lower_use_tree(text: &str) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            output.extend(&chars[index..=end]);
            index = end + 1;
            continue;
        }
        let separates = chars[index] == '.'
            && index > 0
            && is_ident_continue(chars[index - 1])
            && chars
                .get(index + 1)
                .is_some_and(|&next| is_ident_start(next) || matches!(next, '*' | '{'));
        if separates {
            output.push_str("::");
        } else {
            output.push(chars[index]);
        }
        index += 1;
    }
    output
}

/// A field, a variant, or a struct header; the fields of a one-line `Name { a: Vec[u8] }` are
/// signatures rather than a const expression.
pub(crate) fn lower_field_or_variant(text: &str) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    match top_level_position(&chars, 0, |at| chars[at] == '{')
        .filter(|&open| matching_bracket(&chars, open, '{', '}') == Some(chars.len() - 1))
    {
        Some(open) => format!(
            "{}{{{}}}",
            lower_signature(&chars[..open].iter().collect::<String>()),
            lower_signature(&chars[open + 1..chars.len() - 1].iter().collect::<String>())
        ),
        None => lower_signature(text),
    }
}

pub(crate) fn lower_expr(text: &str) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let statements = top_level_statements(&chars);
    if statements.len() > 1 {
        // Each statement of a line lowers alone, and a `let` binds its names for the statements
        // after it, as `s` in `let t = s.y(); let s = 1; s.len()`, but not before it.
        let mut names = Vec::new();
        let mut lowered = String::new();
        for range in statements {
            let statement = &chars[range.clone()];
            let text = statement.iter().collect::<String>();
            let code = text.trim();
            let (head, tail) = text.split_at(text.len() - text.trim_start().len());
            lowered.push_str(head);
            lowered.push_str(&with_value_scope(names.clone(), || lower_expr(code)));
            lowered.push_str(&tail[code.len()..]);
            if range.end < chars.len() {
                lowered.push(';');
            }
            if is_let_statement(statement) {
                names.extend(let_pattern_bindings(statement, false));
            }
        }
        return lowered;
    }
    let (text, inline_ifs) = extract_argument_inline_ifs(text);
    inline_ifs
        .iter()
        .enumerate()
        .fold(lower_expr_head(&text), |lowered, (index, inline_if)| {
            lowered.replacen(&inline_if_placeholder(index), &lower_expr(inline_if), 1)
        })
}

fn inline_if_placeholder(index: usize) -> String {
    format!("__renk_inline_if_{index}")
}

/// Replaces each `if c then a else b` argument or element, which runs from a `(`/`[`/`,` to the
/// next top-level `,` or closing delimiter, with a placeholder; macro bodies stay Rust syntax.
fn extract_argument_inline_ifs(text: &str) -> (String, Vec<String>) {
    let chars = text.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut inline_ifs = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        if let Some(end) =
            opaque_span_end(&chars, index).or_else(|| macro_invocation_end(&chars, index))
        {
            output.extend(chars[index..=end].iter());
            index = end + 1;
            continue;
        }
        let is_argument = keyword_at(&chars, index, "if")
            && chars[..index]
                .iter()
                .rfind(|character| !character.is_whitespace())
                // A closure body, as in `.map(|x| if c then a else b)`, or an operand, as in
                // `n + if c then a else b`, too.
                .is_some_and(|character| {
                    matches!(
                        character,
                        '(' | '[' | ',' | '|' | '+' | '-' | '*' | '/' | '%' | '&' | '^' | '!' | '<'
                    )
                });
        if is_argument {
            let end = argument_end(&chars, index);
            let inline_if = chars[index..end].iter().collect::<String>();
            if lower_if_then_else(inline_if.trim_end()).is_some() {
                output.push_str(&inline_if_placeholder(inline_ifs.len()));
                output.push_str(&inline_if[inline_if.trim_end().len()..]);
                inline_ifs.push(inline_if.trim_end().to_string());
                index = end;
                continue;
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    (output, inline_ifs)
}

/// End of the argument starting at `start`: the next `,` or `;` at its depth, or the closing
/// delimiter of its group.
pub(crate) fn argument_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < chars.len() {
        if let Some(end) =
            opaque_span_end(chars, index).or_else(|| macro_invocation_end(chars, index))
        {
            index = end + 1;
            continue;
        }
        match chars[index] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' | ',' | ';' if depth == 0 => return index,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    chars.len()
}

fn lower_expr_head(text: &str) -> String {
    if let Some(lowered) = lower_if_then_else(text) {
        return lowered;
    }
    // `let x = if c then a else b`, `return if ...`, `x => if ...`; arguments are extracted
    // by `extract_argument_inline_ifs`.
    if let Some(at) = find_top_level_keyword(text, "if") {
        let before = text[..at].trim_end();
        if (before.ends_with(['=', '>'])
            || before
                .strip_suffix("return")
                .is_some_and(|rest| !rest.ends_with(is_ident_continue)))
            && let Some(lowered) = lower_if_then_else(&text[at..])
        {
            return format!("{}{}", lower_basic_expr(&text[..at]), lowered);
        }
    }
    lower_basic_expr(text)
}

pub(crate) fn is_multiline_closure_head(text: &str) -> bool {
    let text = strip_block_marker(text).0;
    let closure = text.trim().strip_prefix("move ").unwrap_or(text.trim());
    let Some(rest) = closure.strip_prefix('|') else {
        return false;
    };
    let Some(end) = rest.find('|') else {
        return false;
    };
    let tail = rest[end + 1..].trim();
    tail.is_empty() || tail == "{" || tail.starts_with("->")
}

/// `let PAT = EXPR` of a `let PAT = EXPR else:` line that owns the diverging block.
pub(crate) fn let_else_head(text: &str) -> Option<&str> {
    let (head, has_block) = strip_block_marker(text);
    // The `else` of `let x = if c then a else:` belongs to the `if`.
    let is_if = |head: &str| {
        head.split_once(" = ")
            .is_some_and(|(_, value)| value.trim_start().starts_with("if "))
    };
    (has_block && is_let(head))
        .then(|| head.strip_suffix(" else"))
        .flatten()
        .filter(|head| !is_if(head))
}

pub(crate) fn is_raw_braced_expr_head(text: &str) -> bool {
    text.trim().ends_with('{') || ends_with_closure_block(text)
}

/// Whether `text` ends with a closure head that owns the indented block after `:`, as in
/// `xs.sort_by(|a, b|:` or `spawn(move || -> u8:`.
pub(crate) fn ends_with_closure_block(text: &str) -> bool {
    let (head, has_block) = strip_block_marker(text);
    let params = match head.rsplit_once("->") {
        Some((params, output)) if !output.contains('|') => params,
        _ => head,
    };
    has_block && params.trim_end().ends_with('|')
}

/// The head of a block-owning expression line without its `{` or `:`.
pub(crate) fn strip_block_opener(text: &str) -> &str {
    strip_open_brace(strip_block_marker(text).0)
}

/// `text` without the `{` that ends its code; a `{` in a trailing `//` comment stays.
pub(crate) fn strip_open_brace(text: &str) -> &str {
    let text = text.trim();
    if without_trailing_line_comment(text).len() < text.len() {
        return text;
    }
    text.trim_end_matches('{').trim_end()
}

/// The closers, innermost first, of the groups `text` leaves open before the block it opens, as
/// `)` for `xs.map(|x|` or `})` for `S { a: g(|x|`.
pub(crate) fn closing_delimiters(text: &str) -> String {
    let text = text.trim_end();
    open_delimiters(text.strip_suffix('{').unwrap_or(text))
        .unwrap_or_default()
        .iter()
        .rev()
        .map(|open| match open {
            '(' => ')',
            '[' => ']',
            _ => '}',
        })
        .collect()
}

fn lower_basic_expr(text: &str) -> String {
    let text = &lower_view_blocks(text);
    // Std macro arguments see the parameters of the closures around them, as `env` in
    // `.map(|env| format!("{}", env.len()))`.
    let outer = closures(&text.chars().collect::<Vec<_>>());
    let text = map_std_macro_arguments(text, &|start, argument| {
        let names = outer
            .iter()
            .filter(|closure| closure.body.contains(&start))
            .flat_map(|closure| closure.parameters.clone())
            .collect();
        with_value_scope(names, || lower_expr(argument))
    });
    let chars = lower_format_macros(&text).chars().collect::<Vec<_>>();
    // A closure inside a macro body, as in `h!(o.map(|s| s.x))`, stays in its Rust tokens.
    let macros = macro_spans(&chars);
    let closures = closures(&chars)
        .into_iter()
        .filter(|closure| !macros.iter().any(|span| span.contains(&closure.body.start)))
        .collect::<Vec<_>>();
    // Each closure's parameters are values inside its body, as `env` in `.map(|env| *env)`, but
    // not after it.
    let lower_value = |start: usize, end: usize| {
        let mut bounds = closures
            .iter()
            .flat_map(|closure| [closure.body.start, closure.body.end])
            .filter(|&bound| start < bound && bound < end)
            .collect::<Vec<_>>();
        bounds.sort_unstable();
        bounds.dedup();
        std::iter::once(start)
            .chain(bounds.iter().copied())
            .zip(bounds.iter().copied().chain(std::iter::once(end)))
            .map(|(start, end)| {
                let names = closures
                    .iter()
                    .filter(|closure| closure.body.contains(&start))
                    .flat_map(|closure| closure.parameters.clone())
                    .collect();
                with_value_scope(names, || {
                    replace_dotted_paths(
                        &replace_square_generics(
                            &chars[start..end].iter().collect::<String>(),
                            GenericMode::Expr,
                        ),
                        PathMode::Expr,
                    )
                })
            })
            .collect::<String>()
    };
    let mut output = String::new();
    let mut start = 0usize;
    for (type_start, type_end) in type_annotation_spans(&chars) {
        output.push_str(&lower_value(start, type_start));
        output.push_str(&lower_signature(
            &chars[type_start..type_end].iter().collect::<String>(),
        ));
        start = type_end;
    }
    output.push_str(&lower_value(start, chars.len()));
    unwrap_path_receivers(&output)
}

/// `rust` without the parentheses around a lone path receiver, as in `(items).a()`,
/// `(Ordering::Less).is_lt()`, or `(TABLE)[i]`, which Renk reads as a value but Rust needs no
/// parentheses for; call arguments and macro delimiters, as in `g(x).y` or `m!(x).y`, keep theirs.
fn unwrap_path_receivers(rust: &str) -> String {
    let chars = rust.chars().collect::<Vec<_>>();
    // The `)` closing a lone path such as `a::B` or `r#type` opened at `open`.
    let lone_path_end = |open: usize| {
        let close = open
            + 1
            + chars[open + 1..]
                .iter()
                .take_while(|&&c| is_ident_continue(c) || matches!(c, ':' | '#'))
                .count();
        let text = chars[open + 1..close].iter().collect::<String>();
        let path = text.strip_prefix("::").unwrap_or(&text);
        let is_path = path.split("::").all(|segment| {
            let segment = segment.strip_prefix("r#").unwrap_or(segment);
            segment.chars().next().is_some_and(is_ident_start)
                && segment.chars().all(is_ident_continue)
        });
        (is_path && chars.get(close) == Some(&')')).then_some(close)
    };
    // A group after a callee, as in `g(x)`, `m!(x)`, or `f::<T>(x)`, holds arguments; after a
    // space, only a keyword such as `return` or `mut` precedes a value.
    let follows_value_position = |at: usize| {
        let before = chars[..at].iter().rev();
        let spaces = before.clone().take_while(|c| c.is_whitespace()).count();
        let word = before
            .skip(spaces)
            .take_while(|&&c| is_ident_continue(c))
            .collect::<String>();
        match at.checked_sub(1).map(|before| chars[before]) {
            None => true,
            Some(c) if is_ident_continue(c) || matches!(c, ')' | ']' | '>' | '!') => false,
            Some(_) if spaces > 0 && !word.is_empty() => matches!(
                &*word.chars().rev().collect::<String>(),
                "return" | "in" | "if" | "match" | "while" | "break" | "else" | "mut"
            ),
            Some(_) => true,
        }
    };
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) =
            opaque_span_end(&chars, index).or_else(|| macro_invocation_end(&chars, index))
        {
            output.extend(&chars[index..=end]);
            index = end + 1;
            continue;
        }
        if chars[index] == '('
            && let Some(close) = lone_path_end(index)
            && matches!(chars.get(close + 1), Some('.' | '['))
            && follows_value_position(index)
        {
            output.extend(&chars[index + 1..close]);
            index = close + 1;
            continue;
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

/// Type positions inside expressions: `let`/`static`/`const` annotations, closure
/// parameter/return types, and `as` cast targets.
fn type_annotation_spans(chars: &[char]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut index = 0usize;
    let text = chars.iter().collect::<String>();
    if is_let(&text) || starts_item(&text, "static") || starts_item(&text, "const") {
        let pattern_end = top_level_position(chars, 1, |at| {
            chars[at] == '='
                && !matches!(chars.get(at + 1), Some('=' | '>'))
                && !matches!(chars[at - 1], '=' | '!' | '<' | '>')
        })
        .unwrap_or(chars.len());
        if let Some(colon) =
            top_level_position(&chars[..pattern_end], 0, |at| is_single_colon(chars, at))
        {
            spans.push((colon + 1, pattern_end));
            index = pattern_end;
        }
    }
    while index < chars.len() {
        if let Some(end) =
            opaque_span_end(chars, index).or_else(|| macro_invocation_end(chars, index))
        {
            index = end + 1;
            continue;
        }
        // Generic arguments such as `[(<T as Tr>::Out, usize)]` in `f[...](x)` lower as one type
        // list in `replace_square_generics`.
        if is_ident_start(chars[index])
            && (index == 0 || !is_ident_continue(chars[index - 1]))
            && let Some((close, _)) = generic_brackets(chars, index, read_ident(chars, index))
        {
            index = close + 1;
            continue;
        }
        if index > 0 && keyword_at(chars, index, "let") && chars.get(index + 3) == Some(&' ') {
            // A `let` inside a block of a group, as in `f(unsafe {` / `let x: T = ...`.
            let pattern_end = top_level_position(chars, index + 3, |at| {
                chars[at] == ';'
                    || (chars[at] == '='
                        && !matches!(chars.get(at + 1), Some('=' | '>'))
                        && !matches!(chars[at - 1], '=' | '!' | '<' | '>'))
            })
            .unwrap_or(chars.len());
            if let Some(colon) = top_level_position(&chars[..pattern_end], index + 3, |at| {
                is_single_colon(chars, at)
            }) {
                spans.push((colon + 1, pattern_end));
            }
            index = pattern_end;
            continue;
        }
        // The self type of a qualified path such as `<Vec[u8] as PartialEq[Vec[u8]]>::eq` or
        // `<Vec[u8]>::new`.
        if chars[index] == '<'
            && chars
                .get(index + 1)
                .is_some_and(|next| !next.is_whitespace() && *next != '=')
            && chars[..index]
                .iter()
                .rfind(|character| !character.is_whitespace())
                .is_none_or(|before| !is_ident_continue(*before) && !matches!(before, ')' | ']'))
            && let Some(as_at) = top_level_position(chars, index + 1, |at| {
                chars[at] == '>' || keyword_at(chars, at, "as")
            })
            .filter(|&at| chars[at] != '>' || chars[at + 1..].starts_with(&[':', ':']))
        {
            spans.push((index + 1, as_at));
            index = as_at;
            continue;
        }
        if keyword_at(chars, index, "as") && chars.get(index + 2) == Some(&' ') {
            let end = cast_type_end(chars, index + 3);
            spans.push((index + 3, end));
            index = end;
            continue;
        }
        if chars[index] != '|' || !is_closure_start(chars, index) {
            index += 1;
            continue;
        }
        let params_end = if chars.get(index + 1) == Some(&'|') {
            index + 1
        } else {
            let Some(end) = top_level_position(chars, index + 1, |at| chars[at] == '|') else {
                break;
            };
            let mut param_start = index + 1;
            while param_start < end {
                let param_end =
                    top_level_position(&chars[..end], param_start, |at| chars[at] == ',')
                        .unwrap_or(end);
                if let Some(colon) = top_level_position(&chars[..param_end], param_start, |at| {
                    is_single_colon(chars, at)
                }) {
                    spans.push((colon + 1, param_end));
                }
                param_start = param_end + 1;
            }
            end
        };
        let mut cursor = params_end + 1;
        skip_whitespace_chars(chars, &mut cursor);
        index = params_end + 1;
        if chars[cursor..].starts_with(&['-', '>']) {
            let return_end =
                top_level_position(chars, cursor + 2, |at| chars[at] == '{').unwrap_or(chars.len());
            spans.push((cursor + 2, return_end));
            // Spans stay ordered: the scan resumes after the return type, at its body.
            index = return_end;
        }
    }
    spans
}

/// The end of the cast type starting at `start`, such as `*const [Bucket[K, V]]` in
/// `entries as *const [Bucket[K, V]] as *const Self`.
fn cast_type_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < chars.len() {
        match chars[index] {
            '[' | '(' => depth += 1,
            ']' | ')' if depth == 0 => break,
            ']' | ')' => depth -= 1,
            '.' if depth == 0 && chars.get(index + 1) == Some(&'.') => break,
            ' ' if depth == 0 => {
                // `*const T`, `&mut T`, `dyn T`, and `&'a T` continue after their space.
                let word = chars[start..index]
                    .iter()
                    .rev()
                    .take_while(|character| !matches!(character, ' ' | '&' | '*'))
                    .collect::<String>();
                if !matches!(&*word, "tsnoc" | "tum" | "nyd") && !word.ends_with('\'') {
                    break;
                }
            }
            character
                if depth == 0
                    && !is_ident_continue(character)
                    && !matches!(character, '.' | ':' | '&' | '*' | '\'') =>
            {
                break;
            }
            _ => {}
        }
        index += 1;
    }
    index
}

fn lower_if_then_else(text: &str) -> Option<String> {
    let (prefix, condition_and_body) = if let Some(rest) = text.strip_prefix("if ") {
        ("if", rest)
    } else {
        ("else if", text.strip_prefix("else if ")?)
    };

    let then_index = find_top_level_keyword(condition_and_body, "then")?;

    let condition = condition_and_body[..then_index].trim();
    let body = condition_and_body[then_index + "then".len()..].trim();
    if body.is_empty() {
        return Some(format!("{} {}", prefix, lower_basic_expr(condition)));
    }

    let condition = lower_basic_expr(condition);
    if let Some(else_index) = find_else_for_then_body(body) {
        let then_expr = body[..else_index].trim();
        let else_expr = body[else_index + "else".len()..].trim();
        let else_branch = match lower_if_then_else(else_expr) {
            Some(chained) if chained.starts_with("if ") => chained,
            _ => format!("{{ {} }}", lower_expr(else_expr)),
        };
        Some(format!(
            "{} {} {{ {} }} else {}",
            prefix,
            condition,
            lower_expr(then_expr),
            else_branch
        ))
    } else {
        Some(format!(
            "{} {} {{ {} }}",
            prefix,
            condition,
            lower_expr(body)
        ))
    }
}

fn find_else_for_then_body(text: &str) -> Option<usize> {
    let chars = text.chars().collect::<Vec<_>>();
    let byte_offsets = text
        .char_indices()
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    let mut round = 0usize;
    let mut square = 0usize;
    let mut curly = 0usize;
    let mut nested_if_count = 0usize;
    let mut index = 0usize;

    while index < chars.len() {
        if let Some(end) =
            opaque_span_end(&chars, index).or_else(|| macro_invocation_end(&chars, index))
        {
            index = end + 1;
            continue;
        }

        let character = chars[index];
        match character {
            '(' => round += 1,
            ')' => round = round.saturating_sub(1),
            '[' => square += 1,
            ']' => square = square.saturating_sub(1),
            '{' => curly += 1,
            '}' => curly = curly.saturating_sub(1),
            _ => {}
        }

        if round == 0 && square == 0 && curly == 0 {
            if keyword_at(&chars, index, "if") {
                nested_if_count += 1;
            } else if keyword_at(&chars, index, "else") {
                if nested_if_count == 0 {
                    return byte_offsets.get(index).copied();
                }
                nested_if_count -= 1;
            }
        }

        index += 1;
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenericMode {
    Type,
    Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathMode {
    Type,
    Expr,
}

fn replace_square_generics(text: &str, mode: GenericMode) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::new();
    let mut index = 0usize;
    let mut square_const = Vec::<bool>::new();
    let mut curly_depth = 0usize;
    while index < chars.len() {
        if let Some(end) =
            opaque_span_end(&chars, index).or_else(|| macro_invocation_end(&chars, index))
        {
            output.extend(chars[index..=end].iter());
            index = end + 1;
            continue;
        }

        let in_type_const_expr = mode == GenericMode::Type
            && (curly_depth > 0 || square_const.last().copied().unwrap_or(false));

        if (mode == GenericMode::Expr || in_type_const_expr)
            && is_ident_start(chars[index])
            && (index == 0 || !is_ident_continue(chars[index - 1]))
        {
            let name_end = read_ident(&chars, index);
            if let Some((close, dot_path)) = generic_brackets(&chars, index, name_end) {
                let content: String = chars[name_end + 1..close].iter().collect();
                output.extend(chars[index..name_end].iter());
                output.push_str("::<");
                output.push_str(&lower_generic_content(&content));
                output.push('>');
                if dot_path {
                    output.push_str("::");
                }
                index = close + 1 + usize::from(dot_path);
                continue;
            }
            output.extend(chars[index..name_end].iter());
            index = name_end;
            continue;
        }

        // Type-only positions read `Name[T]` as generics. Once a type grammar enters a const
        // expression, the expression rules above apply again.
        if mode == GenericMode::Type
            && !in_type_const_expr
            && chars[index] == '['
            && has_ident_before(&chars, index)
            && let Some(close) = matching_bracket(&chars, index, '[', ']')
        {
            let content: String = chars[index + 1..close].iter().collect();
            let inner = lower_generic_content(&content);
            output.push('<');
            output.push_str(&inner);
            output.push('>');
            index = close + 1;
            continue;
        }

        match chars[index] {
            '[' => square_const.push(false),
            ';' if !square_const.is_empty() => {
                if let Some(is_const) = square_const.last_mut() {
                    *is_const = true;
                }
            }
            ']' => {
                square_const.pop();
            }
            '{' => curly_depth += 1,
            '}' => curly_depth = curly_depth.saturating_sub(1),
            _ => {}
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

/// The closing `]` of generic arguments after the identifier `chars[start..name_end]`, and
/// whether a `.` follows them. `name[...](` is a generic call unless a local value, as a `let` or
/// parameter, binds `name`. An uppercase `Name[...]` before `::` or `.name` is a generic path.
/// `(xs[i])(` and `(NAME)[i].name` force indexing.
fn generic_brackets(chars: &[char], start: usize, name_end: usize) -> Option<(usize, bool)> {
    if chars.get(name_end) != Some(&'[') {
        return None;
    }
    let close = matching_bracket(chars, name_end, '[', ']')?;
    let after = &chars[close + 1..];
    // A CamelCase segment of a dotted path, as in `std.vec.Vec[u8].new()`, is a type.
    let name = chars[start..name_end].iter().collect::<String>();
    let path_head = chars[start].is_uppercase()
        && !is_screaming_case(&name)
        && (start == 0
            || chars[start - 1] != '.'
            || (is_camel_case(&name) && {
                let mut segments = dotted_segments_before(chars, start);
                segments.push(name.clone());
                should_convert_path(&segments, PathMode::Expr, None)
            }));
    let dot_path = path_head
        && after.first() == Some(&'.')
        && after.get(1).copied().is_some_and(is_ident_start);
    let colon_path = path_head && after.starts_with(&[':', ':']);
    let call = after.first() == Some(&'(')
        && !((start == 0 || chars[start - 1] != '.') && is_local_value(&name));
    (call || dot_path || colon_path).then_some((close, dot_path))
}

/// The `a.b.` identifiers right before `index`, in order.
fn dotted_segments_before(chars: &[char], mut index: usize) -> Vec<String> {
    let mut segments = Vec::new();
    while index > 1 && chars[index - 1] == '.' && is_ident_continue(chars[index - 2]) {
        let end = index - 1;
        let mut start = end;
        while start > 0 && is_ident_continue(chars[start - 1]) {
            start -= 1;
        }
        segments.insert(0, chars[start..end].iter().collect());
        index = start;
    }
    segments
}

pub(crate) fn lower_generic_content(content: &str) -> String {
    replace_dotted_paths(
        &replace_square_generics(content, GenericMode::Type),
        PathMode::Type,
    )
}

fn replace_dotted_paths(text: &str, mode: PathMode) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::new();
    let mut index = 0usize;
    let mut square_const = Vec::<bool>::new();
    let mut curly_depth = 0usize;
    while index < chars.len() {
        if let Some(end) =
            opaque_span_end(&chars, index).or_else(|| macro_invocation_end(&chars, index))
        {
            output.extend(chars[index..=end].iter());
            index = end + 1;
            continue;
        }

        // A number such as `0.0f64` keeps its suffix, so `0.0f64.to_bits()` stays a method call.
        if chars[index].is_ascii_digit() {
            let start = index;
            while index < chars.len()
                && (is_ident_continue(chars[index])
                    || (chars[index] == '.'
                        && chars.get(index + 1).is_some_and(char::is_ascii_digit)))
            {
                index += 1;
            }
            output.extend(chars[start..index].iter());
            continue;
        }

        if is_ident_start(chars[index]) {
            let start = index;
            let explicit_path_before = start >= 2 && chars[start - 2..start] == [':', ':'];
            // A member of a value, as `inner` in `make().inner.len()`, but not a range end.
            let member =
                start >= 1 && chars[start - 1] == '.' && (start < 2 || chars[start - 2] != '.');
            let mut segments = Vec::new();
            let mut dot_positions = Vec::new();
            let mut explicit_path = explicit_path_before;
            let mut before_first_dot = None;
            let first_end = read_ident(&chars, index);
            segments.push(chars[start..first_end].iter().collect::<String>());
            index = first_end;

            loop {
                if dot_positions.is_empty()
                    && chars[index..].starts_with(&[':', ':'])
                    && chars.get(index + 2).copied().is_some_and(is_ident_start)
                {
                    explicit_path = true;
                    let segment_end = read_ident(&chars, index + 2);
                    segments.push(chars[index + 2..segment_end].iter().collect::<String>());
                    index = segment_end;
                    continue;
                }
                if index + 2 < chars.len()
                    && chars[index] == ':'
                    && chars[index + 1] == ':'
                    && chars[index + 2] == '<'
                    && let Some(close) = matching_bracket(&chars, index + 2, '<', '>')
                {
                    index = close + 1;
                    continue;
                }
                if chars.get(index) == Some(&'<')
                    && let Some(close) = matching_bracket(&chars, index, '<', '>')
                {
                    index = close + 1;
                    continue;
                }
                if index + 1 < chars.len()
                    && chars[index] == '.'
                    && is_ident_start(chars[index + 1])
                {
                    before_first_dot
                        .get_or_insert_with(|| segments.last().cloned().unwrap_or_default());
                    dot_positions.push(index);
                    let segment_start = index + 1;
                    let segment_end = read_ident(&chars, segment_start);
                    segments.push(chars[segment_start..segment_end].iter().collect::<String>());
                    index = segment_end;
                    continue;
                }
                break;
            }

            let in_type_const_expr = mode == PathMode::Type
                && (curly_depth > 0 || square_const.last().copied().unwrap_or(false));
            // After an explicit `::` path only a CamelCase segment is a type whose `.name` is a
            // path; `Config::DEFAULT.len()` and `(Ordering::Less).reverse()` keep value receivers.
            let should_convert = !dot_positions.is_empty()
                && !in_type_const_expr
                && !member
                && if explicit_path {
                    mode == PathMode::Type || before_first_dot.as_deref().is_some_and(is_camel_case)
                } else {
                    should_convert_path(&segments, mode, chars.get(index).copied())
                };
            if should_convert {
                let mut part_start = start;
                for dot in dot_positions {
                    output.extend(chars[part_start..dot].iter());
                    output.push_str("::");
                    part_start = dot + 1;
                }
                output.extend(chars[part_start..index].iter());
            } else {
                output.extend(chars[start..index].iter());
            }
        } else {
            match chars[index] {
                '[' => square_const.push(false),
                ';' if !square_const.is_empty() => {
                    if let Some(is_const) = square_const.last_mut() {
                        *is_const = true;
                    }
                }
                ']' => {
                    square_const.pop();
                }
                '{' => curly_depth += 1,
                '}' => curly_depth = curly_depth.saturating_sub(1),
                _ => {}
            }
            output.push(chars[index]);
            index += 1;
        }
    }
    output
}

pub(crate) fn type_like_generic(content: &str) -> bool {
    let trimmed = content.trim();
    !trimmed.is_empty()
        && !trimmed.chars().all(|character| character.is_ascii_digit())
        && generic_tokens(trimmed).into_iter().any(|token| {
            token == "_"
                || is_builtin_type(token)
                || token
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_uppercase())
        })
}

pub(crate) fn generic_tokens(content: &str) -> Vec<&str> {
    content
        .split(|character: char| !is_ident_continue(character))
        .filter(|token| !token.is_empty())
        .collect()
}

fn is_builtin_type(token: &str) -> bool {
    matches!(
        token,
        "bool"
            | "char"
            | "str"
            | "String"
            | "usize"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "f32"
            | "f64"
    )
}

/// Whether `a.b` reads as a path: unless `a` is `self`, a constant (`SCREAMING_CASE`), or a
/// value bound around it (a `let`, parameter, or pattern), `a` names a type, module, or crate.
pub(crate) fn should_convert_path(segments: &[String], mode: PathMode, next: Option<char>) -> bool {
    match mode {
        PathMode::Type => true,
        PathMode::Expr => {
            let first = segments.first().map(String::as_str).unwrap_or_default();
            next == Some('!')
                || (first != "self" && !is_screaming_case(first) && !is_value_bound(first))
        }
    }
}

/// A constant or static name such as `MAX` or `TABLE_2`; a single letter like `T` is a type.
pub(crate) fn is_screaming_case(name: &str) -> bool {
    name.chars().count() > 1
        && name.starts_with(char::is_uppercase)
        && name
            .chars()
            .all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
}
