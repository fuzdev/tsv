//! The stores a parse's expressions live in, and the statement stores that nothing fills yet,
//! beside the arena that holds the rest of the tree, and the per-thread parking of their
//! capacity.
//!
//! [`AstStores`] is one document's: every parse of a document, embedded islands included,
//! appends to the same one, so an id means the same row wherever in the document it is
//! read, and the first reading of an acorn re-parse stays valid beside the second. Its
//! scratch lists live here and not on the parser, because a parser is built per island.
//!
//! The stores hold rows of `'arena`, so what outlives a parse is the empty form,
//! [`AstStores<'static>`]: [`AstStores::park`] rewinds and retypes the capacity, and
//! covariance alone takes a parked value back to any shorter lifetime. Handles die with
//! the parse that minted them: nothing id-shaped leaves [`with_stores`]'s closure or a
//! [`Parsed`], whose stores it keeps beside the tree they belong to.

use std::cell::Cell;
use std::fmt;
use std::ops::Deref;

use tsv_lang::store::{Id, ListBuf, ListMark, Row, Store};

use super::{
    EmptyStatement, Expression, ExpressionKind, Program, Span, Statement, StatementKind,
    ThisExpression,
};

/// The family tag of [`Expression`] rows, naming them without their lifetime.
pub enum Expr {}

impl Row for Expression<'_> {
    type Family = Expr;
}

/// A handle to one [`Expression`] row.
pub type ExprId = Id<Expr>;

#[cfg(test)]
const _: () = {
    use tsv_lang::store::{Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<ExprId>() == 4);
    assert!(STORE_CHECK || size_of::<Option<ExprId>>() == 4);
    assert!(STORE_CHECK || size_of::<Run<ExprId>>() == 8);
};

/// Slot 0 of the expression store. No id reaches it, so its contents are never read.
const EXPR_SENTINEL: Expression<'static> = Expression {
    span: Span { start: 0, end: 0 },
    kind: ExpressionKind::ThisExpression(ThisExpression),
};

/// The family tag of [`Statement`] rows, naming them without their lifetime.
pub enum Stmt {}

impl Row for Statement<'_> {
    type Family = Stmt;
}

/// A handle to one [`Statement`] row.
pub type StmtId = Id<Stmt>;

#[cfg(test)]
const _: () = {
    use tsv_lang::store::{Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<StmtId>() == 4);
    assert!(STORE_CHECK || size_of::<Option<StmtId>>() == 4);
    assert!(STORE_CHECK || size_of::<Run<StmtId>>() == 8);
};

/// Slot 0 of the statement store. No id reaches it, so its contents are never read.
const STMT_SENTINEL: Statement<'static> = Statement {
    span: Span { start: 0, end: 0 },
    kind: StatementKind::EmptyStatement(EmptyStatement),
};

/// One document's expression and statement stores and the scratch stacks that build their
/// lists.
///
/// A fresh one allocates one one-row buffer per store, for its sentinel, and nothing for the
/// scratch stacks.
pub struct AstStores<'arena> {
    pub(crate) exprs: Store<Expression<'arena>>,
    pub(crate) expr_lists: Store<ExprId>,
    pub(crate) expr_opt_lists: Store<Option<ExprId>>,
    pub(crate) expr_scratch: ListBuf<Expression<'arena>>,
    pub(crate) expr_list_scratch: ListBuf<ExprId>,
    pub(crate) expr_opt_scratch: ListBuf<Option<ExprId>>,
    // TODO: the statement stores ride the stores' lifecycle (`new`, `park`, the scratch marks)
    // but nothing pushes a row or reads one yet: statements are still arena nodes. The parser's
    // statement construction fills them and the view's statement accessors read them.
    pub(crate) stmts: Store<Statement<'arena>>,
    pub(crate) stmt_lists: Store<StmtId>,
    pub(crate) stmt_list_scratch: ListBuf<StmtId>,
}

/// The tops of the scratch stacks, taken together: a speculation's or a discarded parse's
/// restore point.
#[derive(Clone, Copy)]
pub(crate) struct ScratchMarks {
    exprs: ListMark<Expr>,
    lists: ListMark<ExprId>,
    opt_lists: ListMark<Option<ExprId>>,
    stmt_lists: ListMark<StmtId>,
}

