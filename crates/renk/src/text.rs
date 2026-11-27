//! Text scanners for both directions: literals, comments, delimiters, identifiers, indentation.

use super::*;

/// `text` split before a `//` comment that ends it, keeping the spaces before the comment with
/// the comment; a line that is only a comment stays code.
pub(crate) fn split_trailing_comment(text: &str) -> (&str, &str) {
    let code_len = without_trailing_line_comment(text).trim_end().len();
    text.split_at(if code_len == 0 { text.len() } else { code_len })
}

/// The byte length of `text` without the spaces and `//` or `/* */` comments that end it.
pub(crate) fn code_len(text: &str) -> usize {
    let chars = text.chars().collect::<Vec<_>>();
    let (mut end, mut index) = (0, 0);
    while index < chars.len() {
        if let Some(span) = opaque_span_end(&chars, index) {
            if !(chars[index] == '/' && matches!(chars.get(index + 1), Some('/' | '*'))) {
                end = span + 1;
            }
            index = span + 1;
            continue;
        }
        if !chars[index].is_whitespace() {
            end = index + 1;
        }
        index += 1;
    }
    text.char_indices()
        .nth(end)
        .map_or(text.len(), |(at, _)| at)
}

/// `text` without a `//` comment that runs to its end.
pub(crate) fn without_trailing_line_comment(text: &str) -> &str {
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            if chars[index..].starts_with(&['/', '/']) && end + 1 == chars.len() {
                let at = text
                    .char_indices()
                    .nth(index)
                    .map_or(text.len(), |(at, _)| at);
                return &text[..at];
            }
            index = end + 1;
            continue;
        }
        index += 1;
    }
    text
}

/// For each line of `source`, whether it ends inside a string literal or block comment, whose
/// trailing whitespace and delimiters are contents rather than code.
pub(crate) fn lines_ending_in_literal(source: &str) -> Vec<bool> {
    let mut statement = String::new();
    source
        .lines()
        .map(|line| {
            statement.push_str(line);
            let open = open_delimiters(&statement).is_none();
            if open {
                statement.push('\n');
            } else {
                statement.clear();
            }
            open
        })
        .collect()
}

pub(crate) fn raw_indent(raw: &str) -> usize {
    raw.chars()
        .take_while(|character| *character == ' ')
        .count()
}

pub(crate) fn has_top_level_arrow(text: &str) -> bool {
    let chars = text.chars().collect::<Vec<_>>();
    top_level_position(&chars, 0, |at| chars[at..].starts_with(&['=', '>'])).is_some()
}

/// `text` split at its first top-level `=>`, so a macro arm pattern such as `($a:expr => $b:expr)`
/// keeps its inner arrow.
pub(crate) fn split_arrow(text: &str) -> Option<(&str, &str)> {
    let chars = text.chars().collect::<Vec<_>>();
    let at = top_level_position(&chars, 0, |at| chars[at..].starts_with(&['=', '>']));
    match at {
        Some(at) => {
            let at = text
                .char_indices()
                .nth(at)
                .map_or(text.len(), |(byte, _)| byte);
            Some((&text[..at], &text[at + 2..]))
        }
        None => text.split_once("=>"),
    }
}

pub(crate) fn split_once_top_level(text: &str, needle: char) -> Option<(&str, &str)> {
    let mut round = 0usize;
    let mut square = 0usize;
    let mut angle = 0usize;
    let mut previous = '\0';
    for (index, character) in text.char_indices() {
        match character {
            '(' => round += 1,
            ')' => round = round.saturating_sub(1),
            '[' => square += 1,
            ']' => square = square.saturating_sub(1),
            '<' if previous == ':' => angle += 1,
            '>' if angle > 0 => angle -= 1,
            character if character == needle && round == 0 && square == 0 && angle == 0 => {
                return Some((&text[..index], &text[index + character.len_utf8()..]));
            }
            _ => {}
        }
        previous = character;
    }
    None
}

