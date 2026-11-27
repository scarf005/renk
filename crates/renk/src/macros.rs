//! Format macro arguments and `view!` blocks, in both directions.

use super::*;

pub(crate) fn lower_format_macros(text: &str) -> String {
    rewrite_format_macros(text, lower_format_invocation)
}

/// Std format macros and how many arguments precede their format string.
fn format_macro_prefix(name: &str) -> Option<usize> {
    match name {
        "format" | "format_args" | "print" | "println" | "eprint" | "eprintln" | "panic"
        | "unreachable" | "todo" | "unimplemented" => Some(0),
        "write" | "writeln" | "assert" | "debug_assert" => Some(1),
        "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne" => Some(2),
        _ => None,
    }
}

/// Std macros whose arguments are expressions, or a pattern in `matches!`, so Renk syntax in them
/// lowers like any other expression; other macro token trees stay Rust tokens.
fn is_std_expression_macro(name: &str) -> bool {
    format_macro_prefix(name).is_some() || matches!(name, "dbg" | "matches" | "vec")
}

/// Whether a macro name after `before`, read nearest character first, is a std macro path:
/// `write`, `std::write`, `std.write` and `::core::write` are, `defmt::write` and `log.write` are
/// not.
fn is_std_macro_path(before: impl Iterator<Item = char>) -> bool {
    let mut before = before.peekable();
    if before.next_if_eq(&'.').is_none()
        && (before.next_if_eq(&':').is_none() || before.next_if_eq(&':').is_none())
    {
        return true;
    }
    let root = std::iter::from_fn(|| before.next_if(|character| is_ident_continue(*character)))
        .collect::<Vec<_>>();
    // The root must not follow another segment, as in `a::std::write` or `a.std.write`.
    ["std", "core"]
        .iter()
        .any(|name| name.chars().rev().eq(root.iter().copied()))
        && match before.next() {
            Some('.') => false,
            Some(':') => before.nth(1).is_none_or(|c| !is_ident_continue(c)),
            _ => true,
        }
}

/// Multi-line `{` blocks in a `view!` body, as in a Leptos handler `on:click=move |_| {`, hold
/// Renk statements; the rest of the body stays Rust tokens.
pub(crate) fn lower_view_blocks(text: &str) -> String {
    map_view_blocks(text, &lower_view_block)
}

/// `text` with the text between the braces of each multi-line `{` block in a `view!` body
/// replaced by `block(inner, names)`, where `names` are the parameters of the closures the block
/// is the body of; nested blocks are `block`'s to map.
fn map_view_blocks(text: &str, block: &dyn Fn(&str, Vec<String>) -> String) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            output.extend(&chars[index..=end]);
            index = end + 1;
            continue;
        }
        let path_start =
            index == 0 || !(is_ident_continue(chars[index - 1]) || chars[index - 1] == ':');
        let Some(end) = path_start
            .then(|| macro_invocation_end(&chars, index))
            .flatten()
        else {
            output.push(chars[index]);
            index += 1;
            continue;
        };
        let bang = (index..end).find(|&at| chars[at] == '!').unwrap_or(end);
        let open = (bang..end)
            .find(|&at| matches!(chars[at], '(' | '[' | '{'))
            .unwrap_or(end);
        let path = chars[index..bang].iter().collect::<String>();
        output.extend(&chars[index..=open]);
        let body = chars[open + 1..end].iter().collect::<String>();
        if path.trim_end().rsplit("::").next() == Some("view") {
            output.push_str(&map_view_body(&body, true, block));
        } else {
            output.push_str(&body);
        }
        output.push(chars[end]);
        index = end + 1;
    }
    output
}

