//! The value names in scope, which decide if `a.b` is a field access or a path.

use std::cell::RefCell;

use super::*;

thread_local! {
    /// Value names bound by each enclosing block, innermost last, while emitting Renk.
    pub(crate) static VALUE_SCOPES: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
    /// Names the node being emitted binds for its block only, such as `x` in `for x in xs:`.
    pub(crate) static BLOCK_BINDINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Parameters of the closures whose bodies the node being emitted opens, which its closing
    /// line ends, as `env` in `.map(|env| {` / `}) + env.args()`.
    pub(crate) static OPEN_CLOSURES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Const generics of the enclosing `impl` or `trait`, which its functions see.
    pub(crate) static ITEM_GENERICS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Free functions and statics, values everywhere, as in `parse_args.parse2(tokens)`.
    pub(crate) static MODULE_VALUES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The functions and statics of a module, which are values wherever no local shadows them.
pub(crate) fn module_value_names(nodes: &[Node]) -> Vec<String> {
    nodes
        .iter()
        .flat_map(|node| {
            let header = strip_block_marker(node.text.trim()).0;
            let keyword = if is_function(header) {
                Some("fn ")
            } else {
                ["static mut ", "static ", "const "]
                    .into_iter()
                    .find(|keyword| starts_item(header, keyword.trim_end()))
            };
            let name = keyword.and_then(|keyword| {
                let after = &header[header.find(keyword)? + keyword.len()..];
                Some(
                    after[..after
                        .find(|c: char| !is_ident_continue(c))
                        .unwrap_or(after.len())]
                        .to_string(),
                )
            });
            let nested = if is_impl_or_mod_item(header) && !starts_item(header, "mod")
                || is_trait_item(header)
            {
                Vec::new()
            } else {
                module_value_names(&node.children)
            };
            name.into_iter().chain(nested)
        })
        .collect()
}

/// The names `nodes` declare or import from other crates as modules, as in `mod config` or
/// `use std.env`, which keep paths beside a function of the same name.
pub(crate) fn module_names(nodes: &[Node]) -> Vec<String> {
    nodes
        .iter()
        .flat_map(|node| {
            let header = strip_block_marker(node.text.trim()).0;
            let path = without_visibility(header)
                .strip_prefix("use ")
                .unwrap_or_default();
            // `use super.parse_args` imports a local item, which stays what it is.
            let local = ["crate", "super", "self"].iter().any(|root| {
                path.strip_prefix(root)
                    .is_some_and(|rest| rest.starts_with(['.', ':']))
            });
            let names = if starts_item(header, "use") && !local {
                header
                    .split(|c: char| !is_ident_continue(c))
                    .map(str::to_string)
                    .collect()
            } else if starts_item(header, "mod") {
                header
                    .split_whitespace()
                    .last()
                    .map(|name| name.trim_end_matches([';', ':', '{']).to_string())
                    .into_iter()
                    .collect()
            } else {
                Vec::new()
            };
            names.into_iter().chain(module_names(&node.children))
        })
        .collect()
}

pub(crate) fn with_value_scopes<T>(convert: impl FnOnce() -> T) -> T {
    let scopes = VALUE_SCOPES.take();
    let generics = ITEM_GENERICS.take();
    let functions = MODULE_VALUES.take();
    let result = convert();
    VALUE_SCOPES.set(scopes);
    ITEM_GENERICS.set(generics);
    MODULE_VALUES.set(functions);
    result
}

pub(crate) fn is_local_value(name: &str) -> bool {
    VALUE_SCOPES.with_borrow(|scopes| scopes.iter().flatten().any(|bound| bound == name))
}

pub(crate) fn is_value_bound(name: &str) -> bool {
    is_local_value(name)
        || MODULE_VALUES.with_borrow(|functions| functions.iter().any(|function| function == name))
}

pub(crate) fn with_value_scope<T>(names: Vec<String>, emit: impl FnOnce() -> T) -> T {
    VALUE_SCOPES.with_borrow_mut(|scopes| scopes.push(names));
    let result = emit();
    VALUE_SCOPES.with_borrow_mut(|scopes| scopes.pop());
    result
}

/// Value names a node binds: in its own text and children (closure, function, match arm, and
/// `matches!` parameters, and the `let`s inside an expression, as in
/// `let n = if let Some(v) = x then v.len() else 0`), in its block only (an `if let`, `while let`,
/// or `for` pattern, whose head still sees the outer names), and for later siblings (a statement
/// `let`).
#[derive(Default)]
pub(crate) struct NodeBindings {
    pub(crate) own: Vec<String>,
    pub(crate) block: Vec<String>,
    pub(crate) siblings: Vec<String>,
}

pub(crate) fn node_bindings(text: &str, context: Context, has_block: bool) -> NodeBindings {
    let mut bindings = NodeBindings::default();
    if !matches!(
        context,
        Context::Root | Context::Block | Context::Match | Context::Trait
    ) {
        return bindings;
    }
    let (text, _) = strip_explicit_semicolon(text.trim());
    let (text, _) = strip_block_marker(text);
    let chars = text.chars().collect::<Vec<_>>();
    // A closure body continued on later lines binds its parameters there too; one closed on
    // this line binds them only inside it.
    bindings.own = closures(&chars)
        .into_iter()
        .filter(|closure| closure.body.end == chars.len())
        .flat_map(|closure| closure.parameters)
        .collect();
    bindings.own.extend(format_placeholder_bindings(&chars));
    bindings.own.extend(matches_pattern_bindings(text));
    bindings.own.extend(inline_arm_bindings(text));
    if context == Context::Match {
        if let Some((pattern, body)) = split_arrow(text) {
            let pattern =
                find_top_level_keyword(pattern, "if").map_or(pattern, |at| &pattern[..at]);
            bindings.own.extend(pattern_bindings(pattern));
            // The `let`s of a one-line body, as in `Some(s) => { let t = s; t.len() }`.
            let body = body.chars().collect::<Vec<_>>();
            bindings.own.extend(let_pattern_bindings(&body, true));
        }
        return bindings;
    }
    if is_function(text) {
        let name = text.find("fn ").map_or(0, |at| text[..at].chars().count());
        if let Some(open) = top_level_position(&chars, name, |at| chars[at] == '(')
            && let Some(close) = matching_bracket(&chars, open, '(', ')')
        {
            bindings.own.extend(const_generic_bindings(&chars[..open]));
            let parameters = chars[open + 1..close].iter().collect::<String>();
            for parameter in split_macro_arguments(&parameters, false) {
                bindings
                    .own
                    .extend(pattern_bindings(without_type(parameter)));
            }
        }
        // The `let`s of a one-line shorthand body, as in
        // `fn f(x: T) -> U = if let Some(v) = x then`; a body opening a block binds them there.
        if !has_block && let Some((_, body)) = split_function_body(text) {
            let body = body.chars().collect::<Vec<_>>();
            bindings.own.extend(let_pattern_bindings(&body, true));
        }
        return bindings;
    }
    if is_impl_or_mod_item(text) || is_trait_item(text) {
        bindings.own = const_generic_bindings(&chars);
        return bindings;
    }
    let mut lets = let_pattern_bindings(&chars, true);
    let statements = top_level_statements(&chars);
    if statements
        .iter()
        .any(|statement| is_let_statement(&chars[statement.clone()]))
    {
        // Each `let` statement of the line, as in `let a = 1; let b = 2`, binds for later lines;
        // `lower_basic_expr` binds it for the line's later statements.
        for statement in statements {
            let statement = &chars[statement];
            let mut lets = let_pattern_bindings(statement, true);
            if is_let_statement(statement) {
                let first = let_pattern_bindings(statement, false);
                bindings.siblings.extend(lets.drain(..first.len()));
            }
            bindings.own.extend(lets);
        }
        return bindings;
    }
    // The head's `for` or `let` pattern; a `let` nested in the condition, as in
    // `if let Some(v) = if let Some(x) = a {`, binds where it is written.
    let nested = lets.split_off(let_pattern_bindings(&chars, false).len().min(lets.len()));
    let mut head = lets;
    if let Some(rest) = strip_control_label(text).strip_prefix("for ")
        && let Some(at) = find_top_level_keyword(rest, "in")
    {
        head.extend(pattern_bindings(&rest[..at]));
    }
    if has_block {
        bindings.block = head;
    } else {
        bindings.own.extend(head);
    }
    bindings.own.extend(nested);
    bindings
}

pub(crate) fn is_let_statement(statement: &[char]) -> bool {
    is_let(statement.iter().collect::<String>().trim_start())
}

/// Const generic parameters a header such as `impl[const N: usize] Grid[N]` declares.
fn const_generic_bindings(chars: &[char]) -> Vec<String> {
    (0..chars.len())
        .filter(|&at| keyword_at(chars, at, "const"))
        .filter_map(|at| {
            let mut start = at + "const".len();
            skip_whitespace_chars(chars, &mut start);
            let end = read_ident(chars, start);
            let mut after = end;
            skip_whitespace_chars(chars, &mut after);
            (end > start && chars.get(after) == Some(&':'))
                .then(|| chars[start..end].iter().collect())
        })
        .collect()
}

/// Names the patterns of `matches!(value, PATTERN if guard)` bind, which its guard uses.
/// The names bound by the arms of a `match` written inside `text`, as `name` in
/// `member: match &node.ident { Some(name) => name.clone(), ... }`.
fn inline_arm_bindings(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut names = Vec::new();
    let mut index = 0;
    while index + 1 < chars.len() {
        if let Some(end) =
            opaque_span_end(&chars, index).or_else(|| macro_invocation_end(&chars, index))
        {
            index = end + 1;
            continue;
        }
        if chars[index..].starts_with(&['=', '>']) {
            // The pattern starts after the `{` or `,` that opens this arm.
            let mut depth = 0usize;
            let mut start = index;
            while start > 0 {
                match chars[start - 1] {
                    ')' | ']' | '}' => depth += 1,
                    '(' | '[' if depth > 0 => depth -= 1,
                    '{' | ',' if depth == 0 => break,
                    '{' => depth -= 1,
                    _ => {}
                }
                start -= 1;
            }
            let pattern = chars[start..index].iter().collect::<String>();
            let pattern =
                find_top_level_keyword(&pattern, "if").map_or(&*pattern, |at| &pattern[..at]);
            names.extend(pattern_bindings(pattern));
        }
        index += 1;
    }
    names
}

fn matches_pattern_bindings(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for (at, _) in text.match_indices("matches!(") {
        let chars = text[at + "matches!".len()..].chars().collect::<Vec<_>>();
        let Some(close) = matching_bracket(&chars, 0, '(', ')') else {
            continue;
        };
        let arguments = chars[1..close].iter().collect::<String>();
        if let Some(pattern) = split_macro_arguments(&arguments, false).get(1) {
            let pattern =
                find_top_level_keyword(pattern, "if").map_or(*pattern, |at| &pattern[..at]);
            names.extend(pattern_bindings(pattern));
        }
    }
    names
}

/// Names bound by the first or, with `all`, each `let PATTERN =`, as in
/// `if let Some(a) = x && let b = y`.
pub(crate) fn let_pattern_bindings(chars: &[char], all: bool) -> Vec<String> {
    let mut names = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(chars, index) {
            index = end + 1;
            continue;
        }
        if keyword_at(chars, index, "let") {
            let pattern_start = index + 3;
            let end = top_level_position(chars, pattern_start, |at| {
                chars[at] == '='
                    && !matches!(chars.get(at + 1), Some('=' | '>'))
                    && !matches!(chars[at - 1], '=' | '!' | '<' | '>')
            })
            .unwrap_or(chars.len());
            let pattern = chars[pattern_start..end].iter().collect::<String>();
            names.extend(pattern_bindings(without_type(&pattern)));
            if !all {
                break;
            }
            index = end;
        }
        index += 1;
    }
    names
}

