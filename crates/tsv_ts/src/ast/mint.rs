//! The write surface the stores offer a crate outside this one: push a row, build a list, copy
//! a string into the pool — what a host that builds TypeScript of its own around a parsed tree
//! (a Svelte document's synthesized expressions, a compiler's output program) writes with.
//! Re-exported from `crate::__private`; not public API.
//!
//! The functions are free functions over `&mut AstStores`, generic over the row or element
//! type: [`MintRow`] for a row pushed one at a time (an [`Expression`], a [`Statement`], every
//! payload), [`MintList`] for a list element (an id list's [`ExprId`], [`Option<ExprId>`] and
//! [`StmtId`], a program's [`Comment`]s, a parameter list's [`Expression`]s, every value-run
//! row). Both traits are sealed and their impls are written by the store macros, so a payload
//! or a value run added there is writable here with no line of its own. What a sealed trait
//! hands out — a store or a scratch stack by `&mut` — takes a seal argument no other crate can
//! build, so the only rows another crate writes beside a parse's are the ones below: a row is
//! pushed whole and never edited, and no single store or scratch stack is reset or truncated
//! from outside — the stores reset only as a whole ([`AstStores::reset`]), every id at once.
//!
//! The contracts a writer inherits:
//!
//! - **Ids die with their parse.** An id, run or pool text minted here belongs to these stores
//!   until their next reset, like the ids the parse minted beside it, and reads nowhere else.
//!   A host mints into the stores of the tree it builds around and lets nothing it minted
//!   outlive them.
//! - **A row is read in place.** A row read through the view and copied out
//!   (`let row = *stores.view().expr(id)`) is a copy: build the new row from it and push that,
//!   and key nothing by the copy's address. A reference into a store cannot be held across a
//!   push (the borrow forbids it), so push into a local first, then read.
//! - **Frames nest last in, first out**, as the parser's do: [`open`] a frame, fill it with
//!   [`push_item`] and [`extend_from_run`], [`flush`] it before any frame opened under it. A
//!   frame left open is the host's to flush; the next parse into these stores asserts, in a
//!   build with debug assertions, that every scratch stack is empty.

use std::ops::Range;

use tsv_lang::store::{Id, ListBuf, ListMark, PoolText, Run, Store};

use super::{AstStores, Comment, ExprId, Expression, Statement, StmtId, Text};

pub(super) mod sealed {
    use tsv_lang::store::{ListBuf, Store};

    use super::super::AstStores;

    /// The argument that keeps a sealed trait's accessors to this crate: no other crate can
    /// build one, so none can call them, even through a bound on the public trait.
    pub struct Seal(());

    impl Seal {
        /// The one way to build a seal.
        pub(in crate::ast) const NEW: Self = Self(());
    }

    /// A row pushed one at a time: its store.
    pub trait RowStore: Copy {
        /// The store this row type is pushed into.
        fn store(stores: &mut AstStores, seal: Seal) -> &mut Store<Self>;
    }

    /// A list element: its store and the scratch stack that builds its runs, borrowed apart.
    pub trait ListStore: Copy {
        /// The store this element type's runs live in and the scratch stack that builds them.
        fn split(stores: &mut AstStores, seal: Seal) -> (&mut Store<Self>, &mut ListBuf<Self>);
    }
}

use sealed::{ListStore, RowStore, Seal};

/// A row [`push`] can write: an [`Expression`], a [`Statement`], or a payload, each into its
/// own store. Sealed: the impls are this crate's.
pub trait MintRow: RowStore {}

impl<T: RowStore> MintRow for T {}

/// A list element [`open`], [`push_item`], [`extend_from_run`], [`flush`] and [`push_list`]
/// build runs of: an [`ExprId`] (call arguments, sequences), an [`Option<ExprId>`] (array
/// elements), a [`StmtId`] (a body), a [`Comment`] (a program's comments), an [`Expression`]
/// (a parameter list) or a value-run row. Sealed: the impls are this crate's.
pub trait MintList: ListStore {}

impl<T: ListStore> MintList for T {}

impl RowStore for Expression {
    #[inline]
    fn store(stores: &mut AstStores, _: Seal) -> &mut Store<Self> {
        &mut stores.exprs
    }
}

impl RowStore for Statement {
    #[inline]
    fn store(stores: &mut AstStores, _: Seal) -> &mut Store<Self> {
        &mut stores.stmts
    }
}