/// A `view!` body with its multi-line `{` blocks and nested `view!` invocations mapped as
/// [`map_view_blocks`] says. `markup` is false for the Rust code in braces in such a body, which
/// has no tags.
fn map_view_body(body: &str, markup: bool, block: &dyn Fn(&str, Vec<String>) -> String) -> String {
    let chars = body.chars().collect::<Vec<_>>();
    let closures = closures(&chars);
    let mut output = String::new();
    let mut index = 0usize;
    // The last char before `index` outside whitespace and comments, and its index, with `"` for
    // a string.
    let mut previous = None::<(char, usize)>;
    // The keyword that heads the statement in each open `(` or `[`, the innermost last. A `,`, a
    // `;`, a `{` block, a match arm's `=>`, and the `=` of an attribute such as `for=x` end it,
    // and so does a line end, unless the line ends with an operator or the next one starts with
    // one, a `.`, or a `?`, as `&& ok {` after `if n > 0`. The `=` of `if let` and the pattern
    // of `if let`, `while let`, or `for` do not end it.
    let mut heads = vec![None::<&str>];
    // The depth of generic `<` in each open `(` or `[` while a head is set or in a tag, as in
    // `Ok::<Vec<u8>,` or `|ev: Box<dyn Fn()>|`, so its `,` and `>` end no head. Outside groups,
    // only the generics of a tag name, as `<Comp<T>`, keep a `>` from ending the tag.
    let mut angles = vec![0usize];
    // Whether `index` is in a tag, as `<p class=x>`, or in the text after one, as `Find a match`
    // in `<p>Find a match</p>`; `None` before the first tag and in code without tags.
    let mut tag = None::<bool>;
    // The index just after the name of the current tag, as `Comp` in `<Comp<T>`, where a `<`
    // opens the name's generics.
    let mut name_end = 0usize;
    while index < chars.len() {
        let top = heads.len() - 1;
        if heads[top].is_none() && tag != Some(true) {
            angles[top] = 0;
        }
        if let Some(end) = opaque_span_end(&chars, index) {
            output.extend(&chars[index..=end]);
            if !(chars[index] == '/' && matches!(chars.get(index + 1), Some('/' | '*'))) {
                previous = Some(('"', end));
            }
            index = end + 1;
        } else if let Some(end) = macro_invocation_end(&chars, index)
            .filter(|_| index == 0 || !is_ident_continue(chars[index - 1]))
        {
            let invocation = chars[index..=end].iter().collect::<String>();
            output.push_str(&map_view_blocks(&invocation, block));
            previous = Some((chars[end], end));
            index = end + 1;
        } else if chars[index] == '{'
            && let Some(close) = matching_bracket(&chars, index, '{', '}')
        {
            let inner = chars[index + 1..close].iter().collect::<String>();
            // The parameters of the closure whose body the block is, as `ev` in `move |ev| {`.
            let names = closures
                .iter()
                .filter(|closure| {
                    closure.body.start <= index
                        && chars[closure.body.start..index]
                            .iter()
                            .all(|c| c.is_whitespace())
                })
                .flat_map(|closure| closure.parameters.clone())
                .collect();
            // A block after text, as in `<p>Find a match {`, is a child.
            let opens = || {
                top == 0 && tag == Some(false)
                    || view_block_opens(&chars, index, previous, heads[top], &closures)
            };
            output.push('{');
            output.push_str(&if inner.contains('\n') && opens() {
                block(&inner, names)
            } else {
                map_view_body(&inner, false, block)
            });
            output.push('}');
            // A pattern, as `Point { x, y }` in `if let Point { x, y } = p {`, keeps the head.
            heads[top] = heads[top].filter(|&head| matches!(head, "if let" | "while let" | "for"));
            previous = Some(('}', close));
            index = close + 1;
        } else if is_ident_start(chars[index])
            && (index == 0 || !is_ident_continue(chars[index - 1]))
        {
            let end = read_ident(&chars, index);
            let word = chars[index..end].iter().collect::<String>();
            let keywords = [
                "if", "while", "for", "else", "async", "unsafe", "loop", "match",
            ];
            // Text, as `match` in `<p>Find a match</p>`, or an attribute name, as `for` in
            // `<label for>` or `<label class={a} for>`, and not a value, as `if` in
            // `on:click=move |_| if`, or the `else` of an `if` value.
            let markup_word = top == 0
                && match tag {
                    Some(false) => true,
                    Some(true) => previous.is_some_and(|(c, _)| {
                        is_ident_continue(c)
                            || matches!(c, '"' | ')' | ']')
                            || c == '}' && word != "else"
                    }),
                    None => false,
                };
            heads[top] = match (heads[top], word.as_str()) {
                (None, _) if markup_word => None,
                // A bound, as `for<'a> Fn(&'a u8)`.
                (head, "for") if chars.get(end) == Some(&'<') => head,
                (Some("if"), "let") => Some("if let"),
                (Some("while"), "let") => Some("while let"),
                (Some("for"), "in") => Some("for in"),
                (head, word) => keywords
                    .into_iter()
                    .find(|keyword| *keyword == word)
                    .or(head),
            };
            output.extend(&chars[index..end]);
            previous = Some((chars[end - 1], end - 1));
            index = end;
        } else {
            let at = index;
            let next = chars.get(at + 1).copied();
            let assigns = chars[at] == '='
                && !matches!(next, Some('=' | '>'))
                && (at == 0 || !matches!(chars[at - 1], '=' | '!' | '<' | '>'));
            // Whether the line ending at `at` ends with an operator, or the next starts with one,
            // or with the `{` of a keyword body, as rustfmt puts it after a wrapped condition. A
            // `<` or `>` after a space counts only in a condition, so `<video loop >` ends a tag.
            let continues = || {
                let condition = matches!(
                    heads[top],
                    Some("if" | "if let" | "while" | "while let" | "for in" | "match")
                );
                let ends = previous.is_some_and(|(c, last)| {
                    let pair = chars[last.saturating_sub(1)..=last]
                        .iter()
                        .collect::<String>();
                    matches!(c, '+' | '-' | '*' | '/' | '%' | '=')
                        || ["&&", "||", "!=", "<=", ">="].contains(&pair.as_str())
                        || condition && [" <", " >"].contains(&pair.as_str())
                });
                let line = chars[at + 1..]
                    .iter()
                    .skip_while(|c| matches!(c, ' ' | '\t'))
                    .take(3)
                    .collect::<String>();
                ends || starts_with_binary_operator(&line)
                    || line.starts_with(['.', '?'])
                    || heads[top].is_some() && line.starts_with('{')
            };
            let starts_tag = || {
                next.is_some_and(|c| is_ident_start(c) || matches!(c, '/' | '>'))
                    && (tag == Some(false)
                        || previous.is_none_or(|(c, _)| !is_ident_continue(c) && c != ':'))
            };
            let after_path = |i: usize| is_ident_continue(chars[i]) || chars[i] == ':';
            match chars[at] {
                '<' if markup
                    && top == 0
                    && tag != Some(true)
                    && heads[top].is_none()
                    && starts_tag() =>
                {
                    tag = Some(true);
                    name_end = (at + 1..chars.len())
                        .find(|&i| !after_path(i))
                        .unwrap_or(chars.len())
                }
                '(' | '[' => {
                    heads.push(None);
                    angles.push(0);
                }
                ')' | ']' if heads.len() > 1 => {
                    heads.pop();
                    angles.pop();
                }
                // A generic, as `Vec<u8>`, or a qualified path in one, as the second `<` of
                // `Vec::<<T as Tr>::X>`.
                '<' if at > 0
                    && (after_path(at - 1)
                        || at > 1 && chars[at - 1] == '<' && after_path(at - 2))
                    && !matches!(next, Some('/' | '='))
                    && (tag != Some(true) || top > 0 || angles[top] > 0 || at == name_end) =>
                {
                    angles[top] += 1
                }
                '>' if angles[top] > 0 && !matches!(chars[at - 1], '-' | '=') => angles[top] -= 1,
                // A tag end, as in Leptos: the first `>` outside groups, literals, comments, and
                // the tag name's generics, also in `n > 5`, `->`, or `=>`, but not in the `->` of
                // `<Comp<F: Fn() -> u8>`. Text after it, as `> more`, may hold a `>`.
                '>' if tag == Some(true) && top == 0 && angles[top] == 0 => {
                    tag = Some(false);
                    heads[top] = None
                }
                ',' if angles[top] > 0 => {}
                ',' | ';' | ')' | ']' | '}' => heads[top] = None,
                '=' if next == Some('>') => heads[top] = None,
                '=' if assigns => {
                    heads[top] = match heads[top] {
                        Some("if" | "if let") => Some("if"),
                        Some("while" | "while let") => Some("while"),
                        _ => None,
                    }
                }
                '\n' if !continues() => heads[top] = None,
                _ => {}
            }
            output.push(chars[at]);
            if !chars[at].is_whitespace() {
                previous = Some((chars[at], at));
            }
            index += 1;
        }
    }
    output
}

