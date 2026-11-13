//! The stores a parse's expressions, statements, types, payloads, by-value list elements,
//! decoded strings and comments live in, and the per-thread parking of their capacity.
//!
//! Beside the three families, [`PayloadStores`] holds one store per payload too rare to sit
//! inline in the node that refers to it: the function, class and declaration payloads, the
//! constructor, import and `infer` types, qualified names, a `for` head, a `for … in` / `for …
//! of` left side, a catch clause and a parameter name's extras. One macro declares each such
//! store with its family tag, the id the tree holds and its view accessor.
//!
//! [`RunStores`] holds one store per element type of a list built by value: object
//! properties, template quasis, declarators, cases, import and export specifiers, class and
//! type members, type parameters, heritage clauses, enum members, index-signature parameters
//! and decorators. Each element type has a scratch stack that builds its lists, and a second
//! macro declares the store, the scratch, the list the tree holds and its view accessor.
//!
//! [`AstStores`] is one document's: every parse of a document, embedded islands included,
//! appends to the same one, so an id means the same row wherever in the document it is
//! read, and the first reading of an acorn re-parse stays valid beside the second. Its
//! scratch lists live here and not on the parser, because a parser is built per island.
//!
//! What outlives a parse is the rewound form: [`AstStores::reset`] rewinds the stores in
//! place, keeping each written store's slot 0 and every allocation's capacity, and never
//! trims. Handles die with the parse that minted them: nothing id-shaped leaves
//! [`with_stores`]'s closure or a [`Parsed`], whose stores it keeps beside the tree they
//! belong to.

use std::cell::Cell;
use std::fmt;
use std::ops::Deref;

use tsv_lang::store::{Id, ListBuf, ListMark, MarkGroup, PoolText, Row, Run, Store, TextPool};

use super::{
    ArrowFunctionExpression, AstView, CatchClause, ClassDeclaration, ClassExpression, ClassMember,
    Comment, Decorator, ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration,
    ExportSpecifier, Expression, ForInOfLeft, ForInit, FunctionDeclaration, FunctionExpression,
    Identifier, IdentifierParamExtra, ImportAttribute, ImportDeclaration, ImportSpecifier,
    MetaProperty, ObjectPatternProperty, ObjectProperty, Program, Statement, SwitchCase,
    TSConstructorType, TSDeclareFunction, TSEnumDeclaration, TSEnumMember, TSExportAssignment,
    TSImportEqualsDeclaration, TSImportType, TSInferType, TSInterfaceDeclaration,
    TSInterfaceHeritage, TSModuleDeclaration, TSQualifiedName, TSType, TSTypeAliasDeclaration,
    TSTypeElement, TSTypeParameter, TaggedTemplateExpression, TemplateElement, VariableDeclarator,
};

/// The family tag of [`Expression`] rows.
pub enum Expr {}

impl Row for Expression {
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

/// The family tag of [`Statement`] rows.
pub enum Stmt {}

impl Row for Statement {
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

/// The family tag of [`TSType`] rows.
pub enum Ty {}

impl Row for TSType {
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

/// The family tag of the tree's text pool: the decoded and normalized strings a span of the
/// source cannot express.
pub enum Text {}

/// One string of the tree's text — an escaped identifier's decoded name, a string literal's
/// or a template element's decoded value, a template element's normalized raw text — read
/// through [`AstView::text`]: a handle into the stores' text pool.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PoolStr {
    pub(super) text: PoolText<Text>,
}

impl PoolStr {
    /// The handle `text` names.
    #[inline]
    pub(crate) fn new(text: PoolText<Text>) -> Self {
        Self { text }
    }
}

impl fmt::Debug for PoolStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text.fmt(f)
    }
}

/// A program's comments, read through [`AstView::comments`]: a run of the stores' comment
/// rows.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct CommentList {
    pub(super) run: Run<Comment>,
}

impl CommentList {
    /// The list `run` names.
    #[inline]
    pub(crate) fn new(run: Run<Comment>) -> Self {
        Self { run }
    }
}

impl fmt::Debug for CommentList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.run.fmt(f)
    }
}

// A handle keeps its niche, so an absent string costs nothing beside a present one.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::STORE_CHECK;
    assert!(STORE_CHECK || size_of::<PoolStr>() == 8);
    assert!(STORE_CHECK || size_of::<Option<PoolStr>>() == 8);
    assert!(STORE_CHECK || size_of::<CommentList>() == 8);
    assert!(STORE_CHECK || size_of::<Option<CommentList>>() == 8);
};

