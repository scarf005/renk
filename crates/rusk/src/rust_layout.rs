//! What the Rust syntax tree says about the lines of a Rust source: which lines end with a `{`
//! that opens a Rusk block, and where the statement or header starting on each line ends.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

use ra_ap_parser::{LexedStr, StrStep, TopEntryPoint};
use ra_ap_syntax::{
    AstNode, Edition, NodeOrToken, SyntaxKind, SyntaxKind::*, SyntaxNode, SyntaxToken, T, TextSize,
    ast, ast::HasLoopBody, match_ast,
};

/// Lists whose `{` opens a Rusk block when it ends its line and its `}` starts a later one.
const BLOCK_LISTS: [SyntaxKind; 7] = [
    STMT_LIST,
    ITEM_LIST,
    ASSOC_ITEM_LIST,
    EXTERN_ITEM_LIST,
    RECORD_FIELD_LIST,
    VARIANT_LIST,
    MATCH_ARM_LIST,
];

pub(crate) struct RustLayout {
    /// For each line, whether it ends inside a string literal or block comment.
    pub(crate) in_literal: Vec<bool>,
    /// For each line ending with the `{` of a Rusk block, whether it opens inside a `(`, as in
    /// `f(|x| {` or `}, |y| {`.
    pub(crate) in_group: Vec<bool>,
    /// For each line ending with the `{` of an item's Rusk block, the line of its `}`.
    pub(crate) item_end: Vec<Option<usize>>,
    /// The offset each line starts at.
    starts: Vec<usize>,
    /// The tokens other than whitespace.
    tokens: Vec<SyntaxToken>,
    /// For each line, its first token and its last token other than a comment.
    first: Vec<Option<usize>>,
    last_code: Vec<Option<usize>>,
    /// For each line, whether it ends with the `{` of a Rusk block.
    opens: Vec<bool>,
    /// The offsets of the `}` closing Rusk blocks.
    closers: HashSet<TextSize>,
    /// The source file and the lists of Rusk blocks, whose children are statements or items.
    containers: HashSet<SyntaxNode>,
    /// For each token, the groups, such as `(` or a struct literal's `{`, open after it, and how
    /// many of them were opened at the end of a line, as in `f(` or `quote! {`.
    depth: Vec<(usize, usize)>,
    /// For each element's first token, how far `open_groups` scanned it and what it found.
    scans: RefCell<HashMap<usize, GroupScan>>,
}

/// The delimiters open after the scanned tokens, and whether a line break falls inside them;
/// `None` for a Rusk block. Also the line closing the last group spanning lines.
#[derive(Default)]
struct GroupScan {
    next: usize,
    open: Vec<Option<(SyntaxKind, bool)>>,
    group_end: Option<usize>,
}

