//! How a tree's readers reach an [`Expression`] from the field that refers to it.
//!
//! A field of expression type is spelled with one of the aliases here, and every read of one
//! goes through an [`ExprView`], so a reader's code does not depend on what an edge is.

use std::marker::PhantomData;

use super::{AstStores, Expression, Parsed, Program};

/// One edge to an expression held in the tree.
// TODO: `ExprId`.
pub type ExprEdge<'a> = &'a Expression<'a>;

/// An edge that may be absent.
// TODO: `Option<ExprId>`.
pub type OptExprEdge<'a> = Option<ExprEdge<'a>>;

/// A list of edges: call and `new` arguments, sequence and template expressions.
// TODO: `Run<ExprId>`.
pub type ExprList<'a> = &'a [ExprEdge<'a>];

/// A list of edges with holes: array elements.
// TODO: `Run<Option<ExprId>>`.
pub type OptExprList<'a> = &'a [OptExprEdge<'a>];

/// A run of expressions held by value: parameter lists.
// TODO: `Run<Expr>`.
pub type ExprRun<'a> = &'a [Expression<'a>];

/// An expression its parent holds by value: read through [`ExprView::slot`], as an edge is
/// through [`ExprView::expr`].
// TODO: `ExprId`.
pub type ExprSlot<'a> = Expression<'a>;

/// An array pattern's elements, with holes.
// TODO: `OptExprList`, the elements read as edges.
pub type PatternElements<'a> = &'a [Option<ExprSlot<'a>>];

/// The read access every consumer of the tree holds: resolves edges, lists and slots.
///
/// `Copy`, and bound to the stores of the document whose tree it reads.
#[derive(Clone, Copy)]
pub struct ExprView<'a> {
    // TODO: the `StoreView`s of `exprs`, `expr_lists` and `expr_opt_lists`.
    stores: PhantomData<&'a ()>,
}

impl<'a> ExprView<'a> {
    /// A view for the parser's reads of nodes it has just built, which hold no borrow of the
    /// stores across a mutable call of the parser.
    // TODO: the parser's predicates take ids and read the stores themselves.
    #[inline]
    pub fn of_parse() -> ExprView<'static> {
        ExprView {
            stores: PhantomData,
        }
    }

    /// The expression `edge` names.
    #[inline]
    pub fn expr(self, edge: ExprEdge<'a>) -> &'a Expression<'a> {
        edge
    }

    /// The expression `edge` names, when it is present.
    #[inline]
    pub fn opt(self, edge: OptExprEdge<'a>) -> Option<&'a Expression<'a>> {
        edge
    }

    /// The edges of `list`.
    #[inline]
    pub fn list(self, list: ExprList<'a>) -> &'a [ExprEdge<'a>] {
        list
    }

    /// The edges of `list`, holes included.
    #[inline]
    pub fn opt_list(self, list: OptExprList<'a>) -> &'a [OptExprEdge<'a>] {
        list
    }

    /// The expressions of `run`.
    #[inline]
    pub fn run(self, run: ExprRun<'a>) -> &'a [Expression<'a>] {
        run
    }

    /// The expression `slot` holds.
    #[inline]
    pub fn slot(self, slot: &'a ExprSlot<'a>) -> &'a Expression<'a> {
        slot
    }

    /// The expression `slot` holds, when it holds one.
    #[inline]
    pub fn opt_slot(self, slot: &'a Option<ExprSlot<'a>>) -> Option<&'a Expression<'a>> {
        slot.as_ref()
    }

    /// The elements of an array pattern.
    #[inline]
    pub fn pattern_elements(self, elements: PatternElements<'a>) -> &'a [Option<ExprSlot<'a>>] {
        elements
    }
}

/// An element of a list with holes, read whichever way the list holds it.
// TODO: every such list holds `Option<ExprId>`, and this resolves through `ExprView::expr`.
pub trait ListElement {
    /// The expression this element names.
    fn element<'s>(&'s self, view: ExprView<'s>) -> &'s Expression<'s>;
}

impl ListElement for &Expression<'_> {
    #[inline]
    fn element<'s>(&'s self, view: ExprView<'s>) -> &'s Expression<'s> {
        view.expr(self)
    }
}

impl ListElement for Expression<'_> {
    #[inline]
    fn element<'s>(&'s self, view: ExprView<'s>) -> &'s Expression<'s> {
        view.slot(self)
    }
}

impl<'arena> AstStores<'arena> {
    /// The view reading the expressions of these stores.
    #[inline]
    pub fn view(&self) -> ExprView<'_> {
        ExprView {
            stores: PhantomData,
        }
    }
}

impl<'arena> Parsed<'arena> {
    /// The view reading this tree's expressions.
    #[inline]
    pub fn view(&self) -> ExprView<'_> {
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
    pub view: ExprView<'a>,
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