/// Declares the payload stores. Per entry: the row type's family tag and the id the tree holds,
/// a [`PayloadStores`] field, that field's lines in `new` and `reset`, and the [`AstView`]
/// accessor that resolves one id. The entry's doc comment documents the field and the tag.
macro_rules! payload_stores {
    (
        $(
            $(#[$m:meta])*
            $field:ident : $Row:ident $(<$lt:lifetime>)?
                => $Tag:ident, $Id:ident, $acc:ident;
        )*
    ) => {
        $(
            $(#[$m])*
            ///
            /// The family tag of these rows.
            pub enum $Tag {}

            impl<$($lt)?> Row for $Row$(<$lt>)? {
                type Family = $Tag;
            }

            #[doc = concat!(
                "A handle to one [`", stringify!($Tag), "`] row, read through [`AstView::",
                stringify!($acc), "`]."
            )]
            pub type $Id = Id<$Tag>;
        )*

        /// The stores of the payloads too rare to sit inline in the node that refers to them,
        /// one per row type.
        pub(crate) struct PayloadStores {
            $(
                $(#[$m])*
                pub(crate) $field: Store<$Row$(<$lt>)?>,
            )*
        }

        impl PayloadStores {
            /// Empty stores, allocating nothing.
            pub(crate) fn new() -> Self {
                Self {
                    $($field: Store::new(),)*
                }
            }

            /// Rewinds the stores in place, as [`AstStores::reset`] does.
            #[inline]
            pub(crate) fn reset(&mut self) {
                $(self.$field.reset();)*
            }

            /// Every store's shape, for the tests that read what a reset leaves.
            #[cfg(test)]
            fn shapes(&self, out: &mut Vec<FieldShape>) {
                $(out.push(FieldShape::store(stringify!($field), &self.$field));)*
            }
        }

        impl<'a> AstView<'a> {
            $(
                #[doc = concat!(
                    "The [`", stringify!($Row), "`] `id` names. A miss ends in the store's shared ",
                    "cold path."
                )]
                #[inline]
                pub fn $acc(self, id: $Id) -> &'a $Row$(<$lt>)? {
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
    arrow_fns: ArrowFunctionExpression => ArrowFn, ArrowFnId, arrow_fn;
    /// Function expressions, `function () {}`: [`FunctionExpression`] rows.
    fn_exprs: FunctionExpression => FnExpr, FnExprId, fn_expr;
    /// Class expressions, `class {}`: [`ClassExpression`] rows.
    class_exprs: ClassExpression => ClassExpr, ClassExprId, class_expr;
    /// Tagged templates, ``tag`…` ``: [`TaggedTemplateExpression`] rows.
    tagged_tpls: TaggedTemplateExpression
        => TaggedTpl, TaggedTplId, tagged_tpl;
    /// Meta properties, `import.meta` and `new.target`: [`MetaProperty`] rows.
    meta_props: MetaProperty => MetaProp, MetaPropId, meta_prop;
    /// Type aliases, `type T = …`: [`TSTypeAliasDeclaration`] rows.
    type_aliases: TSTypeAliasDeclaration
        => TypeAlias, TypeAliasId, type_alias;
    /// Interfaces, `interface I {}`: [`TSInterfaceDeclaration`] rows.
    interface_decls: TSInterfaceDeclaration
        => InterfaceDecl, InterfaceDeclId, interface_decl;
    /// Bodiless function declarations, `declare function f(): T`: [`TSDeclareFunction`] rows.
    declare_fns: TSDeclareFunction => DeclareFn, DeclareFnId, declare_fn;
    /// Enums, `enum E {}`: [`TSEnumDeclaration`] rows.
    enum_decls: TSEnumDeclaration => EnumDecl, EnumDeclId, enum_decl;
    /// Namespaces and modules, `namespace N {}`: [`TSModuleDeclaration`] rows.
    module_decls: TSModuleDeclaration
        => ModuleDecl, ModuleDeclId, module_decl;
    /// Function declarations, `function f() {}`: [`FunctionDeclaration`] rows.
    fn_decls: FunctionDeclaration => FnDecl, FnDeclId, fn_decl;
    /// Class declarations, `class C {}`: [`ClassDeclaration`] rows.
    class_decls: ClassDeclaration => ClassDecl, ClassDeclId, class_decl;
    /// Named exports, `export { a }`: [`ExportNamedDeclaration`] rows.
    export_nameds: ExportNamedDeclaration
        => ExportNamed, ExportNamedId, export_named;
    /// Default exports, `export default …`: [`ExportDefaultDeclaration`] rows.
    export_defaults: ExportDefaultDeclaration
        => ExportDefault, ExportDefaultId, export_default;
    /// Star exports, `export * from 'm'`: [`ExportAllDeclaration`] rows.
    export_alls: ExportAllDeclaration => ExportAll, ExportAllId, export_all;
    /// Export assignments, `export = a`: [`TSExportAssignment`] rows.
    export_assigns: TSExportAssignment
        => ExportAssign, ExportAssignId, export_assign;
    /// Imports, `import a from 'm'`: [`ImportDeclaration`] rows.
    import_decls: ImportDeclaration
        => ImportDecl, ImportDeclId, import_decl;
    /// Import-equals declarations, `import a = require('m')`: [`TSImportEqualsDeclaration`] rows.
    import_equals: TSImportEqualsDeclaration
        => ImportEquals, ImportEqualsId, import_equals;
    /// Constructor types, `new () => T`: [`TSConstructorType`] rows.
    ctor_types: TSConstructorType => CtorType, CtorTypeId, ctor_type;
    /// Import types, `import('m')`: [`TSImportType`] rows.
    import_types: TSImportType => ImportType, ImportTypeId, import_type;
    /// Inferred types, `infer T`: [`TSInferType`] rows.
    infer_types: TSInferType => InferType, InferTypeId, infer_type;
    /// Qualified names, `A.B`: [`TSQualifiedName`] rows.
    qual_names: TSQualifiedName => QualName, QualNameId, qual_name;
    /// The head of a `for (…; …; …)` statement: [`ForInit`] rows.
    for_heads: ForInit => ForHead, ForHeadId, for_head;
    /// The left side of a `for … in` or `for … of`: [`ForInOfLeft`] rows.
    for_lefts: ForInOfLeft => ForLeft, ForLeftId, for_left;
    /// Catch clauses, `catch (e) {}`: [`CatchClause`] rows.
    catches: CatchClause => Catch, CatchId, catch;
    /// A parameter name's type annotation and decorators: [`IdentifierParamExtra`] rows.
    param_extras: IdentifierParamExtra
        => ParamExtra, ParamExtraId, param_extra;
}

/// Declares the value-run stores. Per entry: the element type's family tag, whose [`Run`] is
/// the list the tree holds, a [`RunStores`] store and the scratch stack that builds its lists,
/// their lines in `new` and `reset`, the scratch's mark in [`RunMarks`], and the [`AstView`]
/// accessor that reads one list. The entry's doc comment documents the store and the tag.
macro_rules! value_runs {
    (
        $(
            $(#[$m:meta])*
            $field:ident, $scratch:ident : $Row:ident $(<$lt:lifetime>)?
                => $Tag:ident, $acc:ident;
        )*
    ) => {
        $(
            $(#[$m])*
            ///
            /// The family tag of these rows.
            pub enum $Tag {}

            impl<$($lt)?> Row for $Row$(<$lt>)? {
                type Family = $Tag;
            }
        )*

        /// The stores of the elements of the lists built by value, one per element type, and
        /// the scratch stacks that build their lists.
        pub(crate) struct RunStores {
            $(
                $(#[$m])*
                pub(crate) $field: Store<$Row$(<$lt>)?>,
                #[doc = concat!("The scratch stack that builds the lists of `", stringify!($field), "`.")]
                pub(crate) $scratch: ListBuf<$Row$(<$lt>)?>,
            )*
        }

        /// The tops of the value-run scratch stacks, taken together: their part of a
        /// [`ScratchMarks`].
        #[derive(Clone, Copy)]
        pub(crate) struct RunMarks {
            $($field: ListMark<$Tag>,)*
        }

        impl RunStores {
            /// Empty stores, allocating nothing.
            pub(crate) fn new() -> Self {
                Self {
                    $(
                        $field: Store::new(),
                        $scratch: ListBuf::new(),
                    )*
                }
            }

            /// Rewinds the stores and scratch stacks in place, as [`AstStores::reset`] does.
            #[inline]
            pub(crate) fn reset(&mut self) {
                $(
                    self.$field.reset();
                    self.$scratch.reset();
                )*
            }

            /// Every store's and scratch stack's shape, for the tests that read what a reset
            /// leaves.
            #[cfg(test)]
            fn shapes(&self, out: &mut Vec<FieldShape>) {
                $(
                    out.push(FieldShape::store(stringify!($field), &self.$field));
                    out.push(FieldShape::scratch(stringify!($scratch), &self.$scratch));
                )*
            }

            /// The current tops of the scratch stacks, their heights left for `group` to check.
            #[inline]
            pub(crate) fn marks(&self, group: &mut MarkGroup) -> RunMarks {
                RunMarks {
                    $($field: self.$scratch.mark_in(group),)*
                }
            }

            /// Truncates the scratch stacks to `marks`, as [`AstStores::truncate_scratch`] does.
            #[inline]
            pub(crate) fn truncate(&mut self, marks: &RunMarks) {
                $(self.$scratch.truncate(marks.$field);)*
            }

            /// Whether the scratch stacks stand at `marks`.
            #[cfg(debug_assertions)]
            pub(crate) fn is_at(&self, marks: &RunMarks) -> bool {
                true $(&& self.$scratch.len_since(marks.$field) == 0)*
            }
        }

        impl<'a> AstView<'a> {
            $(
                #[doc = concat!(
                    "The [`", stringify!($Row), "`] rows of `run`, one range check for the whole ",
                    "run. A miss ends in the store's shared cold path."
                )]
                #[inline]
                pub fn $acc(self, run: Run<$Tag>) -> &'a [$Row$(<$lt>)?] {
                    self.runs().$field.slice(run)
                }
            )*
        }

        /// Every value-run store's field name, for the test that walks them all.
        #[cfg(test)]
        const RUN_FIELDS: &[&str] = &[$(stringify!($field)),*];
    };
}

value_runs! {
    /// Object literal members, `{ a: 1, ...b }`: [`ObjectProperty`] rows.
    obj_props, obj_prop_scratch: ObjectProperty => ObjProp, obj_props;
    /// Object pattern members, `{ a: b, ...c } = d`: [`ObjectPatternProperty`] rows.
    pat_props, pat_prop_scratch: ObjectPatternProperty => PatProp, pat_props;
    /// The text pieces of a template, ``a${b}c``: [`TemplateElement`] rows.
    quasis, quasi_scratch: TemplateElement => Quasi, quasis;
    /// Declarators, the `a = 1` of `let a = 1`: [`VariableDeclarator`] rows.
    declarators, declarator_scratch: VariableDeclarator => Declarator, declarators;
    /// The `case` and `default` clauses of a `switch`: [`SwitchCase`] rows.
    cases, case_scratch: SwitchCase => Case, cases;
    /// Import specifiers, the `a` and `{ b }` of `import a, { b } from 'm'`: [`ImportSpecifier`]
    /// rows.
    import_specs, import_spec_scratch: ImportSpecifier
        => ImportSpec, import_specs;
    /// Export specifiers, the `a` of `export { a }`: [`ExportSpecifier`] rows.
    export_specs, export_spec_scratch: ExportSpecifier
        => ExportSpec, export_specs;
    /// Import attributes, the `type: 'json'` of `with { type: 'json' }`: [`ImportAttribute`] rows.
    import_attrs, import_attr_scratch: ImportAttribute
        => ImportAttr, import_attrs;
    /// Class members, the `m() {}` of `class C { m() {} }`: [`ClassMember`] rows.
    class_members, class_member_scratch: ClassMember
        => ClassMem, class_members;
    /// Type members, the `a: T` of `{ a: T }` and of an interface body: [`TSTypeElement`] rows.
    type_members, type_member_scratch: TSTypeElement
        => TypeMember, type_members;
    /// Type parameters, the `T` of `<T>`: [`TSTypeParameter`] rows.
    type_params, type_param_scratch: TSTypeParameter
        => TypeParam, type_params;
    /// Heritage clauses, the `B` of `extends B` on an interface and of `implements B` on a
    /// class: [`TSInterfaceHeritage`] rows.
    heritages, heritage_scratch: TSInterfaceHeritage
        => Heritage, heritages;
    /// Enum members, the `A = 1` of `enum E { A = 1 }`: [`TSEnumMember`] rows.
    enum_members, enum_member_scratch: TSEnumMember
        => EnumMem, enum_members;
    /// Index-signature parameters, the `k: string` of `[k: string]: T`: [`Identifier`] rows.
    index_params, index_param_scratch: Identifier
        => IndexParam, index_params;
    /// Decorators, `@d`: [`Decorator`] rows.
    decorators, decorator_scratch: Decorator => Deco, decorators;
}

// Every value run is a `Run<F>`, which lays out alike for every family `F`, and an absent
// list (the decorators', an import's attributes) folds into the run's start.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::STORE_CHECK;
    assert!(STORE_CHECK || size_of::<Run<Deco>>() == 8);
    assert!(STORE_CHECK || size_of::<Option<Run<Deco>>>() == 8);
};

// Every payload id is an `Id<F>`, which lays out alike for every family `F`.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::STORE_CHECK;
    assert!(STORE_CHECK || size_of::<ParamExtraId>() == 4);
    assert!(STORE_CHECK || size_of::<Option<ParamExtraId>>() == 4);
};