pub(crate) fn is_single_colon(chars: &[char], at: usize) -> bool {
    chars[at] == ':' && chars.get(at + 1) != Some(&':') && (at == 0 || chars[at - 1] != ':')
}

pub(crate) fn is_closure_start(chars: &[char], bar: usize) -> bool {
    let end = chars[..bar]
        .iter()
        .rposition(|character| !character.is_whitespace())
        .map_or(0, |at| at + 1);
    let before = &chars[..end];
    before.is_empty()
        || matches!(before.last(), Some('(' | '[' | '{' | ',' | ';' | '=' | ':'))
        || before.ends_with(&['=', '>'])
        || before.last() == Some(&'&')
        || ["move", "return", "mut", "async", "then", "else"]
            .iter()
            .any(|keyword| {
                end.checked_sub(keyword.len())
                    .is_some_and(|at| keyword_at(before, at, keyword))
            })
}

pub(crate) fn skip_whitespace_chars(chars: &[char], index: &mut usize) {
    while chars
        .get(*index)
        .is_some_and(|character| character.is_whitespace())
    {
        *index += 1;
    }
}

/// First index from `start` at bracket depth zero that satisfies `stop`, skipping literals,
/// comments, and macro bodies; stops at an unmatched closing bracket.
pub(crate) fn top_level_position(
    chars: &[char],
    start: usize,
    stop: impl Fn(usize) -> bool,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = start;
    while index < chars.len() {
        if let Some(end) =
            opaque_span_end(chars, index).or_else(|| macro_invocation_end(chars, index))
        {
            index = end + 1;
            continue;
        }
        if depth == 0 && stop(index) {
            return Some(index);
        }
        match chars[index] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return None,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    None
}

pub(crate) fn matching_delimiter(
    text: &str,
    open: usize,
    opening: char,
    closing: char,
    format_macro: bool,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open;
    while index < text.len() {
        let character = text[index..].chars().next()?;
        if let Some(end) = raw_literal_end(text, index) {
            index = end;
            continue;
        }
        match character {
            '"' if format_macro => index = format_string_end(text, index)?,
            '"' | '\'' => index = quoted_string_end(text, index),
            character if character == opening => depth += 1,
            character if character == closing => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += character.len_utf8();
    }
    None
}

pub(crate) fn quoted_string_end(text: &str, start: usize) -> usize {
    let quote = text[start..].chars().next().unwrap();
    let mut escaped = false;
    let mut index = start + quote.len_utf8();
    while index < text.len() {
        let character = text[index..].chars().next().unwrap();
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == quote {
            return index;
        }
        index += character.len_utf8();
    }
    text.len().saturating_sub(1)
}

pub(crate) fn read_ident_at(text: &str, mut index: usize) -> usize {
    while index < text.len() {
        let character = text[index..].chars().next().unwrap();
        if !is_ident_continue(character) {
            break;
        }
        index += character.len_utf8();
    }
    index
}

pub(crate) fn skip_whitespace(text: &str, index: &mut usize) {
    while *index < text.len() {
        let character = text[*index..].chars().next().unwrap();
        if !character.is_whitespace() {
            break;
        }
        *index += character.len_utf8();
    }
}

pub(crate) fn find_top_level_keyword(text: &str, keyword: &str) -> Option<usize> {
    let chars = text.chars().collect::<Vec<_>>();
    let byte_offsets = text
        .char_indices()
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    let mut round = 0usize;
    let mut square = 0usize;
    let mut curly = 0usize;
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

        if round == 0
            && square == 0
            && curly == 0
            && keyword_at(&chars, index, keyword)
            && !chars[..index].iter().collect::<String>().trim().is_empty()
        {
            return byte_offsets.get(index).copied();
        }

        index += 1;
    }

    None
}

pub(crate) fn keyword_at(chars: &[char], index: usize, keyword: &str) -> bool {
    let keyword = keyword.chars().collect::<Vec<_>>();
    let end = index + keyword.len();
    chars.get(index..end) == Some(keyword.as_slice())
        && index
            .checked_sub(1)
            .and_then(|before| chars.get(before))
            .is_none_or(|character| !is_ident_continue(*character))
        && chars
            .get(end)
            .is_none_or(|character| !is_ident_continue(*character))
}

pub(crate) fn matching_bracket(
    chars: &[char],
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open_index;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(chars, index) {
            index = end + 1;
            continue;
        }
        // The `>` of `->` and `=>` never closes a generic argument list.
        let arrow = close == '>' && index > 0 && matches!(chars[index - 1], '-' | '=');
        if chars[index] == open {
            depth += 1;
        } else if chars[index] == close && !arrow {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
        index += 1;
    }
    None
}

pub(crate) fn opaque_span_end(chars: &[char], index: usize) -> Option<usize> {
    if index >= chars.len() {
        return None;
    }
    if chars[index] == '/' && chars.get(index + 1) == Some(&'/') {
        let newline = chars[index..]
            .iter()
            .position(|character| *character == '\n');
        return Some(newline.map_or(chars.len(), |offset| index + offset) - 1);
    }
    if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
        let mut depth = 1usize;
        let mut cursor = index + 2;
        while cursor + 1 < chars.len() {
            if chars[cursor] == '/' && chars[cursor + 1] == '*' {
                depth += 1;
                cursor += 2;
                continue;
            }
            if chars[cursor] == '*' && chars[cursor + 1] == '/' {
                depth -= 1;
                if depth == 0 {
                    return Some(cursor + 1);
                }
                cursor += 2;
                continue;
            }
            cursor += 1;
        }
        return Some(chars.len().saturating_sub(1));
    }
    if let Some(end) = raw_string_span_end(chars, index) {
        return Some(end);
    }
    if chars[index] == '"' {
        return Some(quoted_chars_end(chars, index, '"'));
    }
    if matches!(chars[index], 'b' | 'c') && chars.get(index + 1) == Some(&'"') {
        return Some(quoted_chars_end(chars, index + 1, '"'));
    }
    if chars[index] == '\'' {
        return char_literal_end(chars, index);
    }
    if chars[index] == 'b' && chars.get(index + 1) == Some(&'\'') {
        return char_literal_end(chars, index + 1);
    }
    None
}

