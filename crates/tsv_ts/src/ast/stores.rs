//! The stores a parse's expressions, statements and types live in, beside the arena that holds
//! the rest of the tree, and the per-thread parking of their capacity.
//!
//! Beside those three families, [`PayloadStores`] holds one store per payload too rare to sit
//! inline in the node that refers to it: the function, class and declaration payloads, the
//! constructor, import and `infer` types, qualified names, a `for` head, a `for … in` / `for …
//! of` left side, a catch clause and a parameter name's extras. One macro declares each such
//! store with its family tag, its id and its view accessor.
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
    ArrowFunctionExpression, AstView, CatchClause, ClassDeclaration, ClassExpression,
    ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration, Expression,
    ForInOfLeft, ForInit, FunctionDeclaration, FunctionExpression, IdentifierParamExtra,
    ImportDeclaration, MetaProperty, Program, Statement, TSConstructorType, TSDeclareFunction,
    TSEnumDeclaration, TSExportAssignment, TSImportEqualsDeclaration, TSImportType, TSInferType,
    TSInterfaceDeclaration, TSModuleDeclaration, TSQualifiedName, TSType, TSTypeAliasDeclaration,
    TaggedTemplateExpression,
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

/// The family tag of [`TSType`] rows, naming them without their lifetime.
pub enum Ty {}

impl Row for TSType<'_> {
    type Family = Ty;
}

/// A handle to one [`TSType`] row.
pub type TyId = Id<Ty>;

#[cfg(test)]
const _: () = {
    use tsv_lang::store::{Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<TyId>() == 4);
    assert!(STORE_CHECK || size_of::<Option<TyId>>() == 4);
    assert!(STORE_CHECK || size_of::<Run<TyId>>() == 8);
};

/// Declares the payload stores. Per entry: the row type's family tag and id, a
/// [`PayloadStores`] field, that field's lines in `new` and `park`, and the [`AstView`]
/// accessor that reads one row. The entry's doc comment documents the field and the tag.
macro_rules! payload_stores {
    (
        $(
            $(#[$m:meta])*
            $field:ident : $Row:ident $(<$lt:lifetime>)? => $Tag:ident, $Id:ident, $acc:ident;
        )*
    ) => {
        $(
            $(#[$m])*
            ///
            /// The family tag of these rows, naming them without their lifetime.
            pub enum $Tag {}

            impl<$($lt)?> Row for $Row$(<$lt>)? {
                type Family = $Tag;
            }

            #[doc = concat!("A handle to one [`", stringify!($Tag), "`] row.")]
            pub type $Id = Id<$Tag>;
        )*

        /// The stores of the payloads too rare to sit inline in the node that refers to them,
        /// one per row type.
        pub(crate) struct PayloadStores<'arena> {
            $(
                $(#[$m])*
                pub(crate) $field: Store<$Row$(<$lt>)?>,
            )*
        }

        impl<'arena> PayloadStores<'arena> {
            /// Empty stores, allocating nothing.
            pub(crate) fn new() -> Self {
                Self {
                    $($field: Store::new(),)*
                }
            }

            /// Rewinds and retypes the stores, as [`AstStores::park`] does.
            pub(crate) fn park(self) -> PayloadStores<'static> {
                PayloadStores {
                    $($field: self.$field.recycle(),)*
                }
            }
        }

        impl<'arena> AstView<'arena> {
            $(
                #[doc = concat!(
                    "The [`", stringify!($Tag), "`] row `id` names. A miss ends in the store's ",
                    "shared cold path."
                )]
                #[inline]
                pub fn $acc(self, id: $Id) -> &'arena $Row$(<$lt>)? {
                    self.payloads().$field.get(id)
                }
            )*
        }

        /// Every payload store's field name, for the test that walks them all.
        #[cfg(test)]
        const PAYLOAD_FIELDS: &[&str] = &[$(stringify!($field)),*];
    };
}

