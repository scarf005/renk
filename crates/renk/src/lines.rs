//! Renk source to logical lines: tabs, joined continuation lines, comments and open delimiters.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceLine {
    pub(crate) line: usize,
    pub(crate) indent: usize,
    /// Source line and byte offset `text` starts at.
    pub(crate) start: (usize, usize),
    pub(crate) text: String,
    /// A line of a multi-line `macro_rules!` arm body, kept as Rust tokens.
    pub(crate) raw: bool,
    /// Trailing `//` comment with its leading spaces, moved after the generated line.
    pub(crate) comment: Option<String>,
    /// Comment of the `}` line closing this line's block, as in `} // end`.
    pub(crate) closing_comment: Option<String>,
    /// Whether a closing line without a tail, such as `})` or `)`, ended this line's block.
    pub(crate) closed: bool,
}

fn starts_control_flow(header: &str) -> bool {
    ["if ", "while ", "for ", "match ", "else", "loop", "unsafe"]
        .iter()
        .any(|keyword| header.starts_with(keyword))
}

/// Lines outside multi-line literals, with their 0-based index, whose indentation has a tab.
fn indent_tab_lines(source: &str) -> impl Iterator<Item = usize> {
    let in_literal = lines_ending_in_literal(source);
    source.lines().enumerate().filter_map(move |(index, raw)| {
        let continues_literal = index > 0 && in_literal[index - 1];
        let indent = &raw[..raw.len() - raw.trim_start().len()];
        (!continues_literal && indent.contains('\t')).then_some(index)
    })
}

/// Renk reads structure from indentation, so a tab there is ambiguous.
fn reject_indent_tabs(source: &str) -> Result<(), TranspileError> {
    let Some(index) = indent_tab_lines(source).next() else {
        return Ok(());
    };
    let line = source.lines().nth(index).unwrap_or_default();
    Err(TranspileError {
        line: index + 1,
        column: line.chars().take_while(|&c| c != '\t').count() + 1,
        message: "tabs are not supported; use spaces".to_string(),
    })
}

/// Moves an arm starting on its `macro_rules! m {(...) => ({` line, and the `})}` closing both,
/// onto their own lines, so the arm sits below the item like any other.
pub(crate) fn split_inline_macro_arms(source: &str) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let in_literal = lines_ending_in_literal(source);
    let mut output = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let code = without_trailing_line_comment(line).trim_end();
        let open = (!(index > 0 && in_literal[index - 1]) && is_macro_item(code.trim_start()))
            .then(|| code.find('{'))
            .flatten()
            .filter(|&open| !code[open + 1..].trim().is_empty());
        let mut delimiters = OpenDelimiters::default();
        let mut text = String::new();
        let end = open.and_then(|_| {
            (index..lines.len()).find(|&at| {
                text.push_str(lines[at]);
                text.push('\n');
                delimiters.update(&text).is_some_and(<[char]>::is_empty)
            })
        });
        let closes = |end: usize| lines[end][..code_len(lines[end])].ends_with('}');
        let (Some(open), Some(end)) = (open, end.filter(|&end| end > index && closes(end))) else {
            output.push(line.to_string());
            index += 1;
            continue;
        };
        let indent = spaces(raw_indent(line));
        output.push(line[..=open].to_string());
        output.push(format!("{indent}    {}", line[open + 1..].trim_start()));
        for at in index + 1..end {
            let keep = in_literal[at - 1] || lines[at].trim().is_empty();
            output.push(if keep {
                lines[at].to_string()
            } else {
                format!("    {}", lines[at])
            });
        }
        let close = &lines[end][..code_len(lines[end])];
        let head = &close[..close.len() - 1];
        if !head.trim().is_empty() {
            output.push(format!("    {head}"));
        }
        output.push(format!("{indent}}}{}", &lines[end][close.len()..]));
        index = end + 1;
    }
    let mut converted = output.join("\n");
    if source.ends_with('\n') {
        converted.push('\n');
    }
    converted
}

