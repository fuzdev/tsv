//! How a tree's readers reach a stored node from the field that refers to it.
//!
//! One [`AstView`] serves every family of the tree the stores hold; today that is the
//! expressions. An expression edge is an [`ExprId`], a list an id run or a value run over the
//! stores, and every read of one goes through the view. The view is one word: a walk reaches a
//! row through a load of the store's slice header and an indexed load, and holds one register
//! for the view.

use tsv_lang::store::Run;

use super::{AstStores, Expr, ExprId, Expression, Parsed, Program};

/// The read access every consumer of the tree holds: resolves edges and the three kinds of
/// list.
///
/// `Copy`, and bound to the stores of the document whose tree it reads.
#[derive(Clone, Copy)]
pub struct AstView<'a> {
    stores: &'a AstStores<'a>,
}

// One word, in the check build too: a walk's frame and registers are what it pays for.
#[cfg(test)]
const _: () = assert!(size_of::<AstView<'static>>() == size_of::<usize>());

impl<'a> AstView<'a> {
    /// The expression `id` names. A miss ends in the store's shared cold path.
    #[inline]
    pub fn expr(self, id: ExprId) -> &'a Expression<'a> {
        self.stores.exprs.get(id)
    }

    /// The expression `id` names, when it is present.
    #[inline]
    pub fn opt(self, id: Option<ExprId>) -> Option<&'a Expression<'a>> {
        id.map(|id| self.expr(id))
    }

    /// The ids of `list`: call and `new` arguments, sequence and template expressions.
    #[inline]
    pub fn list(self, list: Run<ExprId>) -> &'a [ExprId] {
        self.stores.expr_lists.slice(list)
    }

    /// The ids of `list`, holes included: array elements and array-pattern elements.
    #[inline]
    pub fn opt_list(self, list: Run<Option<ExprId>>) -> &'a [Option<ExprId>] {
        self.stores.expr_opt_lists.slice(list)
    }

    /// The expressions of `run`, held by value: parameter lists.
    #[inline]
    pub fn run(self, run: Run<Expr>) -> &'a [Expression<'a>] {
        self.stores.exprs.slice(run)
    }
}

impl<'arena> AstStores<'arena> {
    /// The view reading these stores.
    #[inline]
    pub fn view(&self) -> AstView<'_> {
        AstView { stores: self }
    }
}

impl<'arena> Parsed<'arena> {
    /// The view reading this tree's stores.
    #[inline]
    pub fn view(&self) -> AstView<'_> {
        self.stores.view()
    }
}

/// A program as a reading entry point takes it: the tree and the view its edges resolve
/// through.
#[derive(Clone, Copy)]
pub struct ParsedRef<'a> {
    /// The tree.
    pub program: &'a Program<'a>,
    /// The view over the stores the tree's edges index.
    pub view: AstView<'a>,
}

impl<'a, 'arena: 'a> From<&'a Parsed<'arena>> for ParsedRef<'a> {
    #[inline]
    fn from(parsed: &'a Parsed<'arena>) -> Self {
        ParsedRef {
            program: &parsed.program,
            view: parsed.view(),
        }
    }
}