/// One document's expression, statement, type, payload and value-run stores, the scratch
/// stacks that build their lists, its text pool and its comments.
///
/// A fresh one allocates nothing: a store's buffer is allocated by its first write, a
/// scratch stack's by its first push, and the pool's by its first string.
pub struct AstStores {
    pub(crate) exprs: Store<Expression>,
    pub(crate) expr_lists: Store<ExprId>,
    pub(crate) expr_opt_lists: Store<Option<ExprId>>,
    pub(crate) expr_scratch: ListBuf<Expression>,
    pub(crate) expr_list_scratch: ListBuf<ExprId>,
    pub(crate) expr_opt_scratch: ListBuf<Option<ExprId>>,
    pub(crate) stmts: Store<Statement>,
    pub(crate) stmt_lists: Store<StmtId>,
    pub(crate) stmt_list_scratch: ListBuf<StmtId>,
    pub(crate) tys: Store<TSType>,
    pub(crate) ty_lists: Store<TyId>,
    pub(crate) ty_list_scratch: ListBuf<TyId>,
    pub(crate) payloads: PayloadStores,
    pub(crate) runs: RunStores,
    /// The tree's decoded and normalized strings, read through [`PoolStr`].
    pub(crate) text: TextPool<Text>,
    /// Every comment of the document's programs, a program's read through its
    /// [`CommentList`].
    pub(crate) comments: Store<Comment>,
    /// The comments a parser has drained and not yet handed over: one frame per parser,
    /// opened when it is built and flushed into [`AstStores::comments`] when it takes them.
    pub(crate) comment_scratch: ListBuf<Comment>,
}