/// Rust `source` with tabs in indentation expanded to 4-column tab stops.
pub(crate) fn expand_indent_tabs(source: &str) -> String {
    let tabbed = indent_tab_lines(source).collect::<Vec<_>>();
    if tabbed.is_empty() {
        return source.to_string();
    }
    source
        .split('\n')
        .enumerate()
        .map(|(index, line)| {
            if tabbed.binary_search(&index).is_err() {
                return line.to_string();
            }
            let code = line.trim_start();
            let width = line[..line.len() - code.len()].chars().fold(0, |width, c| {
                if c == '\t' {
                    width / 4 * 4 + 4
                } else {
                    width + 1
                }
            });
            format!("{}{code}", spaces(width))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn source_lines(source: &str) -> Result<Vec<SourceLine>, TranspileError> {
    reject_indent_tabs(source)?;
    let mut lines = Vec::new();
    let mut raw_lines = source.lines().enumerate().peekable();
    // Indents of the enclosing `macro_rules!` item and of the multi-line arm being read.
    let mut macro_indent = None;
    let mut arm_indent = None;
    let in_literal = lines_ending_in_literal(source);
    let physical = source.lines().collect::<Vec<_>>();
    let heads = with_pipe_patterns(
        colon_header_heads(&physical, &in_literal),
        &physical,
        &in_literal,
    );
    // The first line with code at or after each line, so a run of comment lines looks ahead once.
    let mut next_code = vec![None; physical.len() + 1];
    for (index, line) in physical.iter().enumerate().rev() {
        next_code[index] = if code_len(line.trim_start()) > 0 {
            Some(*line)
        } else {
            next_code[index + 1]
        };
    }
    // The closing line split off a one-line `else`, as `)` from `else { 2 })`.
    let mut carry = None;
    while let Some((index, raw)) = carry.take().or_else(|| {
        raw_lines
            .next()
            .map(|(index, raw)| (index, raw.to_string()))
    }) {
        let raw = raw.as_str();
        let line = index + 1;
        let without_comment = keep_literal_end(raw, in_literal[index]);
        let trimmed = without_comment.trim();
        let indent = raw_indent(without_comment);
        if let Some(arm) = arm_indent.filter(|&arm| !trimmed.is_empty() && indent > arm) {
            // A macro arm body is Rust tokens laid out as written, so its lines are siblings that
            // keep their indentation past the arm's.
            lines.push(SourceLine {
                line,
                indent: arm + 1,
                start: (line, arm + 1),
                text: without_comment[arm + 1..].trim_end().to_string(),
                raw: true,
                comment: None,
                closing_comment: None,
                closed: false,
            });
            continue;
        }
        // Comment lines at a closing line's indent, as in `    *x` / `// c` / `)`, belong inside the
        // block it ends.
        if trimmed.starts_with([')', ']', ',', '}']) {
            let comments = lines
                .iter()
                .rev()
                .take_while(|line| line.indent == indent && !line.raw && is_comment(&line.text))
                .count();
            let start = lines.len() - comments;
            let body = lines[..start]
                .iter()
                .rposition(|owner| owner.indent <= indent)
                .filter(|&owner| lines[owner].indent == indent)
                .and_then(|owner| lines.get(owner + 1))
                .map(|first| first.indent)
                .filter(|&body| body > indent);
            if let Some(body) = body {
                lines[start..]
                    .iter_mut()
                    .for_each(|comment| comment.indent = body);
            }
        }
        // A line closing the group a `:` block's head left open, as `)` after `xs.map(|x|:`, ends
        // the block as a `})` line ends a `{` block.
        let closes_group_block = trimmed.starts_with([')', ']', ','])
            && lines
                .iter()
                .rev()
                .find(|owner: &&SourceLine| owner.indent <= indent)
                .is_some_and(|owner| {
                    owner.indent == indent
                        && (has_block_marker(&owner.text) || is_branch(owner.text.trim()))
                        && opens_group_block(&lines, owner)
                });
        let braced;
        let (without_comment, trimmed) = if closes_group_block {
            let leading = without_comment.len() - without_comment.trim_start().len();
            braced = format!("{}}}{trimmed}", &without_comment[..leading]);
            (braced.as_str(), braced.trim())
        } else {
            (without_comment, trimmed)
        };
        // The emitter closes a `.method(|x|:` chain line itself, before its sibling `.method()`.
        let chained = closes_before_chain(trimmed, raw_lines.peek().map(|(_, next)| *next), indent)
            && !lines
                .iter()
                .rev()
                .find(|opener: &&SourceLine| opener.indent <= indent)
                .is_some_and(|opener| opener.indent == indent && opener.text.starts_with('.'));
        let code = &trimmed[..code_len(trimmed)];
        if trimmed.is_empty()
            || (is_closing_delimiter_line(code) && closing_tail(code).is_none() && !chained)
        {
            if let Some(owner) = lines
                .iter()
                .rposition(|owner| owner.indent <= indent)
                // A value on the line after `=`, as in `let a =` / `    f(|| {`, closes at its
                // own indent.
                .filter(|&owner| {
                    let value = lines[owner].text.lines().last().unwrap_or_default();
                    (lines[owner].indent == indent
                        || (lines[owner].text.contains('\n') && raw_indent(value) == indent))
                        && !trimmed.is_empty()
                })
            {
                // The line closes what its block's head left open, as `})` after `f(|x| {`.
                let head = &lines[owner];
                if has_block_marker(&head.text)
                    || head.text.ends_with('{')
                    || is_branch(head.text.trim())
                {
                    let expected =
                        block_chain_head(&lines, head).map_or_else(String::new, |head| {
                            format!(
                                "}}{}",
                                closing_delimiters(strip_block_marker(head.text.trim()).0)
                            )
                        });
                    if code.trim_end_matches([';', ',']).replace(' ', "") != expected {
                        return Err(TranspileError {
                            line,
                            column: first_column(without_comment),
                            message: format!(
                                "the line closing this block closes what its head left open, `{}`",
                                // A `:` head closing a group leaves out the `}`, as in `)`.
                                &expected[usize::from(
                                    !head.text.ends_with('{') && expected.len() > 1
                                )..]
                            ),
                        });
                    }
                }
                let owner = &mut lines[owner];
                owner.closed = true;
                // The comment of a closing line, as in `} // end`, follows the generated `}`.
                if code.len() < trimmed.len() {
                    owner.closing_comment = Some(format!(" {}", trimmed[code.len()..].trim()));
                }
            }
            continue;
        }
        arm_indent = None;
        if macro_indent.is_some_and(|item| indent <= item) {
            macro_indent = None;
        }
        let mut text = without_comment.trim_start().to_string();
        // Leading whitespace may hold more than spaces, as an IME's `\u{3000}`.
        let mut start = (line, without_comment.len() - text.len());
        // `} else {` continues a `{` chain whose closing braces the emitter writes itself.
        if let Some(branch) = text
            .strip_prefix('}')
            .map(str::trim_start)
            .filter(|branch| is_branch(branch))
        {
            // The `}` written before a closing line such as `, |y|:` is not in the source.
            start.1 += text.len() - branch.len() - usize::from(closes_group_block);
            text = branch.to_string();
        }
        // A `})` line before `.method()` lines takes them as its closing tail.
        let mut group = chained.then_some('}');
        let mut open = OpenDelimiters::default();
        let mut header_end = heads[index];
        while joins_next_line(
            &text,
            raw_lines.peek().map(|(_, next)| *next),
            raw_indent(without_comment),
            &mut group,
            &mut open,
        ) || header_end.is_some()
            // A macro arm's matcher continued over several lines, up to its `=>`, with the
            // delimiters `joins_next_line` left open.
            || (macro_indent.is_some_and(|item| indent > item) && !open.open.is_empty())
            || continues_line(
                raw_lines.peek().and_then(|&(index, _)| next_code[index]),
                raw_indent(without_comment),
            )
        {
            let Some((index, next)) = raw_lines.next() else {
                let (line, column) = first_unclosed(&physical[line - 1..].join("\n"))
                    .map_or((line, first_column(without_comment)), |(offset, column)| {
                        (line + offset, column)
                    });
                return Err(TranspileError {
                    line,
                    column,
                    message: "unclosed delimiter, literal, or `=` continuation at end of file"
                        .to_string(),
                });
            };
            text.push('\n');
            text.push_str(keep_literal_end(next, in_literal[index]));
            header_end = header_end.filter(|&end| end > index).or(heads[index]);
        }
        if is_macro_item(&text) {
            macro_indent = Some(indent);
        } else if macro_indent.is_some()
            && split_arrow(strip_block_marker(&text).0)
                .is_some_and(|(_, body)| body.trim().is_empty())
        {
            arm_indent = Some(indent);
        }
        let (code, comment) = split_trailing_comment(&text);
        // Attributes before an item on its line, as in `#[inline] pub fn f():`, are lines of
        // their own that the emitter writes back on the item's line.
        let mut code = code;
        while code.starts_with("#[")
            && let Some(close) = matching_delimiter(code, 1, '[', ']', false)
            && !code[close + 1..].trim().is_empty()
        {
            lines.push(SourceLine {
                line,
                indent,
                start,
                text: code[..=close].to_string(),
                raw: false,
                comment: None,
                closing_comment: None,
                closed: false,
            });
            let rest = &code[close + 1..];
            start = text_start(
                start,
                code,
                close + 1 + rest.len() - rest.trim_start().len(),
            );
            code = rest.trim_start();
        }
        lines.push(SourceLine {
            line,
            indent,
            start,
            text: code.to_string(),
            raw: false,
            comment: (!comment.is_empty()).then(|| comment.to_string()),
            closing_comment: None,
            closed: false,
        });
        // A one-line `else` that closes what its chain's head opened, as in `else { 2 })`, ends
        // the chain with the closing line after its branch; the `,` of an arm, as in
        // `else { 2 },`, is the arm's own.
        if is_else_branch(code) && !has_explicit_block_opener(code) {
            let chars = code.chars().collect::<Vec<_>>();
            let end = argument_end(&chars, 0);
            let at = code
                .char_indices()
                .nth(end)
                .map_or(code.len(), |(at, _)| at);
            if matches!(chars.get(end), Some(')' | ']' | ','))
                && opens_group_block(&lines, &lines[lines.len() - 1])
            {
                let branch = lines.last_mut().expect("the `else` line was just pushed");
                branch.text = code[..at].trim_end().to_string();
                branch.comment = None;
                carry = Some((index, format!("{}{}{comment}", spaces(indent), &code[at..])));
            } else if code[at..].trim() == "," {
                let branch = lines.last_mut().expect("the `else` line was just pushed");
                branch.text = code[..at].trim_end().to_string();
            }
        }
    }
    Ok(lines)
}

/// Whether the logical line `text` continues on the `next` physical line: inside a literal or
/// block comment, after a trailing `=` or binary operator, or in a group opened by a line ending
/// with `(`/`[` (or a struct literal or `use` tree `{`). A group lasts until its outer delimiter
/// closes, so blocks inside it stay Rust syntax, and then extends over following `.method` / `?`
/// lines at the same `indent`; any other line ending with `{` or `:` owns an indented block.
fn joins_next_line(
    text: &str,
    next: Option<&str>,
    indent: usize,
    group: &mut Option<char>,
    delimiters: &mut OpenDelimiters,
) -> bool {
    // The line that decides is the last one with code, so a blank or comment line after `=` waits
    // too.
    let code = without_trailing_line_comment(delimiters.last_code_line(text));
    if delimiters.update(text).is_none() {
        return true;
    }
    let open = &delimiters.open;
    if *group == Some('<') {
        // Generic parameters over several lines, closed by a `>` line at the head's indent.
        if !code.trim_start().starts_with('>') {
            return next.is_some_and(|next| {
                (!next.trim().is_empty() && raw_indent(next) > indent)
                    || (raw_indent(next) == indent && next.trim_start().starts_with('>'))
            });
        }
        *group = None;
    }
    if let Some(outer) = *group {
        // Once the group's outer delimiter closes, a trailing `{` or `:` opens an indented block,
        // while a `(` or `[` the closing line opens continues the group.
        return match open.first() {
            Some(first) => {
                *first == outer || (matches!(first, '(' | '[') && !code.trim_end().ends_with(':'))
            }
            None => next.is_some_and(|next| {
                continues_chain(next, indent)
                    || continues_where_clause(text, next, indent, delimiters)
            }),
        };
    }
    if code.trim_end().ends_with(['(', '['])
        || opens_struct_literal(code)
        || (open.last() == Some(&'{') && (is_use_or_extern_crate(code) || starts_raw_item(code)))
        // A `,`, closure bars or `||` inside brackets, as in `.map(|v|` or `(a ||`.
        || (code.trim_end().ends_with([',', '|']) && matches!(open.first(), Some('(' | '[')))
        // A block inside brackets, as in `data: [u64; if Self::COMPRESS {`, or an operand block,
        // as in `+ if c {` or `if (a || {`, rather than a closure body or an element value, as
        // in `[match a {`, whose lines are Renk.
        || (code.trim_end().ends_with('{')
            && ((open.contains(&'[')
                && !is_trailing_match(strip_open_brace(code))
                && !is_trailing_if(strip_open_brace(code)))
                || opens_operand_block(code, open.contains(&'('))))
        // A macro invocation's delimiter left open, as in `quote_spanned! {span=>` or
        // `Some(quote! {`, whose token tree continues on the next lines.
        || delimiters.macro_open.contains(&true)
    {
        *group = open.first().copied();
        return true;
    }
    if code.trim_end().ends_with('<')
        && next.is_some_and(|next| !next.trim().is_empty() && raw_indent(next) > indent)
    {
        *group = Some('<');
        return true;
    }
    let trimmed = code.trim_end();
    open.is_empty()
        && ((trimmed.ends_with('=')
            && !["==", "!=", "<=", ">="]
                .iter()
                .any(|operator| trimmed.ends_with(operator)))
            || ends_with_binary_operator(trimmed)
            || (trimmed.ends_with(',') && delimiters.in_closure_parameters(text))
            || next.is_some_and(|next| continues_where_clause(text, next, indent, delimiters)))
}

/// Whether the `{` ending `code` opens a block operand: on a line continuing an expression with
/// a binary operator, as in `+ if c {`, or after `||` / `&&` inside parentheses.
fn opens_operand_block(code: &str, in_parentheses: bool) -> bool {
    let line = code.trim();
    let head = line[..line.len() - 1].trim_end();
    let chars = head.chars().collect::<Vec<_>>();
    let after_operator = (head.ends_with("||") && !is_closure_start(&chars, chars.len() - 2))
        || head.ends_with("&&");
    // `&& items.iter().any(|item| match item {` opens a closure body inside its call instead.
    let opens_directly = open_delimiters(line).is_some_and(|open| open == ['{']);
    (starts_with_binary_operator(line) && opens_directly) || (in_parentheses && after_operator)
}

/// Whether `next`, the next line with code after any blank or comment lines, continues the line
/// at `indent`: with a binary operator at that indent, as `+ b` or `== b`, or on a more indented
/// line with a match arm's guard, as `if a => b`, or a signature's return type, as `-> u8;`.
/// `|| f()` is a closure and `> Type` closes generic parameters instead.
fn continues_line(next: Option<&str>, indent: usize) -> bool {
    next.is_some_and(|line| {
        let code = line.trim_start();
        (raw_indent(line) == indent
            && starts_with_binary_operator(code)
            && !code.starts_with("|| ")
            && !code.starts_with("> "))
            || (raw_indent(line) > indent
                && (code.starts_with("-> ")
                    || (code.starts_with("if ")
                        && find_top_level_keyword(&code[..code_len(code)], "=>").is_some())))
    })
}

const BINARY_OPERATORS: [&str; 17] = [
    "+ ", "- ", "* ", "/ ", "% ", "&& ", "|| ", "& ", "^ ", "<< ", ">> ", "== ", "!= ", "<= ",
    ">= ", "< ", "> ",
];

/// Whether `line` continues an expression with a binary operator, as in `+ if a {` or `&& b`.
pub(crate) fn starts_with_binary_operator(line: &str) -> bool {
    BINARY_OPERATORS
        .iter()
        .any(|operator| line.starts_with(operator))
}

/// Whether `code` ends with a spaced binary operator that the next line continues, as `a +` or
/// the `"_" |` of an or-pattern. The bars of a closure such as `move ||` do not count, nor `<`
/// and `>`, which may end a tag.
fn ends_with_binary_operator(code: &str) -> bool {
    let chars = code.chars().collect::<Vec<_>>();
    BINARY_OPERATORS[..15]
        .iter()
        .chain(&["| "])
        .any(|operator| {
            let operator = operator.trim_end();
            code.strip_suffix(operator)
                .is_some_and(|head| head.ends_with(' '))
                && !(operator.ends_with('|')
                    && is_closure_start(&chars, chars.len() - operator.len()))
        })
}

/// For each line heading a multi-line header, such as a long `if` condition, the index of the line
/// holding only `opener` (`{` in Rust, `:` in Renk) at the head's indent after its more indented
/// continuation lines, as rustfmt lays them out.
/// `heads` extended so a pattern such as `A` continued with `| B` lines at its indent, with
/// comments between them, reads as one line.
fn with_pipe_patterns(
    mut heads: Vec<Option<usize>>,
    lines: &[&str],
    in_literal: &[bool],
) -> Vec<Option<usize>> {
    let continued = |index: usize| {
        index + 1 < lines.len()
            && !in_literal[index]
            && raw_indent(lines[index + 1]) == raw_indent(lines[index])
    };
    // The last `| B` line in the run of blank, comment, and `| B` lines at one indent starting
    // at each line, found in one backward pass. A `| B => b` line ends its run.
    let mut run_ends = vec![None; lines.len()];
    for index in (0..lines.len()).rev() {
        let code = lines[index].trim();
        let pipe = code.starts_with("| ");
        if pipe || code.is_empty() || is_comment(code) {
            run_ends[index] = (continued(index) && !(pipe && has_top_level_arrow(code)))
                .then(|| run_ends[index + 1])
                .flatten()
                .or(pipe.then_some(index));
        }
    }
    for start in 0..lines.len() {
        if lines[start].trim().is_empty() || (start > 0 && in_literal[start - 1]) {
            continue;
        }
        // After a whole arm, as `a => 1`, a run ending in `| B => b` is an arm with a leading `|`.
        let end = continued(start)
            .then(|| run_ends[start + 1])
            .flatten()
            .filter(|&end| {
                !(has_top_level_arrow(lines[end].trim())
                    && has_top_level_arrow(lines[start].trim()))
            });
        heads[start] = heads[start].max(end);
    }
    heads
}

/// For each line heading a Renk header continued on more indented lines, such as a long `if`
/// condition or a `where` clause, the index of the line ending with the `:` that opens its body.
fn colon_header_heads(lines: &[&str], in_literal: &[bool]) -> Vec<Option<usize>> {
    let code = |line: &str| line.trim()[..code_len(line.trim())].trim_end().to_string();
    let mut heads = vec![None; lines.len()];
    for head in 0..lines.len() {
        let head_code = code(lines[head]);
        // A match arm whose guard starts on the next line, as in `Some(x)` / `    if x > 1 =>:`.
        let guarded_arm = lines[head + 1..]
            .iter()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|next| {
                raw_indent(next) > raw_indent(lines[head]) && next.trim_start().starts_with("if ")
            });
        // A `{` inside a group opens an operand block rather than the body, as in `if (a || {`.
        let operand_block = head_code.ends_with('{')
            && open_delimiters(&head_code).is_some_and(|open| {
                open[..open.len().saturating_sub(1)]
                    .iter()
                    .any(|d| matches!(d, '(' | '['))
            });
        if (head > 0 && in_literal[head - 1])
            || (head_code.ends_with([':', '{', '}', ';', ',']) && !operand_block)
            || !(guarded_arm || is_multi_line_header_head(&head_code))
        {
            continue;
        }
        let indent = raw_indent(lines[head]);
        for end in head + 1..lines.len() {
            let end_code = code(lines[end]);
            if in_literal[end - 1] || end_code.is_empty() {
                continue;
            }
            // Continuation lines are more indented, except a group closed at the head's indent,
            // as in `) -> u8:` or `}) && b:`, an or-pattern's `| B`, and an item's `where`.
            let continues = raw_indent(lines[end]) > indent
                || (raw_indent(lines[end]) == indent
                    && (end_code.starts_with([')', ']', '}'])
                        || end_code.starts_with("| ")
                        || (!starts_control_flow(&head_code)
                            && (end_code == "where" || end_code.starts_with("where ")))));
            // A closure head ends an expression's line, as in `= xs` / `.map(|x|:`.
            if !continues || ends_with_closure_block(&end_code) {
                break;
            }
            // A bound continued on a deeper line, as in `Self::Item:` / `    Clone + Default:`,
            // does not end the header. A closer at the head's indent, as in `) -> u8:`, opens the body.
            let continued = !(raw_indent(lines[end]) == indent
                && end_code.starts_with([')', ']', '}']))
                && awaits_bounds(strip_block_marker(&end_code).0)
                && (end + 1..lines.len())
                    .find(|&next| !in_literal[next - 1] && !code(lines[next]).is_empty())
                    .is_some_and(|next| raw_indent(lines[next]) > raw_indent(lines[end]));
            // A guard ends at its `=>:`, unlike a value continued after `y =`.
            let arm_ends = !guarded_arm
                || is_multi_line_header_head(&head_code)
                || find_top_level_keyword(&end_code, "=>").is_some();
            if has_block_marker(&end_code) && !end_code.ends_with("::") && !continued && arm_ends {
                heads[head] = Some(end);
                break;
            }
        }
    }
    block_condition_heads(heads, lines, in_literal, ":")
}

/// Whether the last predicate of `text` names a type without its bounds, as `Self::Item` or
/// `where T`, unlike `where T: Copy`.
fn awaits_bounds(text: &str) -> bool {
    let chars = text.chars().collect::<Vec<_>>();
    let (mut start, mut predicate) = (0, 0);
    loop {
        let end = argument_end(&chars, start);
        if chars.get(end) != Some(&',') {
            break;
        }
        start = end + 1;
        // A `,` inside `<...>`, as in `Into<Result<u8, ()>>`, separates generic arguments.
        if angle_depth(&chars[predicate..end]) == 0 {
            predicate = start;
        }
    }
    let last = chars[predicate..]
        .iter()
        .collect::<String>()
        .replace("::", "");
    !last.contains(':')
}

/// The depth of the `<` generics `chars` leaves open, past the `>` of `->` and `=>` and the
/// shifts or comparisons inside groups and literals, as in `[u8; 1 << 2]` or `S<'<'>`.
fn angle_depth(chars: &[char]) -> isize {
    let (mut depth, mut groups, mut index) = (0, 0usize, 0);
    while index < chars.len() {
        if let Some(end) = opaque_span_end(chars, index) {
            index = end + 1;
            continue;
        }
        match chars[index] {
            '(' | '[' | '{' => groups += 1,
            ')' | ']' | '}' => groups = groups.saturating_sub(1),
            '<' if groups == 0 => depth += 1,
            '>' if groups == 0 && (index == 0 || !matches!(chars[index - 1], '-' | '=')) => {
                depth -= 1
            }
            _ => {}
        }
        index += 1;
    }
    depth
}

/// `text` split at the `=` before a function's expression body, as in `fn f() -> T = value`; a
/// `=` inside the signature's generics, as in `-> impl Iterator<Item = u8>`, or inside a comment
/// stays in it.
pub(crate) fn split_function_body(text: &str) -> Option<(&str, &str)> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            index = end + 1;
            continue;
        }
        let assigns = chars[index] == '='
            && !matches!(chars.get(index + 1), Some('=' | '>'))
            && (index == 0 || !matches!(chars[index - 1], '=' | '!' | '<' | '>'));
        if assigns {
            let offset = chars[..index].iter().map(|c| c.len_utf8()).sum::<usize>();
            if open_delimiters(&text[..offset]).is_some_and(|open| open.is_empty())
                && angle_depth(&chars[..index]) == 0
            {
                return Some((&text[..offset], &text[offset + 1..]));
            }
        }
        index += 1;
    }
    None
}