/// The byte index just past the raw string literal starting at byte `index`, as after the `#` of
/// `r#"a"#`, whose `"` ends no ordinary string.
pub(crate) fn raw_literal_end(text: &str, index: usize) -> Option<usize> {
    let rest = &text[index..];
    let bytes = rest.as_bytes();
    let prefix = usize::from(matches!(bytes.first(), Some(b'b' | b'c')));
    if bytes.get(prefix) != Some(&b'r')
        || text[..index]
            .chars()
            .next_back()
            .is_some_and(is_ident_continue)
    {
        return None;
    }
    let hashes = bytes[prefix + 1..]
        .iter()
        .take_while(|&&b| b == b'#')
        .count();
    let quote = prefix + 1 + hashes;
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let close = format!("\"{}", "#".repeat(hashes));
    let end = rest[quote + 1..].find(&close);
    Some(index + end.map_or(rest.len(), |end| quote + 1 + end + close.len()))
}

pub(crate) fn raw_string_span_end(chars: &[char], index: usize) -> Option<usize> {
    let mut raw = index;
    if chars
        .get(raw)
        .is_some_and(|character| matches!(*character, 'b' | 'c'))
    {
        raw += 1;
    }
    if chars.get(raw) != Some(&'r') {
        return None;
    }
    let mut quote = raw + 1;
    let mut hashes = 0usize;
    while chars.get(quote) == Some(&'#') {
        hashes += 1;
        quote += 1;
    }
    if chars.get(quote) != Some(&'"') {
        return None;
    }
    let mut cursor = quote + 1;
    while cursor < chars.len() {
        if chars[cursor] == '"'
            && (0..hashes).all(|offset| chars.get(cursor + 1 + offset) == Some(&'#'))
        {
            return Some(cursor + hashes);
        }
        cursor += 1;
    }
    Some(chars.len().saturating_sub(1))
}