/// Parameters of the closures inside format placeholders, as `s` in `"{xs.map(|s| s.len())}"`.
fn format_placeholder_bindings(chars: &[char]) -> Vec<String> {
    let mut names = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let Some(end) = opaque_span_end(chars, index) else {
            index += 1;
            continue;
        };
        if chars[index] == '"' {
            let content = chars[index + 1..end].iter().collect::<String>();
            for expression in
                lower_format_string(&content, true, true).map_or(Vec::new(), |(_, e)| e)
            {
                names.extend(closure_parameter_bindings(
                    &expression.chars().collect::<Vec<_>>(),
                ));
            }
        }
        index = end + 1;
    }
    names
}

/// Parameters of each closure in `chars`, macro arguments included, such as `a` and `b` in
/// `|a, (b, _): (u8, u8)|`.
fn closure_parameter_bindings(chars: &[char]) -> Vec<String> {
    closures(chars)
        .into_iter()
        .flat_map(|closure| closure.parameters)
        .collect()
}

pub(crate) struct Closure {
    pub(crate) parameters: Vec<String>,
    /// The body's character range, up to the `,`, `;`, or closing delimiter that ends it.
    pub(crate) body: std::ops::Range<usize>,
}

pub(crate) fn closures(chars: &[char]) -> Vec<Closure> {
    let mut heads = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(chars, index) {
            index = end + 1;
            continue;
        }
        if chars[index] == '|' && is_closure_start(chars, index) {
            let Some(close) = top_level_position(chars, index + 1, |at| chars[at] == '|') else {
                break;
            };
            heads.push((chars[index + 1..close].iter().collect::<String>(), close));
            index = close;
        }
        index += 1;
    }
    // Markup has no `,`, `;`, or closing bracket, so the body of `move |_| a` in each
    // `<b on:click=move |_| a></b>` of a line reaches the next body. That body starts at the same
    // depth and ends in the same place, so the scan stops there and takes its end. The memo keys
    // each end by the first index its scan tests, past a leading literal or macro such as the
    // `"a"` of `move ||"a"`, because the scan skips that span and tests no index inside it.
    let mut ends = std::collections::HashMap::new();
    let mut closures = Vec::new();
    for (parameters, close) in heads.into_iter().rev() {
        let first = std::cell::Cell::new(None);
        let end = top_level_position(chars, close + 1, |at| {
            first.set(first.get().or(Some(at)));
            matches!(chars[at], ',' | ';' | ')' | ']' | '}') || ends.contains_key(&at)
        })
        .map_or(chars.len(), |at| ends.get(&at).copied().unwrap_or(at));
        if let Some(first) = first.get() {
            ends.insert(first, end);
        }
        closures.push(Closure {
            parameters: split_macro_arguments(&parameters, false)
                .into_iter()
                .flat_map(|parameter| pattern_bindings(without_type(parameter)))
                .collect(),
            body: close + 1..end,
        });
    }
    closures.reverse();
    closures
}