/// Whether `node` is a header whose `:` ends a more indented continuation line, which opens its
/// `{` on a line of its own, as in `if a` / `    && b:`.
pub(crate) fn opens_on_own_line(node: &Node) -> bool {
    let first = node.text.lines().next().unwrap_or_default().trim();
    let guarded_arm =
        node.text
            .lines()
            .nth(1)
            .is_some_and(|guard| guard.trim_start().starts_with("if "))
            && node.text.lines().last().is_some_and(|last| {
                find_top_level_keyword(&last[..code_len(last)], "=>").is_some()
            });
    let last = node.text.lines().last().unwrap_or_default();
    // The value after a pattern over several lines, as in `if let S {` / `    a,` / `} = s:` or
    // `if let Some(` / `    a,` / `) = s:`.
    let pattern_value = ["if let ", "while let "]
        .iter()
        .any(|keyword| first.contains(keyword))
        && last.trim_start().starts_with(['}', ')', ']'])
        && last
            .trim_start()
            .trim_start_matches(['}', ')', ']'])
            .starts_with(" = ");
    node.text.lines().count() > 1
        && (guarded_arm || pattern_value || is_multi_line_header_head(&first[..code_len(first)]))
        // A `where` line at the head's indent, as in `) -> u8` / `where T: Copy:`, too.
        && (raw_indent(last) > node.indent || has_where_line(last) || pattern_value)
        && has_block_marker(last)
}