payload_stores! {
    /// Arrow functions, `() => …`: [`ArrowFunctionExpression`] rows.
    arrow_fns: ArrowFunctionExpression<'arena> => ArrowFn, ArrowFnId, arrow_fn;
    /// Function expressions, `function () {}`: [`FunctionExpression`] rows.
    fn_exprs: FunctionExpression<'arena> => FnExpr, FnExprId, fn_expr;
    /// Class expressions, `class {}`: [`ClassExpression`] rows.
    class_exprs: ClassExpression<'arena> => ClassExpr, ClassExprId, class_expr;
    /// Tagged templates, ``tag`…` ``: [`TaggedTemplateExpression`] rows.
    tagged_tpls: TaggedTemplateExpression<'arena> => TaggedTpl, TaggedTplId, tagged_tpl;
    /// Meta properties, `import.meta` and `new.target`: [`MetaProperty`] rows.
    meta_props: MetaProperty<'arena> => MetaProp, MetaPropId, meta_prop;
    /// Type aliases, `type T = …`: [`TSTypeAliasDeclaration`] rows.
    type_aliases: TSTypeAliasDeclaration<'arena> => TypeAlias, TypeAliasId, type_alias;
    /// Interfaces, `interface I {}`: [`TSInterfaceDeclaration`] rows.
    interface_decls: TSInterfaceDeclaration<'arena>
        => InterfaceDecl, InterfaceDeclId, interface_decl;
    /// Bodiless function declarations, `declare function f(): T`: [`TSDeclareFunction`] rows.
    declare_fns: TSDeclareFunction<'arena> => DeclareFn, DeclareFnId, declare_fn;
    /// Enums, `enum E {}`: [`TSEnumDeclaration`] rows.
    enum_decls: TSEnumDeclaration<'arena> => EnumDecl, EnumDeclId, enum_decl;
    /// Namespaces and modules, `namespace N {}`: [`TSModuleDeclaration`] rows.
    module_decls: TSModuleDeclaration<'arena> => ModuleDecl, ModuleDeclId, module_decl;
    /// Function declarations, `function f() {}`: [`FunctionDeclaration`] rows.
    fn_decls: FunctionDeclaration<'arena> => FnDecl, FnDeclId, fn_decl;
    /// Class declarations, `class C {}`: [`ClassDeclaration`] rows.
    class_decls: ClassDeclaration<'arena> => ClassDecl, ClassDeclId, class_decl;
    /// Named exports, `export { a }`: [`ExportNamedDeclaration`] rows.
    export_nameds: ExportNamedDeclaration<'arena> => ExportNamed, ExportNamedId, export_named;
    /// Default exports, `export default …`: [`ExportDefaultDeclaration`] rows.
    export_defaults: ExportDefaultDeclaration<'arena>
        => ExportDefault, ExportDefaultId, export_default;
    /// Star exports, `export * from 'm'`: [`ExportAllDeclaration`] rows.
    export_alls: ExportAllDeclaration<'arena> => ExportAll, ExportAllId, export_all;
    /// Export assignments, `export = a`: [`TSExportAssignment`] rows.
    export_assigns: TSExportAssignment => ExportAssign, ExportAssignId, export_assign;
    /// Imports, `import a from 'm'`: [`ImportDeclaration`] rows.
    import_decls: ImportDeclaration<'arena> => ImportDecl, ImportDeclId, import_decl;
    /// Import-equals declarations, `import a = require('m')`: [`TSImportEqualsDeclaration`] rows.
    import_equals: TSImportEqualsDeclaration<'arena> => ImportEquals, ImportEqualsId, import_equals;
    /// Constructor types, `new () => T`: [`TSConstructorType`] rows.
    ctor_types: TSConstructorType<'arena> => CtorType, CtorTypeId, ctor_type;
    /// Import types, `import('m')`: [`TSImportType`] rows.
    import_types: TSImportType<'arena> => ImportType, ImportTypeId, import_type;
    /// Inferred types, `infer T`: [`TSInferType`] rows.
    infer_types: TSInferType<'arena> => InferType, InferTypeId, infer_type;
    /// Qualified names, `A.B`: [`TSQualifiedName`] rows.
    qual_names: TSQualifiedName<'arena> => QualName, QualNameId, qual_name;
    /// The head of a `for (…; …; …)` statement: [`ForInit`] rows.
    for_heads: ForInit<'arena> => ForHead, ForHeadId, for_head;
    /// The left side of a `for … in` or `for … of`: [`ForInOfLeft`] rows.
    for_lefts: ForInOfLeft<'arena> => ForLeft, ForLeftId, for_left;
    /// Catch clauses, `catch (e) {}`: [`CatchClause`] rows.
    catches: CatchClause<'arena> => Catch, CatchId, catch;
    /// A parameter name's type annotation and decorators: [`IdentifierParamExtra`] rows.
    param_extras: IdentifierParamExtra<'arena> => ParamExtra, ParamExtraId, param_extra;
}