fn quoted_chars_end(chars: &[char], start: usize, quote: char) -> usize {
    let mut escaped = false;
    let mut cursor = start + 1;
    while cursor < chars.len() {
        let character = chars[cursor];
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == quote {
            return cursor;
        }
        cursor += 1;
    }
    chars.len().saturating_sub(1)
}

pub(crate) fn char_literal_end(chars: &[char], quote: usize) -> Option<usize> {
    let first = *chars.get(quote + 1)?;
    if is_ident_continue(first) && chars.get(quote + 2) != Some(&'\'') {
        return None;
    }
    if first != '\\' {
        return (chars.get(quote + 2) == Some(&'\'')).then_some(quote + 2);
    }
    let mut cursor = quote + 2;
    while cursor < chars.len() {
        if chars[cursor] == '\'' {
            return Some(cursor);
        }
        if chars[cursor].is_whitespace() {
            return None;
        }
        cursor += 1;
    }
    None
}

pub(crate) fn macro_invocation_end(chars: &[char], start: usize) -> Option<usize> {
    if !chars.get(start).copied().is_some_and(is_ident_start) {
        return None;
    }
    let mut cursor = read_ident(chars, start);
    loop {
        if cursor + 2 < chars.len()
            && chars[cursor] == ':'
            && chars[cursor + 1] == ':'
            && is_ident_start(chars[cursor + 2])
        {
            cursor = read_ident(chars, cursor + 2);
        } else {
            break;
        }
    }
    while chars
        .get(cursor)
        .is_some_and(|character| character.is_whitespace())
    {
        cursor += 1;
    }
    if chars.get(cursor) != Some(&'!') {
        return None;
    }
    cursor += 1;
    while chars
        .get(cursor)
        .is_some_and(|character| character.is_whitespace())
    {
        cursor += 1;
    }
    let open = *chars.get(cursor)?;
    let close = match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None,
    };
    matching_bracket(chars, cursor, open, close)
}

/// The macro invocations of `chars`, as `h!(...)`, outside literals and comments.
pub(crate) fn macro_spans(chars: &[char]) -> Vec<std::ops::Range<usize>> {
    let mut spans = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(chars, index) {
            index = end + 1;
        } else if let Some(end) = macro_invocation_end(chars, index) {
            spans.push(index..end + 1);
            index = end + 1;
        } else {
            index += 1;
        }
    }
    spans
}

pub(crate) fn has_ident_before(chars: &[char], index: usize) -> bool {
    index > 0
        && (is_ident_continue(chars[index - 1])
            || chars[index - 1] == ']'
            || chars[index - 1] == '>')
}

/// The statements `chars` separates with top-level `;`, as `let a = 1` and ` a` in
/// `let a = 1; a`; a `;` in a literal, group, or macro body separates none.
pub(crate) fn top_level_statements(chars: &[char]) -> Vec<std::ops::Range<usize>> {
    let mut statements = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = top_level_position(chars, start, |at| chars[at] == ';').unwrap_or(chars.len());
        statements.push(start..end);
        start = end + 1;
    }
    statements
}

pub(crate) fn is_ident_start(character: char) -> bool {
    character == '_' || unicode_ident::is_xid_start(character)
}

pub(crate) fn is_ident_continue(character: char) -> bool {
    unicode_ident::is_xid_continue(character)
}

pub(crate) fn read_ident(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() && is_ident_continue(chars[index]) {
        index += 1;
    }
    index
}

pub(crate) fn spaces(count: usize) -> String {
    " ".repeat(count)
}

pub(crate) fn ensure_trailing_newline(text: &str) -> String {
    if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{}\n", text)
    }
}