/// Whether `code` can head a header continued on more indented lines: a control expression,
/// a `let` or assignment whose value is one, or an item header without an expression body.
pub(crate) fn is_multi_line_header_head(code: &str) -> bool {
    // `if c then a else b` and `fn f() -> T = value` are expressions, not headers.
    (starts_control_flow(code)
        || has_control_operand(code)
        || looks_like_item_header_without_body(code))
        && find_top_level_keyword(code, "then").is_none()
        && !(is_function(code) && split_function_body(code).is_some())
}

/// `heads` where a condition or scrutinee holding a block, as in `if match x {` ... `} {`,
/// `if a || {` ... `} {`, or `match f(|w| match w {` ... `}) {`, ends at its `}` line that
/// opens the body with `opener`.
fn block_condition_heads(
    mut heads: Vec<Option<usize>>,
    lines: &[&str],
    in_literal: &[bool],
    opener: &str,
) -> Vec<Option<usize>> {
    let code = |line: &str| line.trim()[..code_len(line.trim())].to_string();
    for (head, line) in lines.iter().enumerate() {
        let head_code = code(line);
        // The value of a `let` or `return`, as in `let v = if match x {`.
        let expression = head_code
            .strip_prefix("let ")
            .and_then(|binding| binding.split_once(" = "))
            .map(|(_, value)| value)
            .or_else(|| head_code.strip_prefix("return "))
            .filter(|value| value.starts_with("if ") || value.starts_with("match "))
            .unwrap_or(&head_code);
        let condition = ["if ", "while ", "else if ", "match ", "for "]
            .iter()
            .find_map(|keyword| expression.strip_prefix(keyword));
        // A block value, as in `while let Some(x) = {`, `for x in {`, or `match unsafe {`.
        let block_value = head_code.strip_suffix('{').is_some_and(|before| {
            let before = before.trim_end();
            before.ends_with('=')
                || ["in", "unsafe"].iter().any(|keyword| {
                    before
                        .strip_suffix(keyword)
                        .is_some_and(|rest| rest.ends_with(' '))
                })
        });
        // A pattern continued over lines, as in `if let Flags::Coff {` / `    c, ..` / `} = s.flags:`.
        let pattern = condition.is_some_and(|condition| {
            condition.starts_with("let ") && !condition.contains(" = ") && head_code.ends_with('{')
        });
        let holds_block = pattern
            || condition.is_some_and(|condition| {
                // `if let Some(x) = if let ... {` or `while let Some(x) = match y {`.
                let value = condition
                    .strip_prefix("let ")
                    .and_then(|binding| binding.split_once(" = "))
                    .map(|(_, value)| value);
                block_value
                    || (condition.starts_with("match ") && !expression.starts_with("match "))
                    || value.is_some_and(|value| {
                        value.starts_with("if ") || value.starts_with("match ")
                    })
                    || (head_code.ends_with('{') && opens_operand_block(&head_code, true))
                    || open_delimiters(condition)
                        .is_some_and(|open| open.contains(&'(') || open.contains(&'['))
            });
        if (head > 0 && in_literal[head - 1]) || !holds_block || !head_code.ends_with('{') {
            continue;
        }
        let indent = raw_indent(line);
        // The header ends before a statement at its indent, as in `}) && b:` / `while c {`.
        heads[head] = (head + 1..lines.len())
            .take_while(|&end| {
                lines[end].trim().is_empty()
                    || raw_indent(lines[end]) > indent
                    || (raw_indent(lines[end]) == indent
                        && lines[end].trim_start().starts_with(['}', ')', ']']))
            })
            .find(|&end| {
                let end_code = code(lines[end]);
                // `} {`, `}) {`, `}) {}`, `} {};`, `}:`, or `} || b:`, unlike `} && match y {`.
                raw_indent(lines[end]) == indent
                    && end_code
                        .strip_prefix('}')
                        .map(|rest| rest.trim_start_matches([')', ']']).trim_start())
                        .is_some_and(|rest| {
                            rest == opener
                                || matches!(rest, "{}" | "{};")
                                || (starts_with_binary_operator(rest) && rest.ends_with(opener))
                                || (pattern && rest.starts_with("= ") && rest.ends_with(opener))
                        })
            })
            .or(heads[head]);
    }
    heads
}