/// Whether the `{` at `open` in a `view!` body opens a block: the body of the keyword `head` of
/// its statement, as `if` in `if n > 1 {` or `else`, a child or attribute value, as `<p>{`,
/// `"Value: " {`, or `class={`, or the body of a closure or `match` arm. Struct literals and
/// patterns, `match` arms, and element bodies, as `Point {`, `Pair::<u8> {`, `if let Point {`,
/// `match n {`, and Sycamore's `div(class="x") {`, are not. `previous` is the last char before
/// `open` outside whitespace and comments, and its index, with `"` for a string.
fn view_block_opens(
    chars: &[char],
    open: usize,
    previous: Option<(char, usize)>,
    head: Option<&str>,
    closures: &[Closure],
) -> bool {
    if let Some(head) = head {
        return !matches!(head, "match" | "if let" | "while let" | "for");
    }
    // Whether the `>` at `close` ends a turbofish, as in `Pair::<u8> {`, and not `=>` or `->`.
    let turbofish = |close: usize| {
        let mut depth = 0usize;
        for at in (0..=close).rev().take_while(|&at| chars[at] != '\n') {
            match chars[at] {
                '>' if at == 0 || !matches!(chars[at - 1], '=' | '-') => depth += 1,
                '>' if at == close => return false,
                '<' if depth == 1 => return chars[..at].ends_with(&[':', ':']),
                '<' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        false
    };
    let follows_closure = |closure: &Closure| {
        let start = closure.body.start;
        let mut head = chars[start.min(open)..open]
            .iter()
            .skip_while(|c| c.is_whitespace());
        start <= open
            && head
                .next()
                .is_none_or(|&c| c == '-' && head.next() == Some(&'>'))
    };
    let child = match previous {
        None => true,
        Some(('>', close)) => !turbofish(close),
        Some((c, _)) => matches!(c, '"' | '=' | '(' | '[' | '{' | '}' | ',' | ';'),
    };
    child || closures.iter().any(follows_closure)
}

/// A Rust `line` with each line of a multi-line `{` block in a `view!` body written as Renk,
/// without its `;`, or with dotted paths when `dot`, where the block still lowers to the same Rust.
pub(crate) fn view_blocks_as_renk(line: &str, dot: bool) -> Option<String> {
    let renk = map_view_blocks(line, &|inner, names| {
        let lower = |lines: &[String]| lower_view_block(&lines.join("\n"), names.clone());
        let mut lines = inner.split('\n').map(str::to_string).collect::<Vec<_>>();
        let expected = lower(&lines);
        let in_literal = lines_ending_in_literal(inner);
        let rewrite = |line: &str| {
            let (code, comment) = split_trailing_comment(line);
            let code = match dot {
                true => dot_rust_paths(code).unwrap_or_else(|| code.to_string()),
                false => strip_rust_semicolon(code),
            };
            format!("{code}{comment}")
        };
        let pending = (0..lines.len())
            .filter(|&index| {
                !in_literal.get(index).copied().unwrap_or(false)
                    && !is_comment(split_trailing_comment(&lines[index]).0.trim())
                    && rewrite(&lines[index]) != lines[index]
            })
            .collect::<Vec<_>>();
        // Lines are rewritten a group at a time. A group that changes the Rust drops the lines
        // whose own lowered line changed, as the last line before a `}`, and tries again; a group
        // with no such line is halved. So a block lowers about once more per line that must stay.
        let expected_lines = expected.split('\n').collect::<Vec<_>>();
        let mut groups = vec![pending];
        while let Some(group) = groups.pop() {
            let mut trial = lines.clone();
            for &index in &group {
                trial[index] = rewrite(&lines[index]);
            }
            let lowered = lower(&trial);
            if lowered == expected {
                lines = trial;
                continue;
            }
            let lowered = lowered.split('\n').collect::<Vec<_>>();
            // Lowered lines match the block's lines by index only when all three counts agree.
            let aligned = lowered.len() == lines.len() && expected_lines.len() == lines.len();
            let (changed, kept): (Vec<_>, Vec<_>) = group
                .iter()
                .partition(|&&index| aligned && lowered[index] != expected_lines[index]);
            if !changed.is_empty() {
                groups.extend((!kept.is_empty()).then_some(kept));
            } else if let [_, _, ..] = group[..] {
                let (first, second) = group.split_at(group.len() / 2);
                groups.extend([second.to_vec(), first.to_vec()]);
            }
        }
        lines.join("\n")
    });
    (renk != line).then_some(renk)
}

/// `inner`, the text between the braces of a multi-line `{` block in a `view!` body, with its
/// lines emitted as the statements of a Renk block whose last line is its value, where `names`
/// are values. Each line keeps its place and the whitespace around its code; text that does not
/// lower line for line stays as written.
fn lower_view_block(inner: &str, names: Vec<String>) -> String {
    let lines = inner.split('\n').collect::<Vec<_>>();
    let rest = lines[1..].iter().filter(|line| !line.trim().is_empty());
    let closing = rest
        .clone()
        .filter(|line| line.trim_start().starts_with([')', ']', '}']))
        .map(|line| raw_indent(line))
        .min();
    let statements = rest
        .filter(|line| !line.trim_start().starts_with([')', ']', '}']))
        .map(|line| raw_indent(line))
        .min();
    // Text after the `{`, as `move || {` in `{move || {`, sits at the indent of the line closing
    // what it opens, or just outside the continuation lines after it, as `.max(1)`; a comment or a
    // statement sits with the statements.
    let head = lines[0].trim();
    let continued = lines[1..]
        .iter()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|next| {
            next.trim_start().starts_with(['.', '?']) || starts_with_binary_operator(next.trim())
        });
    let indent = match (closing, statements) {
        _ if is_comment(head) => statements.or(closing),
        (Some(closing), _) => Some(closing),
        (None, Some(statements)) if continued => Some(statements.saturating_sub(1)),
        (None, statements) => statements,
    };
    let source = std::iter::once("fn view():".to_string())
        .chain(lines.iter().enumerate().map(|(index, line)| match index {
            0 => format!("{}{head}", spaces(indent.unwrap_or(0).max(1))),
            _ => line.to_string(),
        }))
        .collect::<Vec<_>>()
        .join("\n");
    let Some([function]) = source_lines(&source)
        .and_then(|lines| parse_source_tree(&lines))
        .ok()
        .and_then(|nodes| <[Node; 1]>::try_from(nodes).ok())
    else {
        return inner.to_string();
    };
    let mut emitter = Emitter {
        source_lines: source.lines().map(str::to_string).collect(),
        ..Emitter::default()
    };
    let outer = BLOCK_BINDINGS.replace(names);
    emit_sequence(&function.children, Context::Block, true, &mut emitter);
    BLOCK_BINDINGS.set(outer);
    for (line, comment) in &emitter.comments {
        emitter.lines[*line].push_str(comment);
    }
    emitter.join_split_lines();
    let mut emitted = emitter.lines.iter().filter(|line| !line.trim().is_empty());
    let code_lines = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if emitted.clone().count() != code_lines {
        // A Renk `:` block adds the line of its `}`, so the block takes the emitted lines.
        let lead = &lines[0][..lines[0].len() - lines[0].trim_start().len()];
        let first = match head {
            "" => lines[0].to_string(),
            _ => format!("{lead}{}", emitter.lines[0].trim_start()),
        };
        let body = &emitter.lines[usize::from(!head.is_empty())..];
        // The emitter leaves out the blank lines around the block's lines, which stay.
        let blank = |line: &&&str| line.trim().is_empty();
        // After a head, as `if c:` in `{ if c:`, the emitted lines hold them already.
        let leading = match head {
            "" => lines[1..].iter().take_while(blank).count(),
            _ => 0,
        };
        let trailing = lines[1 + leading..].iter().rev().take_while(blank).count();
        return std::iter::once(first)
            .chain(lines[1..1 + leading].iter().map(|line| line.to_string()))
            .chain(body.iter().cloned())
            .chain(
                lines[lines.len() - trailing..]
                    .iter()
                    .map(|line| line.to_string()),
            )
            .collect::<Vec<_>>()
            .join("\n");
    }
    lines
        .iter()
        .map(|line| match line.trim() {
            "" => line.to_string(),
            code => {
                let start = line.len() - line.trim_start().len();
                let end = start + code.len();
                let lowered = emitted.next().map_or(code, |lowered| lowered.trim());
                format!("{}{lowered}{}", &line[..start], &line[end..])
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `text` with each argument of every std expression macro outside literals and comments replaced
/// by `lower(start, argument)`, where `start` is the argument's index in `text`, keeping the separating `,` (or the `;` of `vec![x; n]`), whitespace, and
/// comments around it.
pub(crate) fn map_std_macro_arguments(text: &str, lower: &dyn Fn(usize, &str) -> String) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            output.extend(&chars[index..=end]);
            index = end + 1;
            continue;
        }
        if !is_ident_start(chars[index]) || (index > 0 && is_ident_continue(chars[index - 1])) {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        let name_end = read_ident(&chars, index);
        let name = chars[index..name_end].iter().collect::<String>();
        let bang = (name_end..chars.len()).find(|&at| !chars[at].is_whitespace());
        let invocation = macro_invocation_end(&chars, index).filter(|_| {
            bang.is_some_and(|bang| chars[bang] == '!')
                && is_std_expression_macro(&name)
                && is_std_macro_path(chars[..index].iter().rev().copied())
        });
        let Some(end) = invocation else {
            output.extend(&chars[index..name_end]);
            index = name_end;
            continue;
        };
        let open = (name_end..end)
            .find(|&at| matches!(chars[at], '(' | '[' | '{'))
            .unwrap_or(end);
        output.extend(&chars[index..=open]);
        let mut start = open + 1;
        let mut depth = 0usize;
        let mut cursor = start;
        while cursor < end {
            if let Some(span_end) = opaque_span_end(&chars, cursor) {
                cursor = span_end + 1;
                continue;
            }
            if chars[cursor..].starts_with(&[':', ':', '<'])
                && let Some(close) = matching_bracket(&chars, cursor + 2, '<', '>')
            {
                cursor = close + 1;
                continue;
            }
            // The commas between closure parameters, as in `|a: u8, b: u8|`, split nothing.
            if chars[cursor] == '|'
                && is_closure_start(&chars, cursor)
                && let Some(close) = top_level_position(&chars, cursor + 1, |at| chars[at] == '|')
            {
                cursor = close + 1;
                continue;
            }
            match chars[cursor] {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                ',' | ';' if depth == 0 => {
                    output.push_str(&map_code(&chars[start..cursor], &|code| lower(start, code)));
                    output.push(chars[cursor]);
                    start = cursor + 1;
                }
                _ => {}
            }
            cursor += 1;
        }
        output.push_str(&map_code(&chars[start..end], &|code| lower(start, code)));
        output.push(chars[end]);
        index = end + 1;
    }
    output
}

/// `lower` applied to the code of `chars`, keeping the whitespace and comments around it.
fn map_code(chars: &[char], lower: &dyn Fn(&str) -> String) -> String {
    let mut first = None;
    let mut last = 0usize;
    let mut index = 0usize;
    while index < chars.len() {
        let is_comment = chars[index] == '/' && matches!(chars.get(index + 1), Some('/' | '*'));
        let end = opaque_span_end(chars, index).unwrap_or(index);
        if !is_comment && !chars[index].is_whitespace() {
            first.get_or_insert(index);
            last = end + 1;
        }
        index = end + 1;
    }
    let Some(first) = first else {
        return chars.iter().collect();
    };
    let code = chars[first..last].iter().collect::<String>();
    chars[..first].iter().collect::<String>()
        + &lower(&code)
        + &chars[last..].iter().collect::<String>()
}

/// Replaces the arguments of every std format macro invocation outside literals and comments with
/// `rewrite(arguments, prefix)`, leaving invocations it declines unchanged.
pub(crate) fn rewrite_format_macros(
    text: &str,
    rewrite: fn(&str, usize) -> Option<String>,
) -> String {
    let mut output = String::with_capacity(text.len());
    let mut index = 0usize;
    while index < text.len() {
        let Some(character) = text[index..].chars().next() else {
            break;
        };
        let rest = &text[index..];
        let skipped = if let Some(end) = raw_literal_end(text, index) {
            end - index
        } else if rest.starts_with("//") {
            rest.find('\n').unwrap_or(rest.len())
        } else if rest.starts_with("/*") {
            rest.find("*/").map_or(rest.len(), |end| end + 2)
        } else if character == '"' {
            quoted_string_end(text, index) + 1 - index
        } else if character == '\'' {
            // The `'` of a lifetime, as in `&'a str`, opens no literal.
            let chars = rest.chars().take(12).collect::<Vec<_>>();
            char_literal_end(&chars, 0)
                .map_or(1, |end| chars[..=end].iter().map(|c| c.len_utf8()).sum())
        } else {
            0
        };
        if skipped > 0 {
            output.push_str(&rest[..skipped]);
            index += skipped;
            continue;
        }
        if is_ident_start(character) {
            let name_end = index + character.len_utf8();
            let name_end = read_ident_at(text, name_end);
            let name = &text[index..name_end];
            let mut cursor = name_end;
            skip_whitespace(text, &mut cursor);
            if let Some(prefix) = format_macro_prefix(name)
                && is_std_macro_path(text[..index].chars().rev())
                && text[cursor..].starts_with('!')
            {
                cursor += 1;
                skip_whitespace(text, &mut cursor);
                if text[cursor..].starts_with('(')
                    && let Some(close) = matching_delimiter(text, cursor, '(', ')', true)
                    && let Some(invocation) = rewrite(&text[cursor + 1..close], prefix)
                {
                    output.push_str(name);
                    output.push_str(&text[name_end..cursor + 1]);
                    output.push_str(&invocation);
                    output.push(')');
                    index = close + 1;
                    continue;
                }
            }
            output.push_str(name);
            index = name_end;
            continue;
        }
        output.push(character);
        index += character.len_utf8();
    }
    output
}

fn lower_format_invocation(arguments: &str, prefix: usize) -> Option<String> {
    let arguments = split_macro_arguments(arguments, true);
    let format_literal = arguments.get(prefix)?.trim();
    let (content_start, content_end) = format_literal_bounds(format_literal)?;
    let content = &format_literal[content_start..content_end];
    // A placeholder in a literal that is not raw writes its strings' quotes as `\"`.
    let escaped = !format_literal[..content_start].contains('r');
    let (lowered, expressions) = lower_format_string(content, false, escaped)?;
    if expressions.is_empty() {
        return None;
    }
    // Without explicit arguments, captures such as `{pos}` become arguments too, so
    // `"{QuotedChar(*ch)} {pos}"` keeps the argument order of `"{} {}", QuotedChar(*ch), pos`.
    let (content, expressions) = if arguments.len() == prefix + 1 {
        lower_format_string(content, true, escaped)?
    } else {
        (lowered, expressions)
    };

    let mut lowered_arguments = arguments[..prefix]
        .iter()
        .map(|argument| argument.trim().to_string())
        .collect::<Vec<_>>();
    lowered_arguments.push(format!(
        "{}{}{}",
        &format_literal[..content_start],
        content,
        &format_literal[content_end..]
    ));
    lowered_arguments.extend(
        arguments
            .iter()
            .skip(prefix + 1)
            .map(|argument| argument.trim().to_string()),
    );
    let explicit_count = lowered_arguments.len() - prefix - 1;
    for (index, expression) in expressions.into_iter().enumerate() {
        let placeholder = if explicit_count == 0 {
            "".to_string()
        } else {
            format!("{}", explicit_count + index)
        };
        let marker = format!("__renk_inline_{}__", index);
        lowered_arguments[prefix] = lowered_arguments[prefix].replace(&marker, &placeholder);
        lowered_arguments.push(lower_expr(&expression));
    }
    Some(lowered_arguments.join(", "))
}

/// Lowers `{expr}` placeholders to positional markers; plain identifiers other than `self` stay as
/// Rust's captured or named arguments unless `captures` lowers them too.
pub(crate) fn lower_format_string(
    content: &str,
    captures: bool,
    escaped: bool,
) -> Option<(String, Vec<String>)> {
    let mut lowered = String::with_capacity(content.len());
    let mut expressions = Vec::new();
    let mut index = 0usize;
    while index < content.len() {
        let character = content[index..].chars().next()?;
        if character == '{' {
            if content[index + 1..].starts_with('{') {
                lowered.push_str("{{");
                index += 2;
                continue;
            }
            let (close, body) = read_format_placeholder(content, index + 1, escaped)?;
            let body = body.trim();
            let expression = split_format_specifier(body).0;
            if is_standard_format_placeholder(body)
                || (!captures
                    && expression != "self"
                    && expression.starts_with(is_ident_start)
                    && expression.chars().all(is_ident_continue))
            {
                lowered.push('{');
                lowered.push_str(body);
                lowered.push('}');
            } else {
                let (expression, specifier) = split_format_specifier(body);
                let expression_index = expressions.len();
                expressions.push(expression.to_string());
                lowered.push('{');
                lowered.push_str(&format!("__renk_inline_{}__", expression_index));
                if let Some(specifier) = specifier {
                    lowered.push(':');
                    lowered.push_str(specifier);
                }
                lowered.push('}');
            }
            index = close + 1;
        } else if character == '}' && content[index + 1..].starts_with('}') {
            lowered.push_str("}}");
            index += 2;
        } else {
            lowered.push(character);
            index += character.len_utf8();
        }
    }
    Some((lowered, expressions))
}

/// The name of a `name = value` format argument.
fn named_format_argument(argument: &str) -> Option<&str> {
    let (name, value) = argument.split_once('=')?;
    let name = name.trim();
    (!value.starts_with('=')
        && name.starts_with(is_ident_start)
        && name.chars().all(is_ident_continue))
    .then_some(name)
}

/// `"{} {:?}", a, b.c` as `"{a} {b.c:?}"` when each argument fills one placeholder, in order, so
/// the inline form evaluates the same expressions in the same order. Arguments that would be hard
/// to read or to split back out of a placeholder (literals, blocks, closures, `:`) stay positional;
/// strings inside an argument, as in `f("a")`, keep their plain quotes.
pub(crate) fn inline_format_arguments(arguments: &str, prefix: usize) -> Option<String> {
    let arguments = split_macro_arguments(arguments, true);
    let literal = arguments.get(prefix)?.trim();
    let (start, end) = format_literal_bounds(literal)?;
    // Renk reads `NAME.len()` inside a placeholder as a path, like any other expression.
    let values = arguments[prefix + 1..]
        .iter()
        .map(|argument| parenthesize_index_calls(&parenthesize_value_receivers(argument.trim())))
        .collect::<Vec<_>>();
    if values.is_empty()
        || values.iter().any(|value| {
            value.is_empty()
                || opaque_span_end(&value.chars().collect::<Vec<_>>(), 0).is_some()
                || value.contains(['{', '}', '|', '$', '\n'])
                || value.replace("::", "").contains(':')
                || named_format_argument(value).is_some()
        })
    {
        return None;
    }
    let content = &literal[start..end];
    let (mut inlined, mut used, mut implicit, mut explicit) = (String::new(), 0, false, false);
    let mut captured = false;
    let mut index = 0;
    while index < content.len() {
        let character = content[index..].chars().next()?;
        if matches!(character, '{' | '}') && content[index + 1..].starts_with(character) {
            inlined.push_str(&content[index..index + 2]);
            index += 2;
            continue;
        }
        if character != '{' {
            inlined.push(character);
            index += character.len_utf8();
            continue;
        }
        let close = index + 1 + content[index + 1..].find('}')?;
        let body = &content[index + 1..close];
        let (position, specifier) = body
            .split_once(':')
            .map_or((body, None), |(p, s)| (p, Some(s)));
        if specifier.is_some_and(|specifier| specifier.contains('$')) {
            return None;
        }
        let position = position.trim();
        if position.is_empty() {
            implicit = true;
        } else if position.chars().all(|c| c.is_ascii_digit()) && position.parse() == Ok(used) {
            explicit = true;
        } else if !position.starts_with(|c: char| c.is_alphabetic() || c == '_') {
            return None;
        } else {
            // Rust evaluates captures after the arguments, so lowering `{pos}` back keeps their
            // order only when no argument follows it.
            captured = true;
            inlined.push_str(&content[index..=close]);
            index = close + 1;
            continue;
        }
        if captured {
            return None;
        }
        inlined.push('{');
        inlined.push_str(values.get(used)?);
        if let Some(specifier) = specifier {
            inlined.push(':');
            inlined.push_str(specifier);
        }
        inlined.push('}');
        used += 1;
        index = close + 1;
    }
    if used != values.len() || (implicit && explicit) {
        return None;
    }
    let mut output = arguments[..prefix]
        .iter()
        .map(|argument| argument.trim().to_string())
        .collect::<Vec<_>>();
    output.push(format!("{}{inlined}{}", &literal[..start], &literal[end..]));
    Some(output.join(", "))
}

/// Generated Rust with format arguments inlined, so `"{}", a.b` and `"{a.b}"` compare equal; line
/// breaks inside an inlined invocation are kept so lines still align with the source map. An
/// argument with a quote, which Renk always lowers back to a positional one, stays positional so
/// the text still lexes as Rust.
pub(crate) fn canonical_format_macros(rust: &str) -> String {
    rewrite_format_macros(rust, |arguments, prefix| {
        let values = split_macro_arguments(arguments, true);
        if values
            .iter()
            .skip(prefix + 1)
            .any(|value| value.contains(['"', '\'']))
        {
            return None;
        }
        let inlined = inline_format_arguments(arguments, prefix)?;
        Some(inlined + &"\n".repeat(arguments.matches('\n').count()))
    })
}

fn is_standard_format_placeholder(body: &str) -> bool {
    body.is_empty()
        || body.starts_with(':')
        || body.split_once(':').is_some_and(|(position, _)| {
            !position.trim().is_empty() && position.trim().chars().all(|c| c.is_ascii_digit())
        })
        || body.chars().all(|c| c.is_ascii_digit())
}

fn split_format_specifier(body: &str) -> (&str, Option<&str>) {
    let mut round = 0usize;
    let mut square = 0usize;
    let mut curly = 0usize;
    let mut index = 0usize;
    while index < body.len() {
        let character = body[index..].chars().next().unwrap();
        match character {
            '(' => round += 1,
            ')' => round = round.saturating_sub(1),
            '[' => square += 1,
            ']' => square = square.saturating_sub(1),
            '{' => curly += 1,
            '}' => curly = curly.saturating_sub(1),
            '"' | '\'' => index = quoted_string_end(body, index),
            ':' if round == 0
                && square == 0
                && curly == 0
                && !body[index + 1..].starts_with(':')
                && (index == 0 || !body[..index].ends_with(':')) =>
            {
                return (body[..index].trim(), Some(body[index + 1..].trim()));
            }
            _ => {}
        }
        index += character.len_utf8();
    }
    (body.trim(), None)
}

/// The `}` closing the placeholder opened before `start`.
fn find_format_placeholder_end(text: &str, start: usize, escaped: bool) -> Option<usize> {
    read_format_placeholder(text, start, escaped).map(|(end, _)| end)
}

/// The `}` closing the placeholder opened before `start`, and the placeholder's code. With
/// `escaped`, the literal is not raw, so a string in the placeholder may be written with its
/// escapes, as `\"-\"` in `"{v.join(\"-\")}"`, besides plain quotes, as in `"{v.join("-")}"`.
fn read_format_placeholder(text: &str, start: usize, escaped: bool) -> Option<(usize, String)> {
    #[derive(PartialEq)]
    enum Mode {
        Code,
        Plain,
        Escaped,
    }
    let mut mode = Mode::Code;
    let mut depth = 1usize;
    let mut body = String::new();
    let mut index = start;
    while index < text.len() {
        let character = text[index..].chars().next()?;
        let next = text[index + character.len_utf8()..].chars().next();
        let escape = escaped && character == '\\';
        match mode {
            Mode::Code if escape && next == Some('"') => {
                body.push('"');
                mode = Mode::Escaped;
                index += 2;
                continue;
            }
            Mode::Code if let Some(end) = raw_literal_end(text, index) => {
                body.push_str(&text[index..end]);
                index = end;
                continue;
            }
            Mode::Code => match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((index, body));
                    }
                }
                '"' => mode = Mode::Plain,
                // A char literal, such as `'}'`; a lifetime such as `'static` is code.
                '\'' => {
                    let rest = text[index..].chars().collect::<Vec<_>>();
                    if let Some(end) = char_literal_end(&rest, 0) {
                        let end = index + rest[..=end].iter().map(|c| c.len_utf8()).sum::<usize>();
                        body.push_str(&text[index..end]);
                        index = end;
                        continue;
                    }
                }
                _ => {}
            },
            Mode::Plain if character == '\\' => {
                body.push(character);
                body.extend(next);
                index += 1 + next.map_or(0, char::len_utf8);
                continue;
            }
            Mode::Plain if character == '"' => mode = Mode::Code,
            Mode::Escaped if escape && next == Some('"') => {
                body.push('"');
                mode = Mode::Code;
                index += 2;
                continue;
            }
            // `\\` is the escape of the string's own escape, as in `\"\\n\"` for `"\n"`, and
            // takes the unit after it.
            Mode::Escaped if escape && next == Some('\\') => {
                body.push('\\');
                index += 2;
                let unit = text[index..].chars().next()?;
                match text[index + unit.len_utf8()..].chars().next() {
                    Some(quoted @ ('"' | '\\')) if unit == '\\' => {
                        body.push(quoted);
                        index += 2;
                    }
                    _ => {
                        body.push(unit);
                        index += unit.len_utf8();
                    }
                }
                continue;
            }
            Mode::Plain | Mode::Escaped => {}
        }
        body.push(character);
        index += character.len_utf8();
    }
    None
}
fn format_literal_bounds(text: &str) -> Option<(usize, usize)> {
    let quote = text.find('"')?;
    if text[..quote]
        .chars()
        .any(|character| !matches!(character, 'b' | 'c' | 'r' | '#'))
    {
        return None;
    }
    // A raw literal ends at its first `"` with as many `#`s as it opened with.
    let (end, closing) = if text[..quote].contains('r') {
        let closing = format!("\"{}", "#".repeat(text[..quote].matches('#').count()));
        (text[quote + 1..].find(&closing)? + quote + 1, closing.len())
    } else {
        (format_string_end(text, quote)?, 1)
    };
    if !text[end + closing..].trim().is_empty() {
        return None;
    }
    Some((quote + 1, end))
}