/// One document's expression, statement, type and payload stores and the scratch stacks that
/// build their lists.
///
/// A fresh one allocates nothing: a store's buffer is allocated by its first write, and a
/// scratch stack's by its first push.
pub struct AstStores<'arena> {
    pub(crate) exprs: Store<Expression<'arena>>,
    pub(crate) expr_lists: Store<ExprId>,
    pub(crate) expr_opt_lists: Store<Option<ExprId>>,
    pub(crate) expr_scratch: ListBuf<Expression<'arena>>,
    pub(crate) expr_list_scratch: ListBuf<ExprId>,
    pub(crate) expr_opt_scratch: ListBuf<Option<ExprId>>,
    pub(crate) stmts: Store<Statement<'arena>>,
    pub(crate) stmt_lists: Store<StmtId>,
    pub(crate) stmt_list_scratch: ListBuf<StmtId>,
    pub(crate) tys: Store<TSType<'arena>>,
    pub(crate) ty_lists: Store<TyId>,
    pub(crate) ty_list_scratch: ListBuf<TyId>,
    // TODO: unread. Nothing pushes or reads these stores until the payload edges move onto
    // their ids.
    pub(crate) payloads: PayloadStores<'arena>,
}

/// The tops of the scratch stacks, taken together: a speculation's or a discarded parse's
/// restore point.
#[derive(Clone, Copy)]
pub(crate) struct ScratchMarks {
    exprs: ListMark<Expr>,
    lists: ListMark<ExprId>,
    opt_lists: ListMark<Option<ExprId>>,
    stmt_lists: ListMark<StmtId>,
    ty_lists: ListMark<TyId>,
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
            ty_lists: self.ty_list_scratch.mark(),
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
        self.ty_list_scratch.truncate(marks.ty_lists);
    }

    /// Whether the scratch stacks stand at `marks`.
    #[cfg(debug_assertions)]
    pub(crate) fn scratch_is_at(&self, marks: ScratchMarks) -> bool {
        self.expr_scratch.len_since(marks.exprs) == 0
            && self.expr_list_scratch.len_since(marks.lists) == 0
            && self.expr_opt_scratch.len_since(marks.opt_lists) == 0
            && self.stmt_list_scratch.len_since(marks.stmt_lists) == 0
            && self.ty_list_scratch.len_since(marks.ty_lists) == 0
    }

    /// Empty stores for one document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            exprs: Store::new(),
            expr_lists: Store::new(),
            expr_opt_lists: Store::new(),
            expr_scratch: ListBuf::new(),
            expr_list_scratch: ListBuf::new(),
            expr_opt_scratch: ListBuf::new(),
            stmts: Store::new(),
            stmt_lists: Store::new(),
            stmt_list_scratch: ListBuf::new(),
            tys: Store::new(),
            ty_lists: Store::new(),
            ty_list_scratch: ListBuf::new(),
            payloads: PayloadStores::new(),
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
            tys,
            mut ty_lists,
            mut ty_list_scratch,
            payloads,
        } = self;
        expr_lists.reset();
        expr_opt_lists.reset();
        expr_list_scratch.reset();
        expr_opt_scratch.reset();
        stmt_lists.reset();
        stmt_list_scratch.reset();
        ty_lists.reset();
        ty_list_scratch.reset();
        AstStores {
            exprs: exprs.recycle(),
            expr_lists,
            expr_opt_lists,
            expr_scratch: expr_scratch.recycle(),
            expr_list_scratch,
            expr_opt_scratch,
            stmts: stmts.recycle(),
            stmt_lists,
            stmt_list_scratch,
            tys: tys.recycle(),
            ty_lists,
            ty_list_scratch,
            payloads: payloads.park(),
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
    use crate::__private::parse_with_goal;
    use crate::Goal;
    use crate::ast::{
        EmptyStatement, ExpressionKind, Span, StatementKind, TSEntityName, TSThisType,
        ThisExpression,
    };

    const SPAN: Span = Span { start: 0, end: 0 };

    fn an_expr() -> Expression<'static> {
        Expression {
            span: SPAN,
            kind: ExpressionKind::ThisExpression(ThisExpression),
        }
    }

    fn a_stmt() -> Statement<'static> {
        Statement {
            span: SPAN,
            kind: StatementKind::EmptyStatement(EmptyStatement),
        }
    }

    fn a_ty() -> TSType<'static> {
        TSType::ThisType(TSThisType { span: SPAN })
    }

    fn with_a_row(stores: &mut AstStores<'_>) -> usize {
        stores.exprs.push(an_expr()).index()
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
        let id = stores.stmts.push(a_stmt());
        stores.stmts.push(a_stmt());
        stores.stmt_lists.push(id);
        stores.stmt_lists.push(id);
        let mut stores = stores.park();
        let id = stores.stmts.push(a_stmt());
        assert_eq!(id.index(), 1, "the rows rewind");
        assert_eq!(stores.stmt_lists.push(id).index(), 1, "the id lists rewind");
    }

    #[test]
    fn a_park_rewinds_the_type_stores() {
        let mut stores = AstStores::new();
        let id = stores.tys.push(a_ty());
        stores.tys.push(a_ty());
        stores.ty_lists.push(id);
        stores.ty_lists.push(id);
        let mut stores = stores.park();
        let id = stores.tys.push(a_ty());
        assert_eq!(id.index(), 1, "the rows rewind");
        assert_eq!(stores.ty_lists.push(id).index(), 1, "the id lists rewind");
    }

    /// One of every payload, at module goal.
    const PAYLOADS: &str = "() => 1;