/// Whether `next` continues the multi-line `where` clause of the item header `text`: a `where`
/// line at the header's `indent`, the more indented bounds after it, and the `:` (or Rust `{`)
/// line at the header's indent that opens the body.
fn continues_where_clause(
    text: &str,
    next: &str,
    indent: usize,
    delimiters: &mut OpenDelimiters,
) -> bool {
    let code = |line: &str| without_trailing_line_comment(line).trim().to_string();
    let last = code(text.lines().last().unwrap_or_default());
    let next_code = code(next);
    let header = text.lines().next().unwrap_or_default().trim_start();
    if last.ends_with([':', '{', ';'])
        || next_code.is_empty()
        || !looks_like_item_header_without_body(header)
    {
        return false;
    }
    if !delimiters.has_where(text) {
        return raw_indent(next) == indent && starts_where(&next_code);
    }
    raw_indent(next) > indent || (raw_indent(next) == indent && matches!(&*next_code, "{" | "{}"))
}

fn starts_where(line: &str) -> bool {
    line == "where" || line.starts_with("where ")
}

/// Whether `text` holds a control expression as an operand or a let-else outside its groups, as
/// in `= if c`, `&mut match x`, `= 'a: loop`, or `= y else`, unlike `|x| if x` in a closure,
/// `"if"`, or `dyn for<'a> Fn`.
fn has_control_operand(text: &str) -> bool {
    let chars = text.chars().collect::<Vec<_>>();
    let mut depth = 0usize;
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            index = end + 1;
            continue;
        }
        match chars[index] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            character
                if depth == 0
                    && (index == 0 || !is_ident_continue(chars[index - 1]))
                    && is_ident_continue(character) =>
            {
                let word = chars[index..]
                    .iter()
                    .take_while(|c| is_ident_continue(**c))
                    .collect::<String>();
                // Skip other words before the scans below, which read to the end of `text`.
                if !matches!(
                    &*word,
                    "else"
                        | "if"
                        | "match"
                        | "while"
                        | "for"
                        | "loop"
                        | "unsafe"
                        | "async"
                        | "const"
                ) {
                    index += word.chars().count();
                    continue;
                }
                let before = chars[..index].iter().collect::<String>();
                let before = before.trim_end();
                // A label, as in `'a: loop`, sits between the operator and the loop.
                let before = before
                    .strip_suffix(':')
                    .and_then(|head| head.trim_end_matches(is_ident_continue).strip_suffix('\''))
                    .unwrap_or(before)
                    .trim_end();
                // An operand follows an operator or a keyword such as `&mut` or `return`, unlike
                // `dyn for<'a>`.
                let operand = before.chars().last().is_some_and(|last| {
                    !is_ident_continue(last) && !matches!(last, ')' | ']' | '}' | '"' | '\'')
                }) || ["mut", "const", "return", "break", "move"].iter().any(
                    |keyword| {
                        before
                            .strip_suffix(keyword)
                            .is_some_and(|head| !head.ends_with(is_ident_continue))
                    },
                );
                let end = index + word.chars().count();
                // The expression stays open to the end of `text` unless a `{` group follows, as
                // in `unsafe { g() } + 1`; a `for` loop also has its `in`.
                let (mut inner, mut cursor, mut closed, mut has_in) = (0usize, end, false, false);
                while cursor < chars.len() {
                    if let Some(span) = opaque_span_end(&chars, cursor) {
                        cursor = span + 1;
                        continue;
                    }
                    match chars[cursor] {
                        '{' => {
                            closed |= inner == 0;
                            inner += 1;
                        }
                        '(' | '[' => inner += 1,
                        ')' | ']' | '}' => inner = inner.saturating_sub(1),
                        'i' if inner == 0
                            && chars[cursor - 1] == ' '
                            && chars[cursor..].starts_with(&['i', 'n', ' ']) =>
                        {
                            has_in = true
                        }
                        _ => {}
                    }
                    cursor += 1;
                }
                let after = chars[end..].iter().collect::<String>();
                let after = after.trim_start();
                let after = after.strip_prefix("move").unwrap_or(after).trim_start();
                // `unsafe fn()`, `for<'a> fn()`, and `async || 1` are types and closures, and
                // `r#else` is a name.
                let opens = !closed
                    && match &*word {
                        "unsafe" | "async" | "const" => after.is_empty(),
                        "for" => has_in,
                        _ => true,
                    };
                let raw = before.ends_with("r#");
                if !raw
                    && opens
                    && (word == "else"
                        || matches!(
                            &*word,
                            "if" | "match"
                                | "while"
                                | "for"
                                | "loop"
                                | "unsafe"
                                | "async"
                                | "const"
                        ) && operand)
                {
                    return true;
                }
                index += word.chars().count();
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    false
}