impl AstStores {
    /// The function declaration `id` names, borrowed from the stores rather than through a
    /// view: for the crate's own read of a row whose parts a host keeps
    /// ([`crate::__private::first_function_declaration`]).
    #[inline]
    pub(crate) fn fn_decl_row(&self, id: FnDeclId) -> &FunctionDeclaration {
        self.payloads.fn_decls.get(id)
    }

    /// A parameter name's extras, borrowed from the stores rather than through a view: for the
    /// crate's own read of a row it rebuilds while appending to the stores
    /// ([`crate::__private::attach_pattern_type_annotation`]).
    #[inline]
    pub(crate) fn param_extra_row(&self, id: ParamExtraId) -> &IdentifierParamExtra {
        self.payloads.param_extras.get(id)
    }
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
    runs: RunMarks,
    comments: ListMark<Comment>,
}

impl AstStores {
    /// The current tops of the scratch stacks, every height checked against the `u32` a mark
    /// holds by one [`MarkGroup`].
    #[inline]
    pub(crate) fn scratch_marks(&self) -> ScratchMarks {
        let mut group = MarkGroup::new();
        let marks = ScratchMarks {
            exprs: self.expr_scratch.mark_in(&mut group),
            lists: self.expr_list_scratch.mark_in(&mut group),
            opt_lists: self.expr_opt_scratch.mark_in(&mut group),
            stmt_lists: self.stmt_list_scratch.mark_in(&mut group),
            ty_lists: self.ty_list_scratch.mark_in(&mut group),
            runs: self.runs.marks(&mut group),
            comments: self.comment_scratch.mark_in(&mut group),
        };
        group.check();
        marks
    }

    /// Truncates the scratch stacks to `marks`, discarding the frames opened since. The
    /// stores keep the rows pushed since: an abandoned row is an orphan, and any id already
    /// handed out stays valid.
    #[inline]
    pub(crate) fn truncate_scratch(&mut self, marks: &ScratchMarks) {
        self.expr_scratch.truncate(marks.exprs);
        self.expr_list_scratch.truncate(marks.lists);
        self.expr_opt_scratch.truncate(marks.opt_lists);
        self.stmt_list_scratch.truncate(marks.stmt_lists);
        self.ty_list_scratch.truncate(marks.ty_lists);
        self.runs.truncate(&marks.runs);
        self.comment_scratch.truncate(marks.comments);
    }

    /// Truncates the comment scratch alone to `marks`: the comments a finished parser
    /// drained and never took are dropped with it.
    #[inline]
    pub(crate) fn truncate_comment_scratch(&mut self, marks: &ScratchMarks) {
        self.comment_scratch.truncate(marks.comments);
    }

    /// Whether the scratch stacks stand at `marks`.
    #[cfg(debug_assertions)]
    pub(crate) fn scratch_is_at(&self, marks: &ScratchMarks) -> bool {
        self.expr_scratch.len_since(marks.exprs) == 0
            && self.expr_list_scratch.len_since(marks.lists) == 0
            && self.expr_opt_scratch.len_since(marks.opt_lists) == 0
            && self.stmt_list_scratch.len_since(marks.stmt_lists) == 0
            && self.ty_list_scratch.len_since(marks.ty_lists) == 0
            && self.runs.is_at(&marks.runs)
            && self.comment_scratch.len_since(marks.comments) == 0
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
            runs: RunStores::new(),
            text: TextPool::new(),
            comments: Store::new(),
            comment_scratch: ListBuf::new(),
        }
    }

    /// Rewinds the stores in place between documents. Every store once written keeps its slot
    /// 0, so the next document's first write to it mints straight behind it; every store,
    /// scratch stack and the pool keep their capacity. Every id, run and pool text minted so
    /// far is invalid from here on.
    // TODO: never trims. Peak row capacity stays parked for as long as the `Formatter` or the
    // thread lives, where the AST arena's rewind releases every block but the largest while
    // idle. Rows held here instead of in the `Bump` escape that release.
    #[inline]
    pub fn reset(&mut self) {
        // destructured whole, so a field added to the stores without a reset here is a build
        // error rather than growth across documents
        let Self {
            exprs,
            expr_lists,
            expr_opt_lists,
            expr_scratch,
            expr_list_scratch,
            expr_opt_scratch,
            stmts,
            stmt_lists,
            stmt_list_scratch,
            tys,
            ty_lists,
            ty_list_scratch,
            payloads,
            runs,
            text,
            comments,
            comment_scratch,
        } = self;
        exprs.reset();
        expr_lists.reset();
        expr_opt_lists.reset();
        expr_scratch.reset();
        expr_list_scratch.reset();
        expr_opt_scratch.reset();
        stmts.reset();
        stmt_lists.reset();
        stmt_list_scratch.reset();
        tys.reset();
        ty_lists.reset();
        ty_list_scratch.reset();
        payloads.reset();
        runs.reset();
        text.reset();
        comments.reset();
        comment_scratch.reset();
    }

    /// Every store's, scratch stack's and the pool's shape, for the tests that read what a
    /// reset leaves. Destructured whole, as [`AstStores::reset`] is, so no field is missed.
    #[cfg(test)]
    fn shapes(&self) -> Vec<FieldShape> {
        let Self {
            exprs,
            expr_lists,
            expr_opt_lists,
            expr_scratch,
            expr_list_scratch,
            expr_opt_scratch,
            stmts,
            stmt_lists,
            stmt_list_scratch,
            tys,
            ty_lists,
            ty_list_scratch,
            payloads,
            runs,
            text,
            comments,
            comment_scratch,
        } = self;
        let mut out = vec![
            FieldShape::store("exprs", exprs),
            FieldShape::store("expr_lists", expr_lists),
            FieldShape::store("expr_opt_lists", expr_opt_lists),
            FieldShape::scratch("expr_scratch", expr_scratch),
            FieldShape::scratch("expr_list_scratch", expr_list_scratch),
            FieldShape::scratch("expr_opt_scratch", expr_opt_scratch),
            FieldShape::store("stmts", stmts),
            FieldShape::store("stmt_lists", stmt_lists),
            FieldShape::scratch("stmt_list_scratch", stmt_list_scratch),
            FieldShape::store("tys", tys),
            FieldShape::store("ty_lists", ty_lists),
            FieldShape::scratch("ty_list_scratch", ty_list_scratch),
            FieldShape::pool("text", text),
            FieldShape::store("comments", comments),
            FieldShape::scratch("comment_scratch", comment_scratch),
        ];
        payloads.shapes(&mut out);
        runs.shapes(&mut out);
        out
    }
}