impl AstStores<'_> {
    /// The current tops of the scratch stacks.
    #[inline]
    pub(crate) fn scratch_marks(&self) -> ScratchMarks {
        ScratchMarks {
            exprs: self.expr_scratch.mark(),
            lists: self.expr_list_scratch.mark(),
            opt_lists: self.expr_opt_scratch.mark(),
            stmt_lists: self.stmt_list_scratch.mark(),
        }
    }

    /// Truncates the scratch stacks to `marks`, discarding the frames opened since. The
    /// stores keep the rows pushed since: an abandoned row is an orphan, and any id already
    /// handed out stays valid.
    #[inline]
    pub(crate) fn truncate_scratch(&mut self, marks: ScratchMarks) {
        self.expr_scratch.truncate(marks.exprs);
        self.expr_list_scratch.truncate(marks.lists);
        self.expr_opt_scratch.truncate(marks.opt_lists);
        self.stmt_list_scratch.truncate(marks.stmt_lists);
    }

    /// Whether the scratch stacks stand at `marks`.
    #[cfg(debug_assertions)]
    pub(crate) fn scratch_is_at(&self, marks: ScratchMarks) -> bool {
        self.expr_scratch.len_since(marks.exprs) == 0
            && self.expr_list_scratch.len_since(marks.lists) == 0
            && self.expr_opt_scratch.len_since(marks.opt_lists) == 0
            && self.stmt_list_scratch.len_since(marks.stmt_lists) == 0
    }

    /// Empty stores for one document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            exprs: Store::new(EXPR_SENTINEL),
            expr_lists: Store::new(Id::SENTINEL),
            expr_opt_lists: Store::new(None),
            expr_scratch: ListBuf::new(),
            expr_list_scratch: ListBuf::new(),
            expr_opt_scratch: ListBuf::new(),
            stmts: Store::new(STMT_SENTINEL),
            stmt_lists: Store::new(Id::SENTINEL),
            stmt_list_scratch: ListBuf::new(),
        }
    }

    /// Rewinds the stores, keeping their capacity in the form that can outlive the parse.
    /// Every id and run minted so far is invalid from here on.
    // TODO: never trims. Peak row capacity stays parked for as long as the `Formatter` or the
    // thread lives, where the AST arena's rewind releases every block but the largest while
    // idle. Rows held here instead of in the `Bump` escape that release.
    #[must_use]
    pub fn park(self) -> AstStores<'static> {
        let Self {
            exprs,
            mut expr_lists,
            mut expr_opt_lists,
            expr_scratch,
            mut expr_list_scratch,
            mut expr_opt_scratch,
            stmts,
            mut stmt_lists,
            mut stmt_list_scratch,
        } = self;
        expr_lists.reset();
        expr_opt_lists.reset();
        expr_list_scratch.reset();
        expr_opt_scratch.reset();
        stmt_lists.reset();
        stmt_list_scratch.reset();
        AstStores {
            exprs: exprs.recycle(EXPR_SENTINEL),
            expr_lists,
            expr_opt_lists,
            expr_scratch: expr_scratch.recycle(),
            expr_list_scratch,
            expr_opt_scratch,
            stmts: stmts.recycle(STMT_SENTINEL),
            stmt_lists,
            stmt_list_scratch,
        }
    }
}

impl Default for AstStores<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AstStores<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AstStores").finish_non_exhaustive()
    }
}

/// A parsed program and the stores its handles index.
///
/// Derefs to the [`Program`] for the tree's own fields. A reading entry point takes `&parsed`,
/// which is also where it gets the view the tree's edges resolve through.
#[derive(Debug)]
pub struct Parsed<'arena> {
    /// The tree.
    pub program: Program<'arena>,
    /// The stores the tree's handles index.
    pub stores: AstStores<'arena>,
}

impl<'arena> Deref for Parsed<'arena> {
    type Target = Program<'arena>;

    fn deref(&self) -> &Program<'arena> {
        &self.program
    }
}

/// Run `f` over this thread's parked stores, or over fresh ones when there are none:
/// `tsv_arena`'s take-and-park protocol, here because that crate cannot name these types.
///
/// The stores are out of the slot while `f` runs, so a nested call takes fresh ones and a
/// panic out of `f` leaves the slot empty, as a new thread finds it. Parked rewound, so
/// every id and run `f` minted is invalid once it returns.
pub fn with_stores<'arena, R>(f: impl FnOnce(&mut AstStores<'arena>) -> R) -> R {
    thread_local! {
        static PARKED: Cell<Option<AstStores<'static>>> = const { Cell::new(None) };
    }
    let mut stores: AstStores<'arena> = PARKED.with(Cell::take).unwrap_or_default();
    let result = f(&mut stores);
    PARKED.with(|cell| cell.set(Some(stores.park())));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_a_row(stores: &mut AstStores<'_>) -> usize {
        stores.exprs.push(EXPR_SENTINEL).index()
    }

    #[test]
    fn a_parked_store_takes_back_rewound_at_a_shorter_lifetime() {
        fn take_back<'arena>(
            parked: AstStores<'static>,
            _arena: &'arena bumpalo::Bump,
        ) -> AstStores<'arena> {
            parked
        }
        let bump = bumpalo::Bump::new();
        let mut stores = take_back(AstStores::new().park(), &bump);
        assert_eq!(with_a_row(&mut stores), 1);
        let mut stores = take_back(stores.park(), &bump);
        assert_eq!(with_a_row(&mut stores), 1, "the park rewinds");
    }

    #[test]
    fn a_park_rewinds_the_statement_stores() {
        let mut stores = AstStores::new();
        stores.stmts.push(STMT_SENTINEL);
        stores.stmts.push(STMT_SENTINEL);
        stores.stmt_lists.push(Id::SENTINEL);
        stores.stmt_lists.push(Id::SENTINEL);
        let mut stores = stores.park();
        assert_eq!(
            stores.stmts.push(STMT_SENTINEL).index(),
            1,
            "the rows rewind"
        );
        assert_eq!(
            stores.stmt_lists.push(Id::SENTINEL).index(),
            1,
            "the id lists rewind"
        );
    }

    #[test]
    fn with_stores_hands_out_rewound_stores() {
        with_stores(|stores: &mut AstStores<'_>| {
            with_a_row(stores);
            with_a_row(stores);
        });
        with_stores(|stores: &mut AstStores<'_>| {
            assert_eq!(with_a_row(stores), 1);
        });
    }

    #[test]
    fn with_stores_recovers_after_a_caught_panic() {
        let caught = std::panic::catch_unwind(|| {
            with_stores(|_: &mut AstStores<'_>| panic!("boom"));
        });
        assert!(caught.is_err());
        with_stores(|stores: &mut AstStores<'_>| {
            assert_eq!(with_a_row(stores), 1);
        });
    }

    #[test]
    fn a_nested_with_stores_takes_fresh_stores() {
        with_stores(|outer: &mut AstStores<'_>| {
            with_a_row(outer);
            with_stores(|inner: &mut AstStores<'_>| {
                assert_eq!(with_a_row(inner), 1, "not the stores already in use");
            });
            assert_eq!(with_a_row(outer), 2, "the outer stores are untouched");
        });
    }
}