/// A trailing `{` after a path such as `Point`, `c.sockaddr_in` or `timeval` opens a struct
/// literal, which stays Rust syntax like a `(`/`[` group; keyword blocks such as `unsafe {` and
/// item, control, and return-type headers own blocks instead.
fn opens_struct_literal(text: &str) -> bool {
    let Some(head) = text.trim_end().strip_suffix('{') else {
        return false;
    };
    let head = head.trim_end();
    // A turbofish after the type, as in `Pair::<(i32, i32)> {`, is part of the path.
    let head = strip_trailing_turbofish(head);
    let path_start = head
        .char_indices()
        .rfind(|&(_, character)| !is_ident_continue(character) && !matches!(character, ':' | '.'))
        .map_or(0, |(at, character)| at + character.len_utf8());
    let path = &head[path_start..];
    let name = path.rsplit([':', '.']).next().unwrap_or_default();
    let header = without_visibility(text.lines().next().unwrap_or_default().trim_start());
    (name.starts_with(|character: char| character.is_ascii_uppercase())
        || (path.starts_with(is_ident_start)
            && name.starts_with(is_ident_start)
            && !matches!(
                name,
                "async" | "break" | "const" | "else" | "loop" | "move" | "return" | "unsafe"
            )))
        && !head[..path_start].trim_end().ends_with("->")
        && !ends_in_condition(&head[..path_start])
        && ![
            "fn ",
            "impl",
            "struct ",
            "enum ",
            "union ",
            "trait ",
            "mod ",
            "loop",
            "else",
            "where",
            "unsafe ",
            "async ",
            "const fn ",
            "extern ",
            "macro_rules!",
        ]
        .iter()
        .any(|keyword| header.starts_with(keyword))
}