(function () {});
(class {});
f`x`;
import.meta;
type A = new () => X;
type B = import('m');
type C = D extends infer E ? 1 : 2;
type F = G.H;
interface I {}
declare function d(): void;
enum En {}
namespace N {}
function g(p: T) {}
class K {}
export { a };
export default 1;
export * from 'm';
export = a;
import i from 'm';
import q = require('m');
for (let j; ; ) {}
for (x in y) {}
try {
} catch {}
";

    /// Pushes a clone of every payload `parsed` holds into its store in `stores`, and returns
    /// each store's field name with the index the push minted.
    fn push_every_payload<'a>(
        stores: &mut AstStores<'a>,
        parsed: &'a Parsed<'_>,
    ) -> Vec<(&'static str, usize)> {
        let mut minted = Vec::new();
        macro_rules! push {
            ($field:ident, $row:expr) => {
                minted.push((
                    stringify!($field),
                    stores.payloads.$field.push($row.clone()).index(),
                ))
            };
        }
        let view = parsed.view();
        // the four type payloads ride four aliases, of which one is pushed
        let mut aliased = false;
        for &id in view.stmt_list(parsed.program.body) {
            match &view.stmt(id).kind {
                StatementKind::ExpressionStatement(s) => match &view.expr(s.expression).kind {
                    ExpressionKind::ArrowFunctionExpression(x) => push!(arrow_fns, *x),
                    ExpressionKind::FunctionExpression(x) => push!(fn_exprs, *x),
                    ExpressionKind::ClassExpression(x) => push!(class_exprs, *x),
                    ExpressionKind::TaggedTemplateExpression(x) => push!(tagged_tpls, *x),
                    ExpressionKind::MetaProperty(x) => push!(meta_props, *x),
                    other => panic!("an unexpected expression: {other:?}"),
                },
                StatementKind::TSTypeAliasDeclaration(x) => {
                    if !aliased {
                        push!(type_aliases, *x);
                        aliased = true;
                    }
                    match view.ty(x.type_annotation) {
                        TSType::Constructor(_, t) => push!(ctor_types, *t),
                        TSType::Import(_, t) => push!(import_types, *t),
                        TSType::Conditional(c) => match view.ty(c.extends_type) {
                            TSType::Infer(_, t) => push!(infer_types, *t),
                            other => panic!("an unexpected extends type: {other:?}"),
                        },
                        TSType::TypeReference(r) => match &r.type_name {
                            TSEntityName::QualifiedName(_, q) => push!(qual_names, *q),
                            other => panic!("an unexpected type name: {other:?}"),
                        },
                        other => panic!("an unexpected type: {other:?}"),
                    }
                }
                StatementKind::TSInterfaceDeclaration(x) => push!(interface_decls, *x),
                StatementKind::TSDeclareFunction(x) => push!(declare_fns, *x),
                StatementKind::TSEnumDeclaration(x) => push!(enum_decls, *x),
                StatementKind::TSModuleDeclaration(x) => push!(module_decls, *x),
                StatementKind::FunctionDeclaration(x) => {
                    push!(fn_decls, *x);
                    for param in view.run(x.params) {
                        if let ExpressionKind::Identifier(name) = &param.kind
                            && let Some(extra) = name.extra
                        {
                            push!(param_extras, extra);
                        }
                    }
                }
                StatementKind::ClassDeclaration(x) => push!(class_decls, *x),
                StatementKind::ExportNamedDeclaration(x) => push!(export_nameds, *x),
                StatementKind::ExportDefaultDeclaration(x) => push!(export_defaults, *x),
                StatementKind::ExportAllDeclaration(x) => push!(export_alls, *x),
                StatementKind::TSExportAssignment(x) => push!(export_assigns, *x),
                StatementKind::ImportDeclaration(x) => push!(import_decls, *x),
                StatementKind::TSImportEqualsDeclaration(x) => push!(import_equals, *x),
                StatementKind::ForStatement(x) => {
                    push!(for_heads, x.init.expect("a `for` head"));
                }
                StatementKind::ForInStatement(x) => push!(for_lefts, x.left),
                StatementKind::TryStatement(x) => {
                    push!(catches, x.handler.expect("a catch clause"));
                }
                other => panic!("an unexpected statement: {other:?}"),
            }
        }
        minted
    }

    #[test]
    fn a_park_rewinds_every_payload_store() {
        let bump = bumpalo::Bump::new();
        let parsed = parse_with_goal(PAYLOADS, Goal::Module, &bump).expect("the payloads parse");
        let mut stores = AstStores::new();
        let first = push_every_payload(&mut stores, &parsed);
        let mut pushed: Vec<&str> = first.iter().map(|&(field, _)| field).collect();
        pushed.sort_unstable();
        let mut every = PAYLOAD_FIELDS.to_vec();
        every.sort_unstable();
        assert_eq!(pushed, every, "one row in every payload store");
        assert!(first.iter().all(|&(_, index)| index == 1), "{first:?}");
        let second = push_every_payload(&mut stores, &parsed);
        assert!(second.iter().all(|&(_, index)| index == 2), "{second:?}");
        let mut stores: AstStores<'_> = stores.park();
        let parked = push_every_payload(&mut stores, &parsed);
        assert!(
            parked.iter().all(|&(_, index)| index == 1),
            "the rows rewind: {parked:?}"
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