/// What a test reads of one store, scratch stack or pool: its length and its capacity.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FieldShape {
    name: &'static str,
    kind: FieldKind,
    len: usize,
    capacity: usize,
}

/// Which kind of buffer a [`FieldShape`] reads, which decides its rewound length.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
    Store,
    Scratch,
    Pool,
}

#[cfg(test)]
impl FieldShape {
    fn store<T: Row>(name: &'static str, store: &Store<T>) -> Self {
        Self {
            name,
            kind: FieldKind::Store,
            len: store.len(),
            capacity: store.capacity(),
        }
    }

    fn scratch<T>(name: &'static str, scratch: &ListBuf<T>) -> Self {
        Self {
            name,
            kind: FieldKind::Scratch,
            len: scratch.len(),
            capacity: scratch.capacity(),
        }
    }

    fn pool<F>(name: &'static str, pool: &TextPool<F>) -> Self {
        Self {
            name,
            kind: FieldKind::Pool,
            len: pool.len(),
            capacity: pool.capacity(),
        }
    }

    /// The length a reset leaves: slot 0 of a store and the sentinel of a pool once written,
    /// nothing of a scratch stack.
    fn rewound_len(self) -> usize {
        match self.kind {
            FieldKind::Store | FieldKind::Pool => self.len.min(1),
            FieldKind::Scratch => 0,
        }
    }
}

impl Default for AstStores {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AstStores {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AstStores").finish_non_exhaustive()
    }
}

/// A parsed program and the stores its handles index.
///
/// Derefs to the [`Program`] for the tree's own fields. A reading entry point takes `&parsed`,
/// which is also where it gets the view the tree's edges resolve through.
#[derive(Debug)]
pub struct Parsed {
    /// The tree.
    pub program: Program,
    /// The stores the tree's handles index.
    pub stores: AstStores,
}

impl Deref for Parsed {
    type Target = Program;

    fn deref(&self) -> &Program {
        &self.program
    }
}

/// Run `f` over this thread's parked stores, or over fresh ones when there are none:
/// `tsv_arena`'s take-and-park protocol, here because that crate cannot name these types.
///
/// The stores are out of the slot while `f` runs, so a nested call takes fresh ones and a
/// panic out of `f` leaves the slot empty, as a new thread finds it. Parked after an
/// [`AstStores::reset`], keeping slot 0 and capacity, so every id, run and pool text `f`
/// minted is invalid once it returns.
pub fn with_stores<R>(f: impl FnOnce(&mut AstStores) -> R) -> R {
    thread_local! {
        static PARKED: Cell<Option<AstStores>> = const { Cell::new(None) };
    }
    let mut stores: AstStores = PARKED.with(Cell::take).unwrap_or_default();
    let result = f(&mut stores);
    stores.reset();
    PARKED.with(|cell| cell.set(Some(stores)));
    result
}

/// Stores a caller keeps warm across calls in a slot it owns, rewound between them: the
/// take-and-park protocol of [`with_stores`] over the caller's slot instead of the thread's, as
/// the `Formatter` keeps its stores.
///
/// A holder names no store type, so its code reads the same whatever the stores hold.
#[derive(Debug, Default)]
pub struct WarmStores {
    parked: Option<AstStores>,
}