/// `head` without the `::<...>` it ends with.
fn strip_trailing_turbofish(head: &str) -> &str {
    let Some(inner) = head.strip_suffix('>') else {
        return head;
    };
    let mut depth = 1usize;
    for (at, character) in inner.char_indices().rev() {
        match character {
            '>' => depth += 1,
            '<' => {
                depth -= 1;
                if depth == 0 {
                    return inner[..at].strip_suffix("::").unwrap_or(head);
                }
            }
            _ => {}
        }
    }
    head
}

/// Whether `head` ends inside an `if`/`while`/`match`/`for ... in` condition, where Rust reads a
/// trailing `Path {` as the block rather than a struct literal.
fn ends_in_condition(head: &str) -> bool {
    let chars = head.chars().collect::<Vec<_>>();
    let mut conditions = vec![false];
    let mut index = 0usize;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            index = end + 1;
            continue;
        }
        let level = conditions.len() - 1;
        match chars[index] {
            '(' | '[' | '{' => conditions.push(false),
            ')' | ']' | '}' if level > 0 => {
                conditions.pop();
            }
            ',' | ';' => conditions[level] = false,
            '=' if chars.get(index + 1) == Some(&'>') => conditions[level] = false,
            _ if ["if", "while", "match", "in"]
                .iter()
                .any(|keyword| keyword_at(&chars, index, keyword)) =>
            {
                conditions[level] = true;
            }
            _ => {}
        }
        index += 1;
    }
    conditions.last() == Some(&true)
}

/// The 1-based column of the first character of `line` after its indentation.
fn first_column(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count() + 1
}

/// The line offset and 1-based column in `text` of the literal or comment left open, or else of
/// the outermost delimiter left open.
fn first_unclosed(text: &str) -> Option<(usize, usize)> {
    let chars = text.chars().collect::<Vec<_>>();
    let (mut open, mut index) = (Vec::new(), 0);
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            if !opaque_span_is_closed(&chars, index, end) {
                open = vec![index];
                break;
            }
            index = end + 1;
            continue;
        }
        match chars[index] {
            '(' | '[' | '{' => open.push(index),
            ')' | ']' | '}' => {
                open.pop();
            }
            _ => {}
        }
        index += 1;
    }
    let at = *open.first()?;
    let line_start = chars[..at]
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |at| at + 1);
    let offset = chars[..at].iter().filter(|&&c| c == '\n').count();
    Some((offset, at - line_start + 1))
}

pub(crate) fn keep_literal_end(line: &str, in_literal: bool) -> &str {
    if in_literal { line } else { line.trim_end() }
}

/// Unclosed delimiters at the end of `text`, or `None` while a literal or block comment is open.
pub(crate) fn open_delimiters(text: &str) -> Option<Vec<char>> {
    OpenDelimiters::default().update(text).map(<[char]>::to_vec)
}

/// The delimiters a logical line leaves open, scanned incrementally as its physical lines are
/// appended, so joining a long group stays linear.
#[derive(Default)]
struct OpenDelimiters {
    open: Vec<char>,
    /// For each open delimiter, whether it follows a macro name and `!`, as in `m!(`.
    macro_open: Vec<bool>,
    /// Bytes of the text already scanned; a literal or comment left open is rescanned.
    scanned: usize,
    /// Bytes of the text already searched for lines with code, and the last such line.
    searched: usize,
    last_code: std::ops::Range<usize>,
    /// The text's characters as far as the closure parameter scan has read them (`read` bytes),
    /// the index it resumes at, and its bracket depth while inside parameters.
    read: usize,
    chars: Vec<char>,
    parameters_at: usize,
    parameters_depth: Option<usize>,
    /// Bytes of the text already searched for a `where`, and whether a line held one.
    where_searched: usize,
    where_found: bool,
}