pub(crate) fn format_string_end(text: &str, start: usize) -> Option<usize> {
    let mut escaped = false;
    let mut index = start + 1;
    while index < text.len() {
        let character = text[index..].chars().next()?;
        if escaped {
            escaped = false;
        } else {
            match character {
                '\\' => escaped = true,
                '{' if text[index + 1..].starts_with('{') => index += 1,
                '{' => index = find_format_placeholder_end(text, index + 1, true)?,
                '"' => return Some(index),
                '}' if text[index + 1..].starts_with('}') => index += 1,
                _ => {}
            }
        }
        index += character.len_utf8();
    }
    None
}

pub(crate) fn split_macro_arguments(text: &str, format_macro: bool) -> Vec<&str> {
    let mut arguments = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut round = 0usize;
    let mut square = 0usize;
    let mut curly = 0usize;
    while index < text.len() {
        let character = text[index..].chars().next().unwrap();
        if let Some(end) = raw_literal_end(text, index) {
            index = end;
            continue;
        }
        match character {
            '(' => round += 1,
            ')' => round = round.saturating_sub(1),
            '[' => square += 1,
            ']' => square = square.saturating_sub(1),
            '{' => curly += 1,
            '}' => curly = curly.saturating_sub(1),
            '"' if format_macro => index = format_string_end(text, index).unwrap_or(index),
            '"' => index = quoted_string_end(text, index),
            '\'' => {
                // A lifetime such as `'a` is not a char literal.
                let rest = text[index..].chars().collect::<Vec<_>>();
                if let Some(end) = char_literal_end(&rest, 0) {
                    index += rest[..end].iter().map(|c| c.len_utf8()).sum::<usize>();
                }
            }
            ',' if round == 0 && square == 0 && curly == 0 => {
                arguments.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        index += character.len_utf8();
    }
    if !text[start..].trim().is_empty() {
        arguments.push(&text[start..]);
    }
    arguments
}