impl WarmStores {
    /// Run `f` over the parked stores, or over fresh ones when there are none, and park them
    /// after an [`AstStores::reset`], keeping slot 0 and capacity, so every id, run and pool
    /// text `f` minted is invalid once it returns. A panic out of `f` leaves the slot empty,
    /// and the next call starts from fresh stores.
    pub fn with<R>(&mut self, f: impl FnOnce(&mut AstStores) -> R) -> R {
        let mut stores: AstStores = self.parked.take().unwrap_or_default();
        let result = f(&mut stores);
        stores.reset();
        self.parked = Some(stores);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::__private::parse_with_goal;
    use crate::Goal;
    use crate::ast::{
        EmptyStatement, ExpressionKind, Span, StatementKind, TSEntityName, TSThisType,
        TSTypeElement, ThisExpression,
    };
    use crate::parser::flush_run;

    const SPAN: Span = Span { start: 0, end: 0 };

    fn an_expr() -> Expression {
        Expression {
            span: SPAN,
            kind: ExpressionKind::ThisExpression(ThisExpression),
        }
    }

    fn a_stmt() -> Statement {
        Statement {
            span: SPAN,
            kind: StatementKind::EmptyStatement(EmptyStatement),
        }
    }

    fn a_ty() -> TSType {
        TSType::ThisType(TSThisType { span: SPAN })
    }

    fn with_a_row(stores: &mut AstStores) -> usize {
        stores.exprs.push(an_expr()).index()
    }

    #[test]
    fn a_reset_rewinds_the_expression_stores() {
        let mut stores = AstStores::new();
        let id = stores.exprs.push(an_expr());
        stores.exprs.push(an_expr());
        stores.expr_lists.push(id);
        stores.expr_lists.push(id);
        stores.expr_opt_lists.push(Some(id));
        stores.expr_opt_lists.push(None);
        stores.reset();
        assert_eq!(with_a_row(&mut stores), 1, "the rows rewind");
        let id = stores.exprs.push(an_expr());
        assert_eq!(stores.expr_lists.push(id).index(), 1, "the id lists rewind");
        assert_eq!(
            stores.expr_opt_lists.push(Some(id)).index(),
            1,
            "the optional id lists rewind"
        );
    }

    #[test]
    fn a_reset_rewinds_the_statement_stores() {
        let mut stores = AstStores::new();
        let id = stores.stmts.push(a_stmt());
        stores.stmts.push(a_stmt());
        stores.stmt_lists.push(id);
        stores.stmt_lists.push(id);
        stores.reset();
        let id = stores.stmts.push(a_stmt());
        assert_eq!(id.index(), 1, "the rows rewind");
        assert_eq!(stores.stmt_lists.push(id).index(), 1, "the id lists rewind");
    }

    #[test]
    fn a_reset_rewinds_the_type_stores() {
        let mut stores = AstStores::new();
        let id = stores.tys.push(a_ty());
        stores.tys.push(a_ty());
        stores.ty_lists.push(id);
        stores.ty_lists.push(id);
        stores.reset();
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
    fn push_every_payload(stores: &mut AstStores, parsed: &Parsed) -> Vec<(&'static str, usize)> {
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
                    &ExpressionKind::ArrowFunctionExpression(x) => {
                        push!(arrow_fns, *view.arrow_fn(x));
                    }
                    &ExpressionKind::FunctionExpression(x) => push!(fn_exprs, *view.fn_expr(x)),
                    &ExpressionKind::ClassExpression(x) => push!(class_exprs, *view.class_expr(x)),
                    &ExpressionKind::TaggedTemplateExpression(x) => {
                        push!(tagged_tpls, *view.tagged_tpl(x));
                    }
                    &ExpressionKind::MetaProperty(x) => push!(meta_props, *view.meta_prop(x)),
                    other => panic!("an unexpected expression: {other:?}"),
                },
                &StatementKind::TSTypeAliasDeclaration(x) => {
                    let x = view.type_alias(x);
                    if !aliased {
                        push!(type_aliases, *x);
                        aliased = true;
                    }
                    match view.ty(x.type_annotation) {
                        &TSType::Constructor(_, t) => push!(ctor_types, *view.ctor_type(t)),
                        &TSType::Import(_, t) => push!(import_types, *view.import_type(t)),
                        TSType::Conditional(c) => match view.ty(c.extends_type) {
                            &TSType::Infer(_, t) => push!(infer_types, *view.infer_type(t)),
                            other => panic!("an unexpected extends type: {other:?}"),
                        },
                        TSType::TypeReference(r) => match &r.type_name {
                            &TSEntityName::QualifiedName(_, q) => {
                                push!(qual_names, *view.qual_name(q));
                            }
                            other => panic!("an unexpected type name: {other:?}"),
                        },
                        other => panic!("an unexpected type: {other:?}"),
                    }
                }
                &StatementKind::TSInterfaceDeclaration(x) => {
                    push!(interface_decls, *view.interface_decl(x));
                }
                &StatementKind::TSDeclareFunction(x) => push!(declare_fns, *view.declare_fn(x)),
                &StatementKind::TSEnumDeclaration(x) => push!(enum_decls, *view.enum_decl(x)),
                &StatementKind::TSModuleDeclaration(x) => push!(module_decls, *view.module_decl(x)),
                &StatementKind::FunctionDeclaration(x) => {
                    let x = view.fn_decl(x);
                    push!(fn_decls, *x);
                    for param in view.run(x.params) {
                        if let ExpressionKind::Identifier(name) = &param.kind
                            && let Some(extra) = view.opt_param_extra(name.extra)
                        {
                            push!(param_extras, *extra);
                        }
                    }
                }
                &StatementKind::ClassDeclaration(x) => push!(class_decls, *view.class_decl(x)),
                &StatementKind::ExportNamedDeclaration(x) => {
                    push!(export_nameds, *view.export_named(x));
                }
                &StatementKind::ExportDefaultDeclaration(x) => {
                    push!(export_defaults, *view.export_default(x));
                }
                &StatementKind::ExportAllDeclaration(x) => push!(export_alls, *view.export_all(x)),
                &StatementKind::TSExportAssignment(x) => {
                    push!(export_assigns, *view.export_assign(x));
                }
                &StatementKind::ImportDeclaration(x) => push!(import_decls, *view.import_decl(x)),
                &StatementKind::TSImportEqualsDeclaration(x) => {
                    push!(import_equals, *view.import_equals(x));
                }
                StatementKind::ForStatement(x) => {
                    push!(for_heads, *view.opt_for_head(x.init).expect("a `for` head"));
                }
                StatementKind::ForInStatement(x) => push!(for_lefts, *view.for_left(x.left)),
                StatementKind::TryStatement(x) => {
                    push!(catches, *view.opt_catch(x.handler).expect("a catch clause"));
                }
                other => panic!("an unexpected statement: {other:?}"),
            }
        }
        minted
    }

    #[test]
    fn a_reset_rewinds_every_payload_store() {
        let parsed = parse_with_goal(PAYLOADS, Goal::Module).expect("the payloads parse");
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
        stores.reset();
        let parked = push_every_payload(&mut stores, &parsed);
        assert!(
            parked.iter().all(|&(_, index)| index == 1),
            "the rows rewind: {parked:?}"
        );
    }

    /// One list of every value-run element type, at module goal.
    const RUNS: &str = "x = { a: 1 };