impl RustLayout {
    pub(crate) fn new(source: &str) -> Self {
        let root = parse(source);
        let starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(at, _)| at + 1))
            .filter(|&start| start < source.len())
            .collect::<Vec<_>>();
        let mut layout = Self {
            in_literal: vec![false; starts.len()],
            in_group: vec![false; starts.len()],
            item_end: vec![None; starts.len()],
            tokens: root
                .descendants_with_tokens()
                .filter_map(NodeOrToken::into_token)
                .filter(|token| token.kind() != WHITESPACE)
                .collect(),
            first: vec![None; starts.len()],
            last_code: vec![None; starts.len()],
            opens: vec![false; starts.len()],
            closers: HashSet::new(),
            containers: HashSet::from([root]),
            depth: Vec::new(),
            scans: RefCell::default(),
            starts,
        };
        for (index, token) in layout.tokens.iter().enumerate() {
            let (start, end) = (layout.start_line(token), layout.end_line(token));
            layout.first[start] = layout.first[start].or(Some(index));
            layout.in_literal[start..end].fill(true);
            if token.kind() != COMMENT {
                layout.last_code[end] = Some(index);
            }
        }
        // In source order, so a block's element is known once the blocks around it are.
        for index in 0..layout.tokens.len() {
            if let Some((list, in_group)) = layout.rusk_block(index) {
                let line = layout.start_line(&layout.tokens[index]);
                layout.opens[line] = true;
                layout.in_group[line] = in_group;
                let item = layout
                    .element(&layout.tokens[index])
                    .is_some_and(|element| ast::Item::can_cast(element.kind()));
                layout.item_end[line] =
                    item.then(|| layout.start_line(&list.last_token().unwrap()));
                layout
                    .closers
                    .insert(list.last_token().unwrap().text_range().start());
                layout.containers.insert(list);
            }
        }
        layout.depth = layout.group_depths();
        layout
    }

    /// The last line of the statement or header starting on `line`.
    pub(crate) fn unit_end(&self, line: usize) -> usize {
        let Some(mut at) = self.first[line] else {
            return line;
        };
        while self.is_closer(&self.tokens[at]) && at + 1 < self.tokens.len() {
            at += 1;
        }
        let token = &self.tokens[at];
        if self.start_line(token) != line {
            return line;
        }
        // A comment line, or an attribute, ends before the code after it.
        let ends_own = |last: &SyntaxToken| {
            let end = self.end_line(last);
            let next = self.tokens[self.index_of(last) + 1..]
                .iter()
                .find(|token| token.kind() != COMMENT);
            next.is_none_or(|next| self.start_line(next) > end)
                .then_some(end)
        };
        if token.kind() == COMMENT
            && let Some(end) = ends_own(token)
        {
            return end;
        }
        if let Some(attribute) = token.parent_ancestors().find(|node| node.kind() == ATTR)
            && let Some(end) = attribute.last_token().as_ref().and_then(ends_own)
        {
            return end;
        }
        let Some(last) = self.element(token).and_then(|element| element.last_token()) else {
            return line;
        };
        let last = self.end_line(&last);
        let open_before = at
            .checked_sub(1)
            .map_or((0, 0), |before| self.depth[before]);
        // A `let`'s `else`, as in `let Some(x) = xs` / `.next()` / `else {`, starts a line; a
        // block opened before it ends the header first.
        let first_open = (line..last).find(|&end| self.opens[end]).unwrap_or(last);
        let last = (line..first_open)
            .find(|&end| {
                self.first[end + 1].is_some_and(|next| {
                    let next = &self.tokens[next];
                    next.kind() == T![else]
                        && next
                            .parent()
                            .is_some_and(|parent| parent.kind() == LET_ELSE)
                })
            })
            .unwrap_or(last);
        // A header continued on chain lines, as in `match xs` / `.count() {`, ends at its body.
        if let Some(end) = (line..=last).find(|&end| self.opens[end])
            && self.last_code[end]
                .is_some_and(|brace| brace == 0 || self.depth[brace - 1].0 <= open_before.0)
        {
            return end;
        }
        (line..last)
            .find(|&end| {
                // A `.method()` or `?` line continues a chain on a line of its own, unless the
                // line before leaves a group open after its opener or a `,`, or inside a group
                // opened at the end of a line.
                let chain = self.first[end + 1].is_some_and(|next| {
                    matches!(self.tokens[next].kind(), T![.] | T![?])
                        && self.last_code[end].is_some_and(|end| {
                            let (groups, line_groups) = self.depth[end];
                            groups <= open_before.0
                                || (line_groups <= open_before.1
                                    && !matches!(
                                        self.tokens[end].kind(),
                                        T!['('] | T!['['] | T!['{'] | T![,]
                                    ))
                        })
                });
                self.opens[end] || chain
            })
            .unwrap_or(last)
    }

    /// The list whose `{` is the token at `index` when it opens a Rusk block, and whether that
    /// block opens inside a `(`.
    fn rusk_block(&self, index: usize) -> Option<(SyntaxNode, bool)> {
        let brace = &self.tokens[index];
        if brace.kind() != T!['{'] {
            return None;
        }
        let list = brace.parent()?;
        let close = list.last_token()?;
        let is_list = BLOCK_LISTS.contains(&list.kind())
            || (list.kind() == TOKEN_TREE
                && list
                    .parent()
                    .is_some_and(|parent| parent.kind() == MACRO_RULES));
        if !is_list || list.first_token().as_ref() != Some(brace) || close.kind() != T!['}'] {
            return None;
        }
        let line = self.start_line(brace);
        let element = self.element(brace);
        // An empty body on a line of its own, as in `while b` / `.next()` / `{}`, ends a header.
        let empty_own_line = self.first[line] == Some(index)
            && self.last_code[line] == Some(index + 1)
            && self.tokens.get(index + 1) == Some(&close)
            && element
                .as_ref()
                .and_then(SyntaxNode::first_token)
                .is_some_and(|first| self.start_line(&first) < line);
        if !(empty_own_line
            || (self.last_code[line] == Some(index)
                && self.first[self.start_line(&close)].map(|at| &self.tokens[at]) == Some(&close)
                && self.start_line(&close) != line))
        {
            return None;
        }
        let from = element
            .as_ref()
            .and_then(SyntaxNode::first_token)
            .map_or(index, |first| self.index_of(&first));
        let in_parentheses = self.open_groups(from, index)?;
        // A block in a statement's header, as in `if match x {` / `} {`, stays Rust up to the
        // header's body; the Rusk reading, as in `if match x:` / `} || b {`, holds only for a
        // `let` value.
        let in_header = element.as_ref().is_some_and(|node| node.kind() != LET_STMT)
            && list
                .ancestors()
                .take_while(|node| Some(node) != element.as_ref())
                .chain(element.clone())
                .filter_map(|node| body(&node))
                .any(|body| body.text_range().start() > brace.text_range().start());
        // A branch after one that stays Rust, as in `} else {`, stays Rust too.
        let branch = list
            .parent()
            .filter(|_| list.kind() == STMT_LIST)
            .map(|block| {
                block
                    .parent()
                    .filter(|parent| {
                        ast::IfExpr::cast(parent.clone())
                            .and_then(|it| it.then_branch())
                            .is_some_and(|then| then.syntax() == &block)
                    })
                    .unwrap_or(block)
            });
        let after_rust_branch = branch
            .and_then(|branch| previous_code(&branch.first_token()?))
            .filter(|token| {
                token.kind() == T![else] && token.parent().is_some_and(|it| it.kind() == IF_EXPR)
            })
            .and_then(|token| previous_code(&token))
            .is_some_and(|close| !self.is_closer(&close));
        // A chain line after a closing line, as in `xs.a(|x| F {` / `})` / `.b(|y| {`, stays
        // Rust unless that line closes a Rusk block a chain line opened, as in `.a(|x| {`.
        let starts_chain = |line: usize| {
            self.first[line].is_some_and(|at| matches!(self.tokens[at].kind(), T![.] | T![?]))
        };
        let after_closing_line = starts_chain(line)
            && previous_code(&self.tokens[self.first[line]?])
                .and_then(|before| self.first[self.start_line(&before)])
                .map(|at| &self.tokens[at])
                .is_some_and(|closing| {
                    matches!(closing.kind(), T!['}'] | T![')'] | T![']'])
                        && !(self.is_closer(closing)
                            && closing
                                .parent()
                                .and_then(|list| list.first_token())
                                .is_some_and(|open| starts_chain(self.start_line(&open))))
                });
        (!in_header
            && !after_rust_branch
            && !after_closing_line
            && !self.is_operand(&list, in_parentheses))
        .then_some((list, in_parentheses))
    }

    /// The open groups around the `{` at `index`, from its element's first token at `from`:
    /// `None` when the block stays Rust, inside a `[`, a brace group such as a struct literal, or
    /// a `(` group a line break inside lays out as Rust, as in `f(` / `|x| {`; otherwise whether
    /// a `(` is open. Each element is scanned once, as its blocks come in source order.
    fn open_groups(&self, from: usize, index: usize) -> Option<bool> {
        let mut scans = self.scans.borrow_mut();
        let scan = scans.entry(from).or_insert_with(|| GroupScan {
            next: from,
            ..GroupScan::default()
        });
        for at in scan.next..index {
            let token = &self.tokens[at];
            if at > from && self.start_line(token) > self.start_line(&self.tokens[at - 1]) {
                for (_, broken) in scan.open.iter_mut().rev().map_while(Option::as_mut) {
                    *broken = true;
                }
            }
            match token.kind() {
                T!['('] | T!['['] | T!['{'] => scan
                    .open
                    .push((!self.opens_rusk_block(token)).then_some((token.kind(), false))),
                T![')'] | T![']'] | T!['}'] => match scan.open.pop() {
                    // A Rusk block the element opened before, as in `} else {`, ends the
                    // header before it and the groups inside it.
                    Some(None) => scan.group_end = None,
                    Some(Some((_, true))) if !scan.open.contains(&None) => {
                        scan.group_end = Some(self.start_line(token))
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        scan.next = index;
        let groups = scan.open.iter().flatten().collect::<Vec<_>>();
        // A `(` opened on the line closing a group, as in `A(B {` / `}) => f(|x| {`, continues it.
        let continues_group =
            !groups.is_empty() && scan.group_end == Some(self.start_line(&self.tokens[index]));
        (!continues_group
            && !groups
                .iter()
                .any(|&&(kind, broken)| kind != T!['('] || broken))
        .then_some(!groups.is_empty())
    }

    /// Whether the block `{` opens in `list` is the operand of a binary operator, which stays
    /// Rust: one starting on a line the operator starts, as in `    + if c {`, or a block right
    /// after `||` or `&&` inside parentheses, as in `(a || {`.
    fn is_operand(&self, list: &SyntaxNode, in_parentheses: bool) -> bool {
        let mut owner = list
            .parent()
            .filter(|_| matches!(list.kind(), STMT_LIST | MATCH_ARM_LIST));
        while let Some(parent) = owner.as_ref().and_then(SyntaxNode::parent)
            && matches!(
                parent.kind(),
                IF_EXPR | WHILE_EXPR | FOR_EXPR | LOOP_EXPR | MATCH_EXPR
            )
        {
            owner = Some(parent);
        }
        let Some(owner) = owner else { return false };
        let Some(binary) = owner.parent().and_then(ast::BinExpr::cast) else {
            return false;
        };
        let Some(operator) = binary.op_token().filter(|_| {
            binary.rhs().is_some_and(|rhs| rhs.syntax() == &owner)
                && !matches!(binary.op_kind(), Some(ast::BinaryOp::Assignment { .. }))
        }) else {
            return false;
        };
        let starts_line = previous_code(&operator)
            .is_none_or(|before| self.start_line(&before) < self.start_line(&operator));
        let operand_line = owner.first_token().map(|first| self.start_line(&first));
        (starts_line && operand_line == Some(self.start_line(&operator)))
            || (in_parentheses
                && owner.kind() == BLOCK_EXPR
                && matches!(operator.kind(), T![||] | T![&&]))
    }

    fn group_depths(&self) -> Vec<(usize, usize)> {
        let mut open = Vec::new();
        self.tokens
            .iter()
            .enumerate()
            .map(|(index, token)| {
                match token.kind() {
                    _ if self.is_closer(token) || self.opens_rusk_block(token) => {}
                    T!['('] | T!['['] | T!['{'] => {
                        open.push(self.last_code[self.end_line(token)] == Some(index))
                    }
                    T![')'] | T![']'] | T!['}'] => {
                        open.pop();
                    }
                    _ => {}
                }
                (open.len(), open.iter().filter(|&&at_end| at_end).count())
            })
            .collect()
    }

    /// The statement, item, field, variant, or match arm holding `token` in the innermost Rusk
    /// block.
    fn element(&self, token: &SyntaxToken) -> Option<SyntaxNode> {
        token.parent_ancestors().find(|node| {
            node.parent()
                .is_some_and(|parent| self.containers.contains(&parent))
        })
    }

    fn is_closer(&self, token: &SyntaxToken) -> bool {
        self.closers.contains(&token.text_range().start())
    }

    fn opens_rusk_block(&self, brace: &SyntaxToken) -> bool {
        brace.kind() == T!['{']
            && brace
                .parent()
                .and_then(|list| list.last_token())
                .is_some_and(|close| self.is_closer(&close))
    }

    fn index_of(&self, token: &SyntaxToken) -> usize {
        self.tokens
            .partition_point(|other| other.text_range().start() < token.text_range().start())
    }

    fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset) - 1
    }

    fn start_line(&self, token: &SyntaxToken) -> usize {
        self.line_of(token.text_range().start().into())
    }

    fn end_line(&self, token: &SyntaxToken) -> usize {
        self.line_of(usize::from(token.text_range().end()) - 1)
    }
}

/// The syntax tree of `source`, built from the parser's steps as `ra_ap_syntax` does. Its
/// `Parse` result drops its tree on a thread of its own, which WebAssembly cannot start. Edition
/// 2021 reads `gen`, which 2024 reserves, as the identifier older code uses, as in `rng.gen()`.
fn parse(source: &str) -> SyntaxNode {
    let lexed = LexedStr::new(Edition::Edition2021, source);
    let output = TopEntryPoint::SourceFile.parse(&lexed.to_input(Edition::Edition2021));
    let mut builder = rowan::GreenNodeBuilder::new();
    let raw = |kind: SyntaxKind| rowan::SyntaxKind(kind.into());
    lexed.intersperse_trivia(&output, &mut |step| match step {
        StrStep::Token { kind, text } => builder.token(raw(kind), text),
        StrStep::Enter { kind } => builder.start_node(raw(kind)),
        StrStep::Exit => builder.finish_node(),
        StrStep::Error { .. } => {}
    });
    SyntaxNode::new_root(builder.finish())
}

/// The list holding the body of the item or expression `node`, such as an `if`'s block or a
/// `let`'s value, after its header.
fn body(node: &SyntaxNode) -> Option<SyntaxNode> {
    let expression = |node: Option<ast::Expr>| node.and_then(|node| body(node.syntax()));
    let block = |node: Option<ast::BlockExpr>| syntax(node.and_then(|block| block.stmt_list()));
    match_ast! {
        match node {
            ast::ExprStmt(it) => expression(it.expr()),
            ast::LetStmt(it) => expression(it.initializer())
                .or_else(|| block(it.let_else()?.block_expr())),
            ast::MatchArm(it) => expression(it.expr()),
            ast::ReturnExpr(it) => expression(it.expr()),
            ast::BinExpr(it) => expression(it.rhs())
                .filter(|_| matches!(it.op_kind(), Some(ast::BinaryOp::Assignment { .. }))),
            ast::Const(it) => expression(it.body()),
            ast::Static(it) => expression(it.body()),
            ast::IfExpr(it) => block(it.then_branch()),
            ast::WhileExpr(it) => block(it.loop_body()),
            ast::ForExpr(it) => block(it.loop_body()),
            ast::LoopExpr(it) => block(it.loop_body()),
            ast::BlockExpr(it) => syntax(it.stmt_list()),
            ast::MatchExpr(it) => syntax(it.match_arm_list()),
            ast::Fn(it) => block(it.body()),
            ast::Impl(it) => syntax(it.assoc_item_list()),
            ast::Trait(it) => syntax(it.assoc_item_list()),
            ast::Module(it) => syntax(it.item_list()),
            ast::ExternBlock(it) => syntax(it.extern_item_list()),
            ast::Enum(it) => syntax(it.variant_list()),
            ast::Struct(it) => syntax(it.field_list()),
            ast::Union(it) => syntax(it.record_field_list()),
            ast::Variant(it) => syntax(it.field_list()),
            _ => None,
        }
    }
}

fn syntax(node: Option<impl AstNode>) -> Option<SyntaxNode> {
    node.map(|node| node.syntax().clone())
}

/// The token before `token`, past whitespace and comments.
fn previous_code(token: &SyntaxToken) -> Option<SyntaxToken> {
    std::iter::successors(token.prev_token(), SyntaxToken::prev_token)
        .find(|token| !token.kind().is_trivia())
}