impl ListStore for Expression {
    #[inline]
    fn split(stores: &mut AstStores, _: Seal) -> (&mut Store<Self>, &mut ListBuf<Self>) {
        (&mut stores.exprs, &mut stores.expr_scratch)
    }
}

impl ListStore for ExprId {
    #[inline]
    fn split(stores: &mut AstStores, _: Seal) -> (&mut Store<Self>, &mut ListBuf<Self>) {
        (&mut stores.expr_lists, &mut stores.expr_list_scratch)
    }
}

impl ListStore for Option<ExprId> {
    #[inline]
    fn split(stores: &mut AstStores, _: Seal) -> (&mut Store<Self>, &mut ListBuf<Self>) {
        (&mut stores.expr_opt_lists, &mut stores.expr_opt_scratch)
    }
}

impl ListStore for StmtId {
    #[inline]
    fn split(stores: &mut AstStores, _: Seal) -> (&mut Store<Self>, &mut ListBuf<Self>) {
        (&mut stores.stmt_lists, &mut stores.stmt_list_scratch)
    }
}

impl ListStore for Comment {
    #[inline]
    fn split(stores: &mut AstStores, _: Seal) -> (&mut Store<Self>, &mut ListBuf<Self>) {
        (&mut stores.comments, &mut stores.comment_scratch)
    }
}

/// Push `row` into its store and mint its id, counted by the AST census as the parser's
/// pushes are. The id reads in these stores until their next reset.
#[inline]
pub fn push<T: MintRow>(stores: &mut AstStores, row: T) -> Id<T> {
    tsv_lang::store::push_row(T::store(stores, Seal::NEW), row)
}

/// Open a frame on `T`'s scratch stack: the start of a list [`flush`] turns into a run.
#[inline]
pub fn open<T: MintList>(stores: &mut AstStores) -> ListMark<T> {
    T::split(stores, Seal::NEW).1.mark()
}

/// Push `item` onto the frame open at the top of `T`'s scratch stack.
#[inline]
pub fn push_item<T: MintList>(stores: &mut AstStores, item: T) {
    T::split(stores, Seal::NEW).1.push(item);
}

/// Copy the elements `range` names in `run` onto the frame open at the top of `T`'s scratch
/// stack, as one block: a rebuilt list's unchanged prefix or suffix. `run` is a run of these
/// stores; a `range` past its end ends in the store's shared cold path.
#[inline]
pub fn extend_from_run<T: MintList>(stores: &mut AstStores, run: Run<T>, range: Range<usize>) {
    let (store, scratch) = T::split(stores, Seal::NEW);
    scratch.extend_from_slice(store.slice(run.sub_run(range)));
}

/// Flush the frame opened at `mark` into `T`'s store as one block and mint its run, counted by
/// the AST census as the parser's flushes are. Every frame opened above `mark` must already be
/// flushed. An empty frame is [`Run::EMPTY`].
#[inline]
pub fn flush<T: MintList>(stores: &mut AstStores, mark: ListMark<T>) -> Run<T> {
    let (store, scratch) = T::split(stores, Seal::NEW);
    let run = scratch.flush_copy(mark, store);
    #[cfg(feature = "_internal_ast_census")]
    tsv_lang::ast_census_probe::mint::<T>(run.len());
    run
}

/// `items` as one run of `T`'s store: a list the host gathered itself, opened, filled and
/// flushed at once.
///
/// Out of line: a host-side builder off every hot path, so its open, copy and flush stay one
/// body rather than joining each caller's code.
#[inline(never)]
pub fn push_list<T: MintList>(stores: &mut AstStores, items: &[T]) -> Run<T> {
    let mark = open::<T>(stores);
    for &item in items {
        push_item(stores, item);
    }
    flush::<T>(stores, mark)
}