let { b } = c;
`d`;
switch (e) {
	case 1:
}
import f from 'm' with { type: 'json' };
export { g };
interface I extends J {
	p: 1;
}
type K<T> = { [k: string]: T };
enum E {
	A
}
@d
class C {
	m() {}
}
";

    /// Flushes the first element of every value-run list `parsed` holds, as a one-row run,
    /// into its store in `stores`, and returns each store's field name with the run it minted.
    fn flush_every_run(stores: &mut AstStores, parsed: &Parsed) -> Vec<(&'static str, String)> {
        push_every_run(stores, parsed, true)
    }

    /// Pushes the first element of every value-run list `parsed` holds onto its scratch stack
    /// in `stores`, flushing it as a one-row run into its store when `flush` is set and
    /// leaving it on the stack otherwise; returns each store's field name with the run it
    /// minted, or `open`.
    fn push_every_run(
        stores: &mut AstStores,
        parsed: &Parsed,
        flush: bool,
    ) -> Vec<(&'static str, String)> {
        let mut minted = Vec::new();
        macro_rules! flush {
            ($field:ident, $scratch:ident, $list:expr) => {{
                let row = $list[0].clone();
                let mark = stores.runs.$scratch.mark();
                stores.runs.$scratch.push(row);
                if flush {
                    let run = flush_run(&mut stores.runs.$scratch, &mut stores.runs.$field, mark);
                    minted.push((stringify!($field), format!("{run:?}")));
                } else {
                    minted.push((stringify!($field), "open".to_owned()));
                }
            }};
        }
        let view = parsed.view();
        for &id in view.stmt_list(parsed.program.body) {
            match &view.stmt(id).kind {
                StatementKind::ExpressionStatement(s) => match &view.expr(s.expression).kind {
                    ExpressionKind::AssignmentExpression(a) => match &view.expr(a.right).kind {
                        ExpressionKind::ObjectExpression(o) => {
                            flush!(obj_props, obj_prop_scratch, view.obj_props(o.properties));
                        }
                        other => panic!("an unexpected value: {other:?}"),
                    },
                    ExpressionKind::TemplateLiteral(t) => {
                        flush!(quasis, quasi_scratch, view.quasis(t.quasis));
                    }
                    other => panic!("an unexpected expression: {other:?}"),
                },
                StatementKind::VariableDeclaration(v) => {
                    let declarators = view.declarators(v.declarations);
                    flush!(declarators, declarator_scratch, declarators);
                    match &view.expr(declarators[0].id).kind {
                        ExpressionKind::ObjectPattern(p) => {
                            flush!(pat_props, pat_prop_scratch, view.pat_props(p.properties));
                        }
                        other => panic!("an unexpected binding: {other:?}"),
                    }
                }
                StatementKind::SwitchStatement(s) => {
                    flush!(cases, case_scratch, view.cases(s.cases));
                }
                &StatementKind::ImportDeclaration(x) => {
                    let x = view.import_decl(x);
                    flush!(
                        import_specs,
                        import_spec_scratch,
                        view.import_specs(x.specifiers)
                    );
                    let attributes = x.attributes.expect("import attributes");
                    flush!(
                        import_attrs,
                        import_attr_scratch,
                        view.import_attrs(attributes)
                    );
                }
                &StatementKind::ExportNamedDeclaration(x) => {
                    let specifiers = view.export_named(x).specifiers;
                    flush!(
                        export_specs,
                        export_spec_scratch,
                        view.export_specs(specifiers)
                    );
                }
                &StatementKind::TSInterfaceDeclaration(x) => {
                    let x = view.interface_decl(x);
                    flush!(heritages, heritage_scratch, view.heritages(x.extends));
                    flush!(
                        type_members,
                        type_member_scratch,
                        view.type_members(x.body.body)
                    );
                }
                &StatementKind::TSTypeAliasDeclaration(x) => {
                    let x = view.type_alias(x);
                    let params = x.type_parameters.as_ref().expect("type parameters").params;
                    flush!(type_params, type_param_scratch, view.type_params(params));
                    let TSType::TypeLiteral(literal) = view.ty(x.type_annotation) else {
                        panic!("an unexpected type: {:?}", view.ty(x.type_annotation));
                    };
                    match &view.type_members(literal.members)[0] {
                        TSTypeElement::IndexSignature(i) => {
                            flush!(
                                index_params,
                                index_param_scratch,
                                view.index_params(i.parameters)
                            );
                        }
                        other => panic!("an unexpected member: {other:?}"),
                    }
                }
                &StatementKind::TSEnumDeclaration(x) => {
                    let members = view.enum_decl(x).members;
                    flush!(
                        enum_members,
                        enum_member_scratch,
                        view.enum_members(members)
                    );
                }
                &StatementKind::ClassDeclaration(x) => {
                    let x = view.class_decl(x);
                    let decorators = x.decorators.expect("decorators");
                    flush!(decorators, decorator_scratch, view.decorators(decorators));
                    flush!(
                        class_members,
                        class_member_scratch,
                        view.class_members(x.body.body)
                    );
                }
                other => panic!("an unexpected statement: {other:?}"),
            }
        }
        minted
    }

    #[test]
    fn a_reset_rewinds_every_value_run_store() {
        let parsed = parse_with_goal(RUNS, Goal::Module).expect("the lists parse");
        let mut stores = AstStores::new();
        let first = flush_every_run(&mut stores, &parsed);
        let mut flushed: Vec<&str> = first.iter().map(|&(field, _)| field).collect();
        flushed.sort_unstable();
        let mut every = RUN_FIELDS.to_vec();
        every.sort_unstable();
        assert_eq!(flushed, every, "one run in every value-run store");
        assert!(first.iter().all(|(_, run)| run == "Run(1, 1)"), "{first:?}");
        let second = flush_every_run(&mut stores, &parsed);
        assert!(
            second.iter().all(|(_, run)| run == "Run(2, 1)"),
            "{second:?}"
        );
        stores.reset();
        let parked = flush_every_run(&mut stores, &parsed);
        assert!(
            parked.iter().all(|(_, run)| run == "Run(1, 1)"),
            "the rows rewind: {parked:?}"
        );
    }

    /// A document that writes every store the parse fills on its own: the expression,
    /// statement and type stores and their lists (a union for the type lists), the optional
    /// lists (an array hole), the pool (an escaped name and a decoded string) and the comments.
    const DOCUMENT: &str = "// a comment