/// Whether a closure whose body holds `at` binds `name`, as `env` in `.map(|env| *env)` but not
/// in the `env.args()` after it.
pub(crate) fn closure_binds_at(chars: &[char], at: usize, name: &str) -> bool {
    closures(chars).iter().any(|closure| {
        closure.body.contains(&at) && closure.parameters.iter().any(|bound| bound == name)
    })
}

/// `pattern` without a top-level `: Type` annotation.
fn without_type(pattern: &str) -> &str {
    let chars = pattern.chars().collect::<Vec<_>>();
    top_level_position(&chars, 0, |at| is_single_colon(&chars, at)).map_or(pattern, |at| {
        &pattern[..pattern.char_indices().nth(at).map_or(0, |(byte, _)| byte)]
    })
}

/// Lowercase names a pattern such as `Some((ref mut a, b @ 1..=3))` or `Point { x: px, y }`
/// binds; paths, constructors, and struct field names before `:` bind nothing.
fn pattern_bindings(pattern: &str) -> Vec<String> {
    let chars = pattern.chars().collect::<Vec<_>>();
    let mut names = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = opaque_span_end(&chars, index) {
            index = end + 1;
            continue;
        }
        let starts_name = is_ident_start(chars[index])
            && (index == 0 || !(is_ident_continue(chars[index - 1]) || chars[index - 1] == '.'));
        if !starts_name {
            index += 1;
            continue;
        }
        let end = read_ident(&chars, index);
        let name = chars[index..end].iter().collect::<String>();
        let mut after = end;
        skip_whitespace_chars(&chars, &mut after);
        let qualified = index >= 2 && chars[index - 2..index] == [':', ':'];
        if !qualified
            && !name.starts_with(char::is_uppercase)
            && !matches!(
                name.as_str(),
                "_" | "ref" | "mut" | "box" | "true" | "false" | "self"
            )
            && !matches!(chars.get(after), Some('(' | '{' | '!' | '.' | '[' | ':'))
        {
            names.push(name);
        }
        index = end;
    }
    names
}

pub(crate) fn is_camel_case(ident: &str) -> bool {
    ident.chars().next().is_some_and(char::is_uppercase) && ident.chars().any(char::is_lowercase)
}