impl OpenDelimiters {
    /// Whether a line of `text` starts with, ends with, or holds the word `where`; `text` extends
    /// the text of the previous call.
    fn has_where(&mut self, text: &str) -> bool {
        self.where_found = self.where_found
            || text[self.where_searched..].lines().any(|line| {
                let line = without_trailing_line_comment(line).trim();
                starts_where(line) || line.ends_with(" where") || line.contains(" where ")
            });
        self.where_searched = text.len();
        self.where_found
    }

    /// The last line of `text` with code; `text` extends the text of the previous call.
    fn last_code_line<'a>(&mut self, text: &'a str) -> &'a str {
        let mut at = self.searched;
        for line in text[self.searched..].split('\n') {
            if code_len(line) > 0 {
                self.last_code = at..at + line.len();
            }
            at += line.len() + 1;
        }
        self.searched = text.len();
        &text[self.last_code.clone()]
    }

    /// Whether `text` ends inside the parameters of a closure, as `let f = |a: u8,` before
    /// `b: u8| {`; `text` extends the text of the previous call.
    fn in_closure_parameters(&mut self, text: &str) -> bool {
        self.chars.extend(text[self.read..].chars());
        self.read = text.len();
        let chars = &self.chars;
        let (mut index, mut depth) = (self.parameters_at, self.parameters_depth);
        while index < chars.len() {
            if let Some(end) = opaque_span_end(chars, index)
                .or_else(|| depth.and_then(|_| macro_invocation_end(chars, index)))
            {
                index = end + 1;
                continue;
            }
            depth = match (depth, chars[index]) {
                (None, '|') if is_closure_start(chars, index) => Some(0),
                (Some(0), '|') => None,
                (Some(inner), '(' | '[' | '{') => Some(inner + 1),
                // An unmatched closing bracket leaves the parameters unclosed for good.
                (Some(0), ')' | ']' | '}') => break,
                (Some(inner), ')' | ']' | '}') => Some(inner - 1),
                _ => depth,
            };
            index += 1;
        }
        (self.parameters_at, self.parameters_depth) = (index, depth);
        depth.is_some()
    }

    /// The delimiters `text` leaves open, or `None` inside a literal or block comment; `text`
    /// extends the text of the previous call.
    fn update(&mut self, text: &str) -> Option<&[char]> {
        let chars = text[self.scanned..].chars().collect::<Vec<_>>();
        let mut index = 0usize;
        while index < chars.len() {
            if let Some(end) = opaque_span_end(&chars, index) {
                if !opaque_span_is_closed(&chars, index, end) {
                    return None;
                }
                self.scanned += chars[index..=end]
                    .iter()
                    .map(|c| c.len_utf8())
                    .sum::<usize>();
                index = end + 1;
                continue;
            }
            match chars[index] {
                delimiter @ ('(' | '[' | '{') => {
                    let mut before = text[..self.scanned]
                        .chars()
                        .rev()
                        .skip_while(|c| c.is_whitespace());
                    self.macro_open.push(
                        before.next() == Some('!') && before.next().is_some_and(is_ident_continue),
                    );
                    self.open.push(delimiter);
                }
                ')' | ']' | '}' => {
                    self.open.pop();
                    self.macro_open.pop();
                }
                _ => {}
            }
            self.scanned += chars[index].len_utf8();
            index += 1;
        }
        Some(&self.open)
    }
}

fn opaque_span_is_closed(chars: &[char], start: usize, end: usize) -> bool {
    match (chars[start], chars.get(start + 1)) {
        ('/', Some('/')) | ('\'', _) | ('b', Some('\'')) => true,
        ('/', Some('*')) => end >= start + 3 && chars[end - 1..=end] == ['*', '/'],
        _ => {
            let quote = (start..=end).find(|&at| chars[at] == '"').unwrap_or(end);
            let hashes = chars[start..quote]
                .iter()
                .filter(|character| **character == '#')
                .count();
            let closing = end.saturating_sub(hashes);
            let escapes = chars[quote + 1..closing.max(quote + 1)]
                .iter()
                .rev()
                .take_while(|character| **character == '\\')
                .count();
            let raw = raw_string_span_end(chars, start).is_some();
            closing > quote
                && chars[closing] == '"'
                && chars[closing + 1..=end]
                    .iter()
                    .all(|character| *character == '#')
                && (raw || escapes % 2 == 0)
        }
    }
}

fn is_closing_delimiter_line(text: &str) -> bool {
    text.contains('}')
        && text
            .chars()
            .all(|character| matches!(character, '}' | ')' | ']' | ';' | ','))
}

/// Whether the block head `owner`, one of `lines`, leaves a `(` or `[` open, as `xs.map(|x|:`
/// or `f(|x| {`, or is an `else` branch of a chain whose head does, as in `max(if c:`.
pub(crate) fn opens_group_block(lines: &[SourceLine], owner: &SourceLine) -> bool {
    block_chain_head(lines, owner).is_some_and(|head| {
        open_delimiters(strip_block_marker(head.text.trim()).0)
            .is_some_and(|open| open.iter().any(|delimiter| matches!(delimiter, '(' | '[')))
    })
}

/// The head of the chain of blocks that `owner`, one of `lines`, ends: `owner` itself, or the
/// line before its branches, as `max(if c:` before `else:`.
fn block_chain_head<'a>(lines: &'a [SourceLine], owner: &'a SourceLine) -> Option<&'a SourceLine> {
    if !is_branch(owner.text.trim()) {
        return Some(owner);
    }
    lines
        .iter()
        .rev()
        .skip_while(|line| !std::ptr::eq(*line, owner))
        .take_while(|line| line.indent >= owner.indent)
        .filter(|line| line.indent == owner.indent)
        .find(|line| !is_branch(line.text.trim()))
}

pub(crate) fn format_line(raw: &str) -> String {
    if raw.trim().is_empty() {
        String::new()
    } else {
        format!("{}{}", spaces(raw_indent(raw)), raw.trim())
    }
}