const \\u0061b = [, 1, 'c\\x41'];
let d: { e: F } = g(h, (i) => i + 1);
type U = V | W;
";

    /// Writes every store, scratch stack and the pool of `stores`: a sizeable parse, a row in
    /// every payload store and a run in every value-run store, then one element left on every
    /// scratch stack, as a parse cut short would leave it.
    fn fill_every_field(stores: &mut AstStores) {
        let payloads = parse_with_goal(PAYLOADS, Goal::Module).expect("the payloads parse");
        let runs = parse_with_goal(RUNS, Goal::Module).expect("the lists parse");
        let document = DOCUMENT.repeat(64);
        let program = crate::__private::parse_into(&document, stores).expect("parses");
        push_every_payload(stores, &payloads);
        flush_every_run(stores, &runs);
        push_every_run(stores, &runs, false);
        let comment = stores.view().comments(program.comments)[0];
        stores.comment_scratch.push(comment);
        stores.expr_scratch.push(an_expr());
        let expr = stores.exprs.push(an_expr());
        stores.expr_list_scratch.push(expr);
        stores.expr_opt_scratch.push(None);
        let stmt = stores.stmts.push(a_stmt());
        stores.stmt_list_scratch.push(stmt);
        let ty = stores.tys.push(a_ty());
        stores.ty_list_scratch.push(ty);
    }

    /// Asserts `filled` wrote every field, so a field nothing fills cannot pass a rewind test
    /// by standing empty: a store past slot 0, a scratch stack holding an element, the pool
    /// past its sentinel.
    fn assert_every_field_written(filled: &[FieldShape]) {
        let unwritten: Vec<_> = filled
            .iter()
            .filter(|shape| match shape.kind {
                FieldKind::Store | FieldKind::Pool => shape.len <= 1,
                FieldKind::Scratch => shape.len == 0,
            })
            .map(|shape| shape.name)
            .collect();
        assert!(
            unwritten.is_empty(),
            "the fill leaves these unwritten: {unwritten:?}"
        );
    }

    /// Asserts `rewound` is `filled` after a reset: every field back at its rewound length,
    /// slot 0 and the sentinel kept, and no capacity shrunk or grown.
    fn assert_rewound(filled: &[FieldShape], rewound: &[FieldShape]) {
        let expected: Vec<FieldShape> = filled
            .iter()
            .map(|&shape| FieldShape {
                len: shape.rewound_len(),
                ..shape
            })
            .collect();
        assert_eq!(
            rewound, expected,
            "every field rewinds and keeps its capacity"
        );
    }

    #[test]
    fn a_reset_rewinds_every_field() {
        let mut stores = AstStores::new();
        fill_every_field(&mut stores);
        let filled = stores.shapes();
        assert_every_field_written(&filled);
        stores.reset();
        assert_rewound(&filled, &stores.shapes());
    }

    #[test]
    fn park_keeps_capacity() {
        let mut stores = AstStores::new();
        fill_every_field(&mut stores);
        let filled = stores.shapes();
        assert_every_field_written(&filled);
        stores.reset();
        let rewound = stores.shapes();
        assert_rewound(&filled, &rewound);
        // slot 0 survives, so a store's next write mints at index 1 with no priming
        assert_eq!(with_a_row(&mut stores), 1);
        assert_eq!(
            stores.exprs.len(),
            2,
            "slot 0 and the one row, nothing primed"
        );
        assert_eq!(stores.exprs.capacity(), rewound[0].capacity);
    }

    #[test]
    fn a_warm_handle_parks_its_stores_with_their_capacity() {
        let mut warm = WarmStores::default();
        let filled = warm.with(|stores| {
            fill_every_field(stores);
            stores.shapes()
        });
        assert_every_field_written(&filled);
        let rewound = warm.with(|stores| stores.shapes());
        assert_rewound(&filled, &rewound);
    }

    #[test]
    fn with_stores_parks_its_stores_with_their_capacity() {
        let filled = with_stores(|stores: &mut AstStores| {
            fill_every_field(stores);
            stores.shapes()
        });
        assert_every_field_written(&filled);
        let rewound = with_stores(|stores: &mut AstStores| stores.shapes());
        assert_rewound(&filled, &rewound);
    }

    /// Whether `read` is refused as the store-check build refuses a handle from another
    /// generation; an unstamped build reads it in bounds instead.
    fn assert_rejected_under_the_store_check(read: impl FnOnce()) {
        let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read));
        if tsv_lang::store::STORE_CHECK {
            let message = read.expect_err("a retained handle is rejected");
            let message = message
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| message.downcast_ref::<&str>().copied())
                .expect("a panic message");
            assert!(message.contains("store stamp mismatch"), "{message}");
        } else {
            assert!(read.is_ok(), "unstamped, a retained handle reads in bounds");
        }
    }

    /// An id or a pool text kept past the park reads the next document's rows or text in
    /// bounds; the store-check build rejects both, because the reset advances every store's
    /// and the pool's generation.
    #[test]
    fn a_handle_retained_across_a_park_is_rejected_under_the_store_check() {
        // one warm handle per kind: a rejected read panics out of `with`, which leaves the
        // slot empty, and fresh stores would be refused as another instance
        let mut warm = WarmStores::default();
        let id = warm.with(|stores| {
            with_a_row(stores);
            stores.exprs.push(an_expr())
        });
        assert_rejected_under_the_store_check(|| {
            warm.with(|stores| {
                with_a_row(stores);
                with_a_row(stores);
                stores.exprs.get(id);
            });
        });
        let mut warm = WarmStores::default();
        let text = warm.with(|stores| {
            stores.text.push("a");
            stores.text.push("b")
        });
        assert_rejected_under_the_store_check(|| {
            warm.with(|stores| {
                stores.text.push("cd");
                stores.text.get(text);
            });
        });
    }

    #[test]
    fn with_stores_hands_out_rewound_stores() {
        with_stores(|stores: &mut AstStores| {
            with_a_row(stores);
            with_a_row(stores);
        });
        with_stores(|stores: &mut AstStores| {
            assert_eq!(with_a_row(stores), 1);
        });
    }

    #[test]
    fn with_stores_recovers_after_a_caught_panic() {
        let caught = std::panic::catch_unwind(|| {
            with_stores(|_: &mut AstStores| panic!("boom"));
        });
        assert!(caught.is_err());
        with_stores(|stores: &mut AstStores| {
            assert_eq!(with_a_row(stores), 1);
        });
    }

    #[test]
    fn a_warm_handle_hands_its_parse_rewound_stores() {
        let source = "const a = [b, c => d];\nfunction f(x: T) { return x; }\n";
        let mut warm = WarmStores::default();
        warm.with(|stores| {
            crate::__private::parse_into(source, stores).expect("parses");
            assert!(with_a_row(stores) > 1, "the parse filled the stores");
        });
        warm.with(|stores| {
            assert_eq!(with_a_row(stores), 1, "the park rewinds");
        });
        warm.with(|stores| {
            crate::__private::parse_into(source, stores).expect("parses again");
        });
    }

    #[test]
    fn a_warm_handle_recovers_after_a_caught_panic() {
        let mut warm = WarmStores::default();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            warm.with(|stores| {
                with_a_row(stores);
                panic!("boom");
            });
        }));
        assert!(caught.is_err());
        warm.with(|stores| {
            assert_eq!(with_a_row(stores), 1);
        });
    }

    #[test]
    fn a_nested_with_stores_takes_fresh_stores() {
        with_stores(|outer: &mut AstStores| {
            with_a_row(outer);
            with_stores(|inner: &mut AstStores| {
                assert_eq!(with_a_row(inner), 1, "not the stores already in use");
            });
            assert_eq!(with_a_row(outer), 2, "the outer stores are untouched");
        });
    }
}
