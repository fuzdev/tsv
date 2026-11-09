//! The stores a parse's expressions live in, beside the arena that holds the rest of the
//! tree, and the per-thread parking of their capacity.
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

use tsv_lang::store::{Id, ListBuf, Row, Store};

use super::{Expression, ExpressionKind, Program, Span, ThisExpression};

/// The family tag of [`Expression`] rows, naming them without their lifetime.
pub enum Expr {}

impl Row for Expression<'_> {
    type Family = Expr;
}

/// A handle to one [`Expression`] row.
pub type ExprId = Id<Expr>;

/// Slot 0 of the expression store. No id reaches it, so its contents are never read.
const EXPR_SENTINEL: Expression<'static> = Expression {
    span: Span { start: 0, end: 0 },
    kind: ExpressionKind::ThisExpression(ThisExpression),
};

/// One document's expression stores and the scratch stacks that build their lists.
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
}

impl AstStores<'_> {
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
        } = self;
        expr_lists.reset();
        expr_opt_lists.reset();
        expr_list_scratch.reset();
        expr_opt_scratch.reset();
        AstStores {
            exprs: exprs.recycle(EXPR_SENTINEL),
            expr_lists,
            expr_opt_lists,
            expr_scratch: expr_scratch.recycle(),
            expr_list_scratch,
            expr_opt_scratch,
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
/// Derefs to the [`Program`], so a tree-reading call takes `&parsed` as it takes
/// `&program`.
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
