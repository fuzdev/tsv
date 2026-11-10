//! How a tree's readers reach a stored node from the field that refers to it.
//!
//! One [`AstView`] serves every family of the tree the stores hold: the expressions and the
//! statements. An edge is an id ([`ExprId`], [`StmtId`]), a list an id run or a value run over
//! the stores, and every read of one goes through the view. The view is one word: a walk
//! reaches a row through a load of the store's slice header and an indexed load, and holds one
//! register for the view.
//!
//! Type edges and lists are read through the view too, spelled with the aliases here while the
//! tree still holds them as references and slices.

use tsv_lang::store::Run;

use super::{AstStores, Expr, ExprId, Expression, Parsed, Program, Statement, StmtId, TSType};

/// One edge to a type held in the tree: an annotation's type, an operand of a type operator,
/// a conditional's four branches, a type parameter's constraint and default, an assertion's
/// type, and the rest of the type grammar's single children.
// TODO: `TyId`.
pub type TyEdge<'a> = &'a TSType<'a>;

/// A type edge that may be absent.
// TODO: `Option<TyId>`.
pub type OptTyEdge<'a> = Option<TyEdge<'a>>;

/// A type list: a union's or an intersection's members, a tuple's elements, a template
/// literal type's interpolations, a type-argument list. Its elements are edges, each read
/// through [`AstView::ty`].
// TODO: `Run<TyId>`.
pub type TyList<'a> = &'a [TyEdge<'a>];

/// The read access every consumer of the tree holds: resolves edges and lists.
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

    /// The statement `id` names. A miss ends in the store's shared cold path.
    #[inline]
    pub fn stmt(self, id: StmtId) -> &'a Statement<'a> {
        self.stores.stmts.get(id)
    }

    /// The statement `id` names, when it is present.
    #[inline]
    pub fn opt_stmt(self, id: Option<StmtId>) -> Option<&'a Statement<'a>> {
        id.map(|id| self.stmt(id))
    }

    /// The ids of `list`: a program's, a block's, a `case`'s, a static block's or a
    /// namespace's body.
    #[inline]
    pub fn stmt_list(self, list: Run<StmtId>) -> &'a [StmtId] {
        self.stores.stmt_lists.slice(list)
    }

    /// The type `edge` names: once edges are ids, the row of the type store it indexes, a miss
    /// ending in the store's shared cold path.
    #[inline]
    pub fn ty(self, edge: TyEdge<'a>) -> &'a TSType<'a> {
        edge
    }

    /// The type `edge` names, when it is present.
    #[inline]
    pub fn opt_ty(self, edge: OptTyEdge<'a>) -> Option<&'a TSType<'a>> {
        edge
    }

    /// The edges of `list`, each resolved through [`Self::ty`]: once lists are id runs, the
    /// run's ids in the type-list store. A union's or an intersection's members, a tuple's
    /// elements, a template literal type's interpolations, a type-argument list.
    #[inline]
    pub fn ty_list(self, list: TyList<'a>) -> &'a [TyEdge<'a>] {
        list
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