/// `text` copied into the tree's text pool: a decoded string no span of the source spells (a
/// string literal's cooked value the host built). The handle reads through
/// [`AstView::text`](super::AstView::text) until the stores' next reset.
#[inline]
pub fn push_text(stores: &mut AstStores, text: &str) -> PoolText<Text> {
    stores.text.push(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        ArrowFunctionBody, BindingExtra, EscapedName, ExpressionKind, Program, StatementKind,
        VariableDeclaration, VariableDeclarator,
    };

    fn parse(source: &str, stores: &mut AstStores) -> Program {
        crate::__private::parse_into(source, stores).expect("parses")
    }

    /// The program's statements, copied out.
    fn statements(stores: &AstStores, program: &Program) -> Vec<Statement> {
        let view = stores.view();
        view.stmt_list(program.body)
            .iter()
            .map(|&id| *view.stmt(id))
            .collect()
    }

    fn declaration(statement: &Statement) -> VariableDeclaration {
        match statement.kind {
            StatementKind::VariableDeclaration(decl) => decl,
            _ => panic!("a variable declaration"),
        }
    }

    /// The call an expression statement holds, as its callee and argument list.
    fn call(stores: &AstStores, statement: &Statement) -> (ExprId, Run<ExprId>) {
        let StatementKind::ExpressionStatement(statement) = statement.kind else {
            panic!("an expression statement");
        };
        match stores.view().expr(statement.expression).kind {
            ExpressionKind::CallExpression(call) => (call.callee, call.arguments),
            _ => panic!("a call"),
        }
    }

    /// `program` with `body` in place of its own, formatted against `source`.
    fn format_with_body(
        stores: &AstStores,
        program: &Program,
        body: Run<StmtId>,
        source: &str,
    ) -> String {
        let rebuilt = Program {
            body,
            ..program.clone()
        };
        crate::__private::format_ast(crate::__private::parsed_ref(&rebuilt, stores), source)
    }

    #[cfg(debug_assertions)]
    fn assert_scratch_empty(stores: &AstStores) {
        assert!(stores.scratch_is_empty(), "every frame flushed");
    }

    #[cfg(not(debug_assertions))]
    fn assert_scratch_empty(_: &AstStores) {}

    /// A pushed row of each kind reads back through the view: an expression, a statement and
    /// two payloads, one holding pool text.
    #[test]
    fn a_pushed_row_reads_back_through_the_view() {
        let mut stores = AstStores::new();
        let program = parse("x; let y = 1;", &mut stores);
        let [first, second] = statements(&stores, &program)[..] else {
            panic!("two statements");
        };
        let StatementKind::ExpressionStatement(statement) = first.kind else {
            panic!("an expression statement");
        };
        let expr = *stores.view().expr(statement.expression);
        let expr_id = push(&mut stores, expr);
        let stmt_id = push(&mut stores, second);
        let text = push_text(&mut stores, "a\u{62}c");
        let name = push(&mut stores, EscapedName { text });
        let extra = push(
            &mut stores,
            BindingExtra {
                type_annotation: None,
                decorators: Some(Run::EMPTY),
            },
        );
        let view = stores.view();
        assert_eq!(view.expr(expr_id).span, expr.span);
        assert_eq!(view.stmt(stmt_id).span, second.span);
        assert_eq!(view.text(view.escaped_name(name).text), "abc");
        assert!(view.binding_extra(extra).type_annotation.is_none());
        assert_eq!(view.binding_extra(extra).decorators, Some(Run::EMPTY));
        let empty = push_text(&mut stores, "");
        assert_eq!(stores.view().text(empty), "");
    }

    /// `push_list` builds a run of every list element type, read back in order, and an empty
    /// slice is the empty run.
    #[test]
    fn push_list_builds_a_run_of_every_element_type() {
        let mut stores = AstStores::new();
        let source = "// c\nf(a, b); let x = 1, y = 2; [p, , q]; (m, n) => m;";
        let program = parse(source, &mut stores);
        let statements = statements(&stores, &program);
        let view = stores.view();
        let (_, arguments) = call(&stores, &statements[0]);
        let ids: Vec<ExprId> = view.expr_list(arguments).to_vec();
        let declarators: Vec<VariableDeclarator> = view
            .declarators(declaration(&statements[1]).declarations)
            .to_vec();
        let StatementKind::ExpressionStatement(array) = statements[2].kind else {
            panic!("an expression statement");
        };
        let ExpressionKind::ArrayExpression(array) = view.expr(array.expression).kind else {
            panic!("an array");
        };
        let holes: Vec<Option<ExprId>> = view.expr_opt_list(array.elements).to_vec();
        let StatementKind::ExpressionStatement(arrow) = statements[3].kind else {
            panic!("an expression statement");
        };
        let ExpressionKind::ArrowFunctionExpression(arrow) = view.expr(arrow.expression).kind
        else {
            panic!("an arrow");
        };
        let params: Vec<Expression> = view.exprs(view.arrow_fn(arrow).params).to_vec();
        let body: Vec<StmtId> = view.stmt_list(program.body).to_vec();
        let comments: Vec<Comment> = view.comments(program.comments).to_vec();
        assert_eq!(comments.len(), 1);

        let id_run = push_list(&mut stores, &ids);
        let decl_run = push_list(&mut stores, &declarators);
        let hole_run = push_list(&mut stores, &holes);
        let param_run = push_list(&mut stores, &params);
        let body_run = push_list(&mut stores, &body);
        let comment_run = push_list(&mut stores, &comments);
        assert_eq!(push_list::<ExprId>(&mut stores, &[]), Run::EMPTY);
        assert_scratch_empty(&stores);

        let view = stores.view();
        assert_eq!(view.expr_list(id_run), &ids[..]);
        let spans = |rows: &[VariableDeclarator]| rows.iter().map(|d| d.span).collect::<Vec<_>>();
        assert_eq!(spans(view.declarators(decl_run)), spans(&declarators));
        assert_eq!(view.expr_opt_list(hole_run), &holes[..]);
        assert_eq!(holes.iter().filter(|hole| hole.is_none()).count(), 1);
        let param_spans: Vec<_> = view.exprs(param_run).iter().map(|e| e.span).collect();
        assert_eq!(
            param_spans,
            params.iter().map(|e| e.span).collect::<Vec<_>>()
        );
        assert_eq!(view.stmt_list(body_run), &body[..]);
        assert_eq!(view.comments(comment_run).len(), 1);
        assert_eq!(view.comments(comment_run)[0].span, comments[0].span);
        assert_eq!(
            format_with_body(&stores, &program, body_run, source),
            crate::__private::format_ast(crate::__private::parsed_ref(&program, &stores), source)
        );
    }

    /// The lazy-prefix rebuild a rewriter runs over a value run: the first changed element
    /// opens the frame and copies the unchanged prefix in one block, later elements are pushed
    /// as they come, a nested frame of the same element type opened and flushed inside the
    /// rebuild leaves the outer frame intact, and the rebuilt list prints as its source would.
    #[test]
    fn a_lazy_prefix_rebuild_of_a_value_run_keeps_the_prefix_and_suffix() {
        let mut stores = AstStores::new();
        let source = "let a = 1, b = 2, c = 3, d = 4;\n";
        let program = parse(source, &mut stores);
        let statement = statements(&stores, &program)[0];
        let decl = declaration(&statement);
        let run = decl.declarations;
        let last_init = stores.view().declarators(run)[3].init;

        let mut out: Option<ListMark<VariableDeclarator>> = None;
        let mut nested: Run<VariableDeclarator> = Run::EMPTY;
        for i in 0..run.len() {
            let item = stores.view().declarators(run)[i];
            if i == 1 {
                // the element's own rewrite builds a list of the same element type, opened and
                // flushed before the outer frame exists
                let mark = open::<VariableDeclarator>(&mut stores);
                push_item(&mut stores, item);
                nested = flush(&mut stores, mark);
            }
            let changed = (i == 1).then_some(VariableDeclarator {
                init: last_init,
                ..item
            });
            match changed {
                None => {
                    if out.is_some() {
                        push_item(&mut stores, item);
                    }
                }
                Some(new) => {
                    let mark = open::<VariableDeclarator>(&mut stores);
                    extend_from_run(&mut stores, run, 0..i);
                    out = Some(mark);
                    // a nested frame above the outer one, flushed before the outer goes on
                    let inner = open::<VariableDeclarator>(&mut stores);
                    extend_from_run(&mut stores, run, i + 1..run.len());
                    let tail = flush(&mut stores, inner);
                    assert_eq!(tail.len(), 2);
                    push_item(&mut stores, new);
                }
            }
        }
        let rebuilt = flush(&mut stores, out.expect("a change opened the frame"));
        assert_scratch_empty(&stores);
        assert_eq!(nested.len(), 1);
        assert_eq!(stores.view().declarators(rebuilt).len(), 4);

        let new_stmt = push(
            &mut stores,
            Statement {
                kind: StatementKind::VariableDeclaration(VariableDeclaration {
                    declarations: rebuilt,
                    ..decl
                }),
                ..statement
            },
        );
        let body = push_list(&mut stores, &[new_stmt]);
        assert_eq!(
            format_with_body(&stores, &program, body, source),
            crate::format("let a = 1, b = 4, c = 3, d = 4;\n").expect("formats")
        );
    }

    /// The same rebuild over an id list, the arguments of a call, with its kept tail taken as
    /// a sub-run.
    #[test]
    fn a_rebuilt_argument_list_prints_as_its_source_would() {
        let mut stores = AstStores::new();
        let source = "f(x, y, z);\n";
        let program = parse(source, &mut stores);
        let statement = statements(&stores, &program)[0];
        let (callee, arguments) = call(&stores, &statement);
        let z = stores.view().expr_list(arguments)[2];

        let mark = open::<ExprId>(&mut stores);
        extend_from_run(&mut stores, arguments, 0..1);
        push_item(&mut stores, z);
        extend_from_run(&mut stores, arguments, 2..3);
        let rebuilt = flush(&mut stores, mark);
        assert_eq!(stores.view().expr_list(arguments.sub_run(1..3))[1], z);

        let StatementKind::ExpressionStatement(expression_statement) = statement.kind else {
            panic!("an expression statement");
        };
        let call_row = *stores.view().expr(expression_statement.expression);
        let ExpressionKind::CallExpression(call) = call_row.kind else {
            panic!("a call");
        };
        assert_eq!(call.callee, callee);
        let new_call = push(
            &mut stores,
            Expression {
                kind: ExpressionKind::CallExpression(crate::ast::CallExpression {
                    arguments: rebuilt,
                    ..call
                }),
                ..call_row
            },
        );
        let new_stmt = push(
            &mut stores,
            Statement {
                kind: StatementKind::ExpressionStatement(crate::ast::ExpressionStatement {
                    expression: new_call,
                    ..expression_statement
                }),
                ..statement
            },
        );
        let body = push_list(&mut stores, &[new_stmt]);
        assert_scratch_empty(&stores);
        assert_eq!(
            format_with_body(&stores, &program, body, source),
            "f(x, z, z);\n"
        );
    }

    fn address<T>(row: &T) -> usize {
        (&raw const *row).addr()
    }

    /// The store-range guard is true for every row a walk of the parse reaches through the
    /// view, of each kind of store (a statement and its id list, a value run, an expression
    /// and an id list, a payload and the parameter run it holds, a comment, a field inside a
    /// row), and false for a copy of any of them and for the program root.
    #[test]
    fn the_guard_holds_every_row_read_in_place_and_no_copy() {
        let mut stores = AstStores::new();
        let source = "// c\nlet a = f(x), b = (p) => p;\n";
        let program = parse(source, &mut stores);
        let view = stores.view();
        let holds = |address: usize| crate::__private::holds_address(view, address);

        let body = view.stmt_list(program.body);
        assert!(holds(address(&body[0])));
        let statement = view.stmt(body[0]);
        assert!(holds(address(statement)));
        let StatementKind::VariableDeclaration(decl) = &statement.kind else {
            panic!("a variable declaration");
        };
        assert!(holds(address(decl)), "a field inside a row lies in the row");
        let declarators = view.declarators(decl.declarations);
        assert!(holds(address(&declarators[1])));
        let init = view.expr(declarators[0].init.expect("an init"));
        assert!(holds(address(init)));
        let ExpressionKind::CallExpression(call) = &init.kind else {
            panic!("a call");
        };
        assert!(holds(address(&view.expr_list(call.arguments)[0])));
        let arrow = view.expr(declarators[1].init.expect("an init"));
        let ExpressionKind::ArrowFunctionExpression(arrow) = arrow.kind else {
            panic!("an arrow");
        };
        let arrow = view.arrow_fn(arrow);
        assert!(holds(address(arrow)));
        assert!(holds(address(&view.exprs(arrow.params)[0])));
        let ArrowFunctionBody::Expression(body_expr) = arrow.body else {
            panic!("an expression body");
        };
        assert!(holds(address(view.expr(body_expr))));
        let comment = &view.comments(program.comments)[0];
        assert!(holds(address(comment)));

        let copies = (*statement, declarators[1], *init, *arrow, *comment);
        assert!(!holds(address(&copies.0)));
        assert!(!holds(address(&copies.1)));
        assert!(!holds(address(&copies.2)));
        assert!(!holds(address(&copies.3)));
        assert!(!holds(address(&copies.4)));
        assert!(!holds(address(&program)), "the root is no store's row");
    }
}
