//! The stores a parse's expressions, statements, types, payloads, by-value list elements,
//! decoded strings and comments live in, and the per-thread parking of their capacity.
//!
//! Beside the three families, [`PayloadStores`] holds one store per payload too rare to sit
//! inline in the node that refers to it: the function, class and declaration payloads, the
//! constructor, import and `infer` types, qualified names, a `for` head, a `for … in` /
//! `for … of` left side, a `try` statement and its catch clause, a labeled statement, a
//! binding's extras, a call's type arguments, a comment-bound pair and an escaped name's
//! text. One macro declares each such store with
//! the id the tree holds and its view accessor.
//!
//! [`RunStores`] holds one store per element type of a list built by value: object
//! properties, template quasis, declarators, cases, import and export specifiers, class and
//! type members, type parameters, heritage clauses, enum members, index-signature parameters
//! and decorators. Each element type has a scratch stack that builds its lists, and
//! `tsv_lang`'s `value_runs!` declares the store, the scratch, the list the tree holds and its
//! view accessor from one entry.
//! Every row type, and what each holds inline, is `Copy`, so a finished list is copied into
//! its store as a block and a store's first write copies its first row into slot 0. A row is
//! read in place, through the reference or slice its accessor returns: `*view.expr(id)` or
//! `rows[i]` by value is a copy of the whole row. A copy also has an address of its own, and
//! the printer's memos key a node by its address in its store (`Printer::chain_share_key_of`,
//! `Printer::head_stripped_paren_shell`), so they are handed a reference into the store — one
//! an accessor returned, or a field of it — never a copy, whose stack slot another node can
//! reuse.
//!
//! [`AstStores`] is one document's: every parse of a document, embedded islands included,
//! appends to the same one, so an id means the same row wherever in the document it is
//! read, and the first reading of an acorn re-parse stays valid beside the second. Its
//! scratch lists live here and not on the parser, because a parser is built per island.
//! Between parses every scratch stack is empty: the parse driver leaves them so, whether the
//! parse succeeded or failed, which is what lets the next parse begin without marking them.
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
#[cfg(test)]
use tsv_lang::value_runs::{FieldKind, FieldShape};

use super::{
    ArrowFunctionExpression, AstView, BindingExtra, CatchClause, ClassDeclaration, ClassExpression,
    ClassMember, Comment, CommentBoundPair, Decorator, EscapedName, ExportAllDeclaration,
    ExportDefaultDeclaration, ExportNamedDeclaration, ExportSpecifier, Expression, ForInOfLeft,
    ForInit, FunctionDeclaration, FunctionExpression, Identifier, ImportAttribute,
    ImportDeclaration, ImportSpecifier, LabeledStatement, MetaProperty, ObjectPatternProperty,
    ObjectProperty, Program, Statement, SwitchCase, TSConstructorType, TSDeclareFunction,
    TSEnumDeclaration, TSEnumMember, TSExportAssignment, TSFunctionType, TSImportEqualsDeclaration,
    TSImportType, TSInferType, TSInterfaceDeclaration, TSInterfaceHeritage, TSModuleDeclaration,
    TSQualifiedName, TSType, TSTypeAliasDeclaration, TSTypeAnnotation, TSTypeElement,
    TSTypeParameter, TSTypeParameterInstantiation, TaggedTemplateExpression, TemplateElement,
    TryStatement, VariableDeclarator,
};

impl Row for Expression {
    type Family = Self;
}

/// A handle to one [`Expression`] row.
pub type ExprId = Id<Expression>;

#[cfg(test)]
const _: () = {
    use tsv_lang::store::{Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<ExprId>() == 4);
    assert!(STORE_CHECK || size_of::<Option<ExprId>>() == 4);
    assert!(STORE_CHECK || size_of::<Run<ExprId>>() == 8);
};

impl Row for Statement {
    type Family = Self;
}

/// A handle to one [`Statement`] row.
pub type StmtId = Id<Statement>;

#[cfg(test)]
const _: () = {
    use tsv_lang::store::{Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<StmtId>() == 4);
    assert!(STORE_CHECK || size_of::<Option<StmtId>>() == 4);
    assert!(STORE_CHECK || size_of::<Run<StmtId>>() == 8);
};

impl Row for TSType {
    type Family = Self;
}

/// A handle to one [`TSType`] row.
pub type TyId = Id<TSType>;

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

// A handle keeps its niche, so an absent string costs nothing beside a present one.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::{PoolText, Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<PoolText<Text>>() == 8);
    assert!(STORE_CHECK || size_of::<Option<PoolText<Text>>>() == 8);
    assert!(STORE_CHECK || size_of::<Run<Comment>>() == 8);
    assert!(STORE_CHECK || size_of::<Option<Run<Comment>>>() == 8);
};

/// Declares the payload stores. Per entry: the row type as its own family and the id the tree
/// holds, a [`PayloadStores`] field, that field's lines in `new` and `reset`, and the
/// [`AstView`] accessor that resolves one id. The entry's doc comment documents the field.
macro_rules! payload_stores {
    (
        $(
            $(#[$m:meta])*
            $field:ident : $Row:ident => $Id:ident, $acc:ident;
        )*
    ) => {
        $(
            impl Row for $Row {
                type Family = Self;
            }

            #[doc = concat!(
                "A handle to one [`", stringify!($Row), "`] row, read through [`AstView::",
                stringify!($acc), "`]."
            )]
            pub type $Id = Id<$Row>;
        )*

        /// The stores of the payloads too rare to sit inline in the node that refers to them,
        /// one per row type.
        pub(crate) struct PayloadStores {
            $(
                $(#[$m])*
                pub(crate) $field: Store<$Row>,
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
                pub fn $acc(self, id: $Id) -> &'a $Row {
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
    arrow_fns: ArrowFunctionExpression => ArrowFnId, arrow_fn;
    /// Function expressions, `function () {}`: [`FunctionExpression`] rows.
    fn_exprs: FunctionExpression => FnExprId, fn_expr;
    /// Class expressions, `class {}`: [`ClassExpression`] rows.
    class_exprs: ClassExpression => ClassExprId, class_expr;
    /// Tagged templates, ``tag`…` ``: [`TaggedTemplateExpression`] rows.
    tagged_tpls: TaggedTemplateExpression => TaggedTplId, tagged_tpl;
    /// Meta properties, `import.meta` and `new.target`: [`MetaProperty`] rows.
    meta_props: MetaProperty => MetaPropId, meta_prop;
    /// Type aliases, `type T = …`: [`TSTypeAliasDeclaration`] rows.
    type_aliases: TSTypeAliasDeclaration => TypeAliasId, type_alias;
    /// Interfaces, `interface I {}`: [`TSInterfaceDeclaration`] rows.
    interface_decls: TSInterfaceDeclaration => InterfaceDeclId, interface_decl;
    /// Bodiless function declarations, `declare function f(): T`: [`TSDeclareFunction`] rows.
    declare_fns: TSDeclareFunction => DeclareFnId, declare_fn;
    /// Enums, `enum E {}`: [`TSEnumDeclaration`] rows.
    enum_decls: TSEnumDeclaration => EnumDeclId, enum_decl;
    /// Namespaces and modules, `namespace N {}`: [`TSModuleDeclaration`] rows.
    module_decls: TSModuleDeclaration => ModuleDeclId, module_decl;
    /// Function declarations, `function f() {}`: [`FunctionDeclaration`] rows.
    fn_decls: FunctionDeclaration => FnDeclId, fn_decl;
    /// Class declarations, `class C {}`: [`ClassDeclaration`] rows.
    class_decls: ClassDeclaration => ClassDeclId, class_decl;
    /// Named exports, `export { a }`: [`ExportNamedDeclaration`] rows.
    export_nameds: ExportNamedDeclaration => ExportNamedId, export_named;
    /// Default exports, `export default …`: [`ExportDefaultDeclaration`] rows.
    export_defaults: ExportDefaultDeclaration => ExportDefaultId, export_default;
    /// Star exports, `export * from 'm'`: [`ExportAllDeclaration`] rows.
    export_alls: ExportAllDeclaration => ExportAllId, export_all;
    /// Export assignments, `export = a`: [`TSExportAssignment`] rows.
    export_assigns: TSExportAssignment => ExportAssignId, export_assign;
    /// Imports, `import a from 'm'`: [`ImportDeclaration`] rows.
    import_decls: ImportDeclaration => ImportDeclId, import_decl;
    /// Import-equals declarations, `import a = require('m')`: [`TSImportEqualsDeclaration`] rows.
    import_equals: TSImportEqualsDeclaration => ImportEqualsId, import_equals;
    /// Function types, `(x: T) => U`: [`TSFunctionType`] rows.
    fn_types: TSFunctionType => FnTypeId, fn_type;
    /// Constructor types, `new () => T`: [`TSConstructorType`] rows.
    ctor_types: TSConstructorType => CtorTypeId, ctor_type;
    /// Import types, `import('m')`: [`TSImportType`] rows.
    import_types: TSImportType => ImportTypeId, import_type;
    /// Inferred types, `infer T`: [`TSInferType`] rows.
    infer_types: TSInferType => InferTypeId, infer_type;
    /// Qualified names, `A.B`: [`TSQualifiedName`] rows.
    qual_names: TSQualifiedName => QualNameId, qual_name;
    /// The head of a `for (…; …; …)` statement: [`ForInit`] rows.
    for_heads: ForInit => ForHeadId, for_head;
    /// The left side of a `for … in` or `for … of`: [`ForInOfLeft`] rows.
    for_lefts: ForInOfLeft => ForLeftId, for_left;
    /// `try` statements, `try {} catch {}`: [`TryStatement`] rows.
    try_stmts: TryStatement => TryStmtId, try_stmt;
    /// Catch clauses, `catch (e) {}`: [`CatchClause`] rows.
    catches: CatchClause => CatchId, catch;
    /// Labeled statements, `l: for (;;) {}`: [`LabeledStatement`] rows.
    labeled_stmts: LabeledStatement => LabeledStmtId, labeled_stmt;
    /// A binding's type annotation and decorators — a parameter name's, a binding pattern's
    /// or a rest element's: [`BindingExtra`] rows.
    binding_extras: BindingExtra => BindingExtraId, binding_extra;
    /// A call's or `new`'s type arguments, the `<T>` of `f<T>()`:
    /// [`TSTypeParameterInstantiation`] rows.
    type_args: TSTypeParameterInstantiation => TypeArgsId, type_args;
    /// Comment-bound grouping pairs, `/** @type {T} */ (x)`: [`CommentBoundPair`] rows.
    comment_pairs: CommentBoundPair => CommentPairId, comment_pair;
    /// The text of a name its span cannot spell, `\u0061`: [`EscapedName`] rows.
    escaped_names: EscapedName => EscapedNameId, escaped_name;
}

tsv_lang::value_runs! {
    /// The stores of the elements of the lists built by value, one per element type, and the
    /// scratch stacks that build their lists.
    pub(crate) struct RunStores;
    /// The tops of the value-run scratch stacks, taken together: their part of a
    /// [`ScratchMarks`].
    pub(crate) struct RunMarks;
    pub view AstView::runs;

    /// Object literal members, `{ a: 1, ...b }`: [`ObjectProperty`] rows.
    obj_props, obj_prop_scratch: ObjectProperty => obj_props;
    /// Object pattern members, `{ a: b, ...c } = d`: [`ObjectPatternProperty`] rows.
    pat_props, pat_prop_scratch: ObjectPatternProperty => pat_props;
    /// The text pieces of a template, ``a${b}c``: [`TemplateElement`] rows.
    quasis, quasi_scratch: TemplateElement => quasis;
    /// Declarators, the `a = 1` of `let a = 1`: [`VariableDeclarator`] rows.
    declarators, declarator_scratch: VariableDeclarator => declarators;
    /// The `case` and `default` clauses of a `switch`: [`SwitchCase`] rows.
    cases, case_scratch: SwitchCase => cases;
    /// Import specifiers, the `a` and `{ b }` of `import a, { b } from 'm'`: [`ImportSpecifier`]
    /// rows.
    import_specs, import_spec_scratch: ImportSpecifier => import_specs;
    /// Export specifiers, the `a` of `export { a }`: [`ExportSpecifier`] rows.
    export_specs, export_spec_scratch: ExportSpecifier => export_specs;
    /// Import attributes, the `type: 'json'` of `with { type: 'json' }`: [`ImportAttribute`] rows.
    import_attrs, import_attr_scratch: ImportAttribute => import_attrs;
    /// Class members, the `m() {}` of `class C { m() {} }`: [`ClassMember`] rows.
    class_members, class_member_scratch: ClassMember => class_members;
    /// Type members, the `a: T` of `{ a: T }` and of an interface body: [`TSTypeElement`] rows.
    type_members, type_member_scratch: TSTypeElement => type_members;
    /// Type parameters, the `T` of `<T>`: [`TSTypeParameter`] rows.
    type_params, type_param_scratch: TSTypeParameter => type_params;
    /// Heritage clauses, the `B` of `extends B` on an interface and of `implements B` on a
    /// class: [`TSInterfaceHeritage`] rows.
    heritages, heritage_scratch: TSInterfaceHeritage => heritages;
    /// Enum members, the `A = 1` of `enum E { A = 1 }`: [`TSEnumMember`] rows.
    enum_members, enum_member_scratch: TSEnumMember => enum_members;
    /// Index-signature parameters, the `k: string` of `[k: string]: T`: [`Identifier`] rows.
    index_params, index_param_scratch: Identifier => index_params;
    /// Decorators, `@d`: [`Decorator`] rows.
    decorators, decorator_scratch: Decorator => decorators;
}

// Every value run is a `Run<F>`, which lays out alike for every family `F`, and an absent
// list (the decorators', an import's attributes) folds into the run's start.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::{Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<Run<Decorator>>() == 8);
    assert!(STORE_CHECK || size_of::<Option<Run<Decorator>>>() == 8);
};

// Every payload id is an `Id<F>`, which lays out alike for every family `F`.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::STORE_CHECK;
    assert!(STORE_CHECK || size_of::<BindingExtraId>() == 4);
    assert!(STORE_CHECK || size_of::<Option<BindingExtraId>>() == 4);
};

/// Source bytes per expression row [`AstStores::for_source`] reserves. This and the two rates
/// below are TypeScript's own: calibrated against the rows a parse of real-code TypeScript
/// writes to each store per source byte, file by file, and set a little denser than the
/// typical file.
const SOURCE_BYTES_PER_EXPRESSION: usize = 9;

/// Source bytes per statement row [`AstStores::for_source`] reserves.
const SOURCE_BYTES_PER_STATEMENT: usize = 59;

/// Source bytes per type row [`AstStores::for_source`] reserves.
const SOURCE_BYTES_PER_TYPE: usize = 148;

/// The longest source [`AstStores::for_source`] sizes its reservation by, above every
/// real-code file the rates were calibrated on.
const SIZED_SOURCE_BYTES_MAX: usize = 1 << 20;

/// One document's expression, statement, type, payload and value-run stores, the scratch
/// stacks that build their lists, its text pool and its comments.
///
/// Stores from [`AstStores::new`] allocate nothing: a store's buffer is allocated by its first
/// write, a scratch stack's by its first push, and the pool's by its first string.
/// [`AstStores::for_source`] reserves the three busiest stores for a document's length.
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
    /// The tree's decoded and normalized strings, each a `PoolText<Text>` read through
    /// [`AstView::text`].
    pub(crate) text: TextPool<Text>,
    /// Every comment of the document's programs, a program's named by its `Run<Comment>` and
    /// read through [`AstView::comments`].
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

    /// A row for the escaped name `text` spells: the id an [`IdentName`] carries for a name
    /// its span cannot spell.
    ///
    /// [`IdentName`]: super::IdentName
    #[inline]
    pub(crate) fn push_escaped_name(&mut self, text: PoolText<Text>) -> EscapedNameId {
        self.payloads.escaped_names.push(EscapedName { text })
    }
}

/// A binding's extras re-bound with `decorators`, keeping the type annotation `extra` holds:
/// a new row, since a row is never edited, the one `extra` names (if any) left behind as an
/// orphan.
pub(crate) fn extra_with_decorators(
    payloads: &mut PayloadStores,
    extra: Option<BindingExtraId>,
    decorators: Run<Decorator>,
) -> BindingExtraId {
    let type_annotation =
        extra.and_then(|extra| payloads.binding_extras.get(extra).type_annotation);
    payloads.binding_extras.push(BindingExtra {
        type_annotation,
        decorators: Some(decorators),
    })
}

/// A binding's extras re-bound with the type annotation `ta`, keeping the decorators `extra`
/// holds: a new row, as [`extra_with_decorators`]'s is.
pub(crate) fn extra_with_type_annotation(
    payloads: &mut PayloadStores,
    extra: Option<BindingExtraId>,
    ta: TSTypeAnnotation,
) -> BindingExtraId {
    let decorators = extra.and_then(|extra| payloads.binding_extras.get(extra).decorators);
    payloads.binding_extras.push(BindingExtra {
        type_annotation: Some(ta),
        decorators,
    })
}

/// The tops of the scratch stacks, taken together: a speculation's restore point.
#[derive(Clone, Copy)]
pub(crate) struct ScratchMarks {
    exprs: ListMark<Expression>,
    expr_lists: ListMark<ExprId>,
    expr_opt_lists: ListMark<Option<ExprId>>,
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
            expr_lists: self.expr_list_scratch.mark_in(&mut group),
            expr_opt_lists: self.expr_opt_scratch.mark_in(&mut group),
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
        self.expr_list_scratch.truncate(marks.expr_lists);
        self.expr_opt_scratch.truncate(marks.expr_opt_lists);
        self.stmt_list_scratch.truncate(marks.stmt_lists);
        self.ty_list_scratch.truncate(marks.ty_lists);
        self.runs.truncate(&marks.runs);
        self.comment_scratch.truncate(marks.comments);
    }

    /// Empties every scratch stack, discarding the frames a failed parse left open. The
    /// stores keep their rows, as [`AstStores::truncate_scratch`] leaves them.
    ///
    /// Out of line and cold: it runs only behind a failed parse, and inlined it is a store
    /// per scratch stack at each of the parse driver's failure exits, in every monomorphized
    /// copy of the driver.
    #[cold]
    #[inline(never)]
    pub(crate) fn clear_scratch(&mut self) {
        // destructured whole, as `reset` is, so a scratch stack added to the stores without
        // a line here is a build error rather than a frame a failed parse leaves open
        let Self {
            exprs: _,
            expr_lists: _,
            expr_opt_lists: _,
            expr_scratch,
            expr_list_scratch,
            expr_opt_scratch,
            stmts: _,
            stmt_lists: _,
            stmt_list_scratch,
            tys: _,
            ty_lists: _,
            ty_list_scratch,
            payloads: _,
            runs,
            text: _,
            comments: _,
            comment_scratch,
        } = self;
        expr_scratch.reset();
        expr_list_scratch.reset();
        expr_opt_scratch.reset();
        stmt_list_scratch.reset();
        ty_list_scratch.reset();
        runs.clear_scratch();
        comment_scratch.reset();
    }

    /// Empties the comment scratch alone: the comments a finished parser drained and never
    /// took are dropped with it.
    #[inline]
    pub(crate) fn clear_comment_scratch(&mut self) {
        self.comment_scratch.reset();
    }

    /// Whether every scratch stack is empty, as the parse driver finds and leaves them.
    #[cfg(debug_assertions)]
    pub(crate) fn scratch_is_empty(&self) -> bool {
        // destructured whole, as `clear_scratch` is, so the check reads every stack it clears
        let Self {
            exprs: _,
            expr_lists: _,
            expr_opt_lists: _,
            expr_scratch,
            expr_list_scratch,
            expr_opt_scratch,
            stmts: _,
            stmt_lists: _,
            stmt_list_scratch,
            tys: _,
            ty_lists: _,
            ty_list_scratch,
            payloads: _,
            runs,
            text: _,
            comments: _,
            comment_scratch,
        } = self;
        expr_scratch.is_empty()
            && expr_list_scratch.is_empty()
            && expr_opt_scratch.is_empty()
            && stmt_list_scratch.is_empty()
            && ty_list_scratch.is_empty()
            && runs.scratch_is_empty()
            && comment_scratch.is_empty()
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

    /// Empty stores for one document of `source_len` bytes, the three a parse writes the most
    /// rows to — the expressions, statements and types — reserved from that length up front:
    /// the stores of a one-shot call, whose rows would otherwise grow from `Vec`'s minimum one
    /// doubling at a time, each a reallocation that copies the rows so far. Every other store
    /// allocates on its first write, as [`AstStores::new`]'s do: most payloads are written by
    /// a minority of documents, and a reservation a document never writes to is an allocation
    /// it would not have made.
    ///
    /// The rates sit a little above what a typical file of that length writes, so most
    /// documents parse within the reservation and a denser one pays one doubling. A source
    /// past [`SIZED_SOURCE_BYTES_MAX`] is sized as one that long: the reservation is made
    /// before the parse reads a byte, so its size stays bounded whatever the input, and a
    /// longer document's stores grow from there. Capacity only — a parse into these stores
    /// mints the same ids and builds the same tree as one into [`AstStores::new`]'s.
    #[must_use]
    pub fn for_source(source_len: usize) -> Self {
        let sized = source_len.min(SIZED_SOURCE_BYTES_MAX);
        Self {
            exprs: Store::with_capacity(sized / SOURCE_BYTES_PER_EXPRESSION),
            stmts: Store::with_capacity(sized / SOURCE_BYTES_PER_STATEMENT),
            tys: Store::with_capacity(sized / SOURCE_BYTES_PER_TYPE),
            ..Self::new()
        }
    }

    /// Rewinds the stores in place between documents. Every store once written keeps its slot
    /// 0, so the next document's first write to it mints straight behind it; every store,
    /// scratch stack and the pool keep their capacity. Every id, run and pool text minted so
    /// far is invalid from here on.
    ///
    /// Nothing is ever trimmed. The capacity a holder's largest document grew stays parked
    /// for as long as the holder keeps the stores — what lets a warm loop parse without
    /// allocating — and dropping the stores is what releases it: a `Formatter` dropped, a
    /// binding's thread retired, a WASM instance replaced. The type strip's edit list is not
    /// among these stores but keeps the same rule: `__private::strip_into` parks it per
    /// thread, cleared, until the thread exits.
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

thread_local! {
    /// This thread's parked stores. Boxed, so a take and a park each move a pointer where the
    /// stores inline would be copied whole both ways.
    static PARKED: Cell<Option<Box<AstStores>>> = const { Cell::new(None) };
}

/// Takes this thread's parked stores out of their slot, or fresh ones when there are none.
/// The slot is empty until [`park_stores`] hands them back, so a second take meanwhile, and a
/// thread that never parks them (a panic unwinding past the holder), get fresh stores.
///
/// The half of [`with_stores`] a holder that keeps the stores in a struct of its own calls:
/// it owns the box for as long as it likes, and the thread still parks one store set.
#[must_use]
pub fn take_stores() -> Box<AstStores> {
    PARKED.with(Cell::take).unwrap_or_default()
}

/// Parks `stores` in this thread's slot after an [`AstStores::reset`], keeping slot 0 and
/// capacity, so every id, run and pool text minted from them is invalid from here on. Stores
/// already in the slot are dropped.
pub fn park_stores(mut stores: Box<AstStores>) {
    stores.reset();
    PARKED.with(|cell| cell.set(Some(stores)));
}

/// Run `f` over this thread's parked stores, or over fresh ones when there are none:
/// `tsv_arena`'s take-and-park protocol, here because that crate cannot name these types.
///
/// The stores are out of the slot while `f` runs ([`take_stores`]), so a nested call takes
/// fresh ones and a panic out of `f` leaves the slot empty, as a new thread finds it. Parked
/// after an [`AstStores::reset`] ([`park_stores`]), keeping slot 0 and capacity, so every id,
/// run and pool text `f` minted is invalid once it returns.
pub fn with_stores<R>(f: impl FnOnce(&mut AstStores) -> R) -> R {
    let mut stores = take_stores();
    let result = f(&mut stores);
    park_stores(stores);
    result
}

/// Stores a caller keeps warm across calls in a slot it owns, rewound between them: the
/// take-and-park protocol of [`with_stores`] over the caller's slot instead of the thread's, as
/// the `Formatter` keeps its stores. The slot holds a box, so a take and a park each move a
/// pointer.
///
/// A holder names no store type, so its code reads the same whatever the stores hold.
#[derive(Debug, Default)]
pub struct WarmStores {
    parked: Option<Box<AstStores>>,
}

impl WarmStores {
    /// Takes the parked stores out of the slot, or fresh ones when there are none. The slot is
    /// empty until [`WarmStores::park`] hands them back.
    #[must_use]
    pub fn take(&mut self) -> Box<AstStores> {
        self.parked.take().unwrap_or_default()
    }

    /// Parks `stores` in the slot after an [`AstStores::reset`], keeping slot 0 and capacity,
    /// so every id, run and pool text minted from them is invalid from here on.
    pub fn park(&mut self, mut stores: Box<AstStores>) {
        stores.reset();
        self.parked = Some(stores);
    }

    /// Run `f` over the parked stores, or over fresh ones when there are none, and park them
    /// after an [`AstStores::reset`], keeping slot 0 and capacity, so every id, run and pool
    /// text `f` minted is invalid once it returns. A panic out of `f` leaves the slot empty,
    /// and the next call starts from fresh stores.
    pub fn with<R>(&mut self, f: impl FnOnce(&mut AstStores) -> R) -> R {
        let mut stores = self.take();
        let result = f(&mut stores);
        self.park(stores);
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
    use tsv_lang::store::flush_run;

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
f<T>();
/** @type {T} */ (x);
\\u0061;
type A = new () => X;
type Z = () => X;
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
l: ;
";

    /// Pushes a copy of every payload `parsed` holds into its store in `stores`, and returns
    /// each store's field name with the index the push minted.
    fn push_every_payload(stores: &mut AstStores, parsed: &Parsed) -> Vec<(&'static str, usize)> {
        let mut minted = Vec::new();
        macro_rules! push {
            ($field:ident, $row:expr) => {
                minted.push((
                    stringify!($field),
                    stores.payloads.$field.push($row).index(),
                ))
            };
        }
        let view = parsed.view();
        // the five type payloads ride five aliases, of which one is pushed
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
                    ExpressionKind::CallExpression(x) => {
                        push!(type_args, *x.type_arguments(view).expect("type arguments"));
                    }
                    &ExpressionKind::CommentBoundPair(x) => {
                        push!(comment_pairs, *view.comment_pair(x));
                    }
                    ExpressionKind::Identifier(x) => {
                        let id = x.escaped_name.expect("an escaped name");
                        push!(escaped_names, *view.escaped_name(id));
                    }
                    other => panic!("an unexpected expression: {other:?}"),
                },
                &StatementKind::TSTypeAliasDeclaration(x) => {
                    let x = view.type_alias(x);
                    if !aliased {
                        push!(type_aliases, *x);
                        aliased = true;
                    }
                    match view.ty(x.type_annotation) {
                        &TSType::Function(_, t) => push!(fn_types, *view.fn_type(t)),
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
                    for param in view.exprs(x.params) {
                        if let ExpressionKind::Identifier(name) = &param.kind
                            && let Some(extra) = view.opt_binding_extra(name.extra)
                        {
                            push!(binding_extras, *extra);
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
                &StatementKind::TryStatement(x) => {
                    let x = view.try_stmt(x);
                    push!(try_stmts, *x);
                    push!(catches, *view.opt_catch(x.handler).expect("a catch clause"));
                }
                &StatementKind::LabeledStatement(x) => {
                    push!(labeled_stmts, *view.labeled_stmt(x));
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
                let mark = stores.runs.$scratch.mark();
                stores.runs.$scratch.push($list[0]);
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
        let mut every = RunStores::FIELDS.to_vec();
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

    /// The scratch stacks' names and heights.
    fn scratch_heights(stores: &AstStores) -> Vec<(&'static str, usize)> {
        stores
            .shapes()
            .into_iter()
            .filter(|shape| shape.kind == FieldKind::Scratch)
            .map(|shape| (shape.name, shape.len))
            .collect()
    }

    fn assert_scratch_empty(stores: &AstStores) {
        let open: Vec<_> = scratch_heights(stores)
            .into_iter()
            .filter(|&(_, len)| len != 0)
            .collect();
        assert!(open.is_empty(), "open scratch frames: {open:?}");
    }

    /// `clear_scratch` empties every scratch stack the stores hold, one left open on each,
    /// and touches nothing else: every store and the pool keep their rows, every buffer its
    /// capacity.
    #[test]
    fn clear_scratch_empties_every_scratch_stack_and_nothing_else() {
        let mut stores = AstStores::new();
        fill_every_field(&mut stores);
        let filled = stores.shapes();
        assert_every_field_written(&filled);
        #[cfg(debug_assertions)]
        assert!(!stores.scratch_is_empty());
        stores.clear_scratch();
        assert_scratch_empty(&stores);
        #[cfg(debug_assertions)]
        assert!(stores.scratch_is_empty());
        let expected: Vec<FieldShape> = filled
            .iter()
            .map(|&shape| match shape.kind {
                FieldKind::Scratch => FieldShape { len: 0, ..shape },
                FieldKind::Store | FieldKind::Pool => shape,
            })
            .collect();
        assert_eq!(stores.shapes(), expected);
    }

    /// `truncate_scratch` rewinds every scratch stack the stores hold to its mark, the
    /// value-run stacks included, more than once, and leaves the element below each mark.
    #[test]
    fn truncate_scratch_rewinds_every_scratch_stack_to_its_mark() {
        let mut stores = AstStores::new();
        fill_every_field(&mut stores);
        let marked = scratch_heights(&stores);
        assert!(marked.iter().all(|&(_, len)| len == 1), "{marked:?}");
        let marks = stores.scratch_marks();
        let runs = parse_with_goal(RUNS, Goal::Module).expect("the lists parse");
        let commented = parse_with_goal(DOCUMENT, Goal::Module).expect("parses");
        let comment = commented.view().comments(commented.program.comments)[0];
        for _ in 0..2 {
            push_every_run(&mut stores, &runs, false);
            stores.comment_scratch.push(comment);
            stores.expr_scratch.push(an_expr());
            let expr = stores.exprs.push(an_expr());
            stores.expr_list_scratch.push(expr);
            stores.expr_opt_scratch.push(None);
            let stmt = stores.stmts.push(a_stmt());
            stores.stmt_list_scratch.push(stmt);
            let ty = stores.tys.push(a_ty());
            stores.ty_list_scratch.push(ty);
            let grown = scratch_heights(&stores);
            assert!(grown.iter().all(|&(_, len)| len == 2), "{grown:?}");
            stores.truncate_scratch(&marks);
            assert_eq!(scratch_heights(&stores), marked);
        }
    }

    /// The acorn re-read's two clears: a reading that fails with a list open leaves no frame
    /// behind the parse that is kept, whichever of the two it is.
    #[test]
    fn a_reread_parse_leaves_every_scratch_stack_empty() {
        let mut stores = AstStores::new();
        // tsc's comparison ends the declarator ahead of a second one that is no binding, the
        // program's statement list open; acorn's reading parses and is kept
        crate::__private::parse_into("g(1);\nconst a = f<T, U><V>(x);", &mut stores)
            .expect("the re-read parses");
        assert_scratch_empty(&stores);
        // the first reading parses, a list kept and a list given up, and is kept: the
        // re-read fails with a call's argument list open
        crate::__private::parse_into("f<T><U>;\ng(1, x<T,U><T>\nax);\n", &mut stores)
            .expect("the first reading stands");
        assert_scratch_empty(&stores);
        // both readings fail, each with lists open
        crate::__private::parse_into("const a = f<T, U><V>(x), {", &mut stores)
            .expect_err("fails twice");
        assert_scratch_empty(&stores);
        crate::__private::parse_into(DOCUMENT, &mut stores).expect("parses");
        assert_scratch_empty(&stores);
    }

    /// A parse that fails with lists open — an array in an object in a call in a template,
    /// a declarator, type members, a class body — leaves no frame behind, so the next parse
    /// into the same stores starts over empty scratch stacks.
    #[test]
    fn a_failed_parse_leaves_every_scratch_stack_empty() {
        let mut stores = AstStores::new();
        for source in [
            "// c\nconst a = `${f({ b: [1, , c, /* d */",
            "let a = 1, b: { c: D; e(",
            "@d class C { m() {} n = [",
            "import { a, b as c } from 'm' with { type:",
            "type T<U, V> = [U, { [k: string]:",
            "switch (a) { case 1: enum E { A, B",
        ] {
            crate::__private::parse_into(source, &mut stores).expect_err("fails");
            assert_scratch_empty(&stores);
        }
        crate::__private::parse_into(DOCUMENT, &mut stores).expect("parses");
        assert_scratch_empty(&stores);
    }

    /// A parse whose caller never takes its comments leaves none on the scratch stack.
    #[test]
    fn a_parse_that_never_takes_its_comments_leaves_none() {
        let mut stores = AstStores::new();
        let end =
            crate::__private::parse_type_extent("/* a */ A | /* b */ B // c", 0, 0, &mut stores)
                .expect("a type");
        assert!(end > 0);
        assert_scratch_empty(&stores);
        assert_eq!(stores.comments.len(), 0, "no comment became a row");
    }

    /// The parse driver's entry check, in a build that carries it: a parse handed stores
    /// with a frame open is a caller's bug.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "a parse began over an open list frame")]
    fn a_parse_over_an_open_frame_is_refused() {
        let mut stores = AstStores::new();
        let id = stores.exprs.push(an_expr());
        stores.expr_list_scratch.push(id);
        let _ = crate::__private::parse_into("a;", &mut stores);
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
    fn a_one_shot_parse_reserves_its_busiest_stores_for_the_source() {
        // a comment carries most of the bytes, so the parse writes under half the rows each
        // store reserves, and a store grown from empty by doubling would stay below them
        let source = format!("/*{}*/\nlet a: A = b;\n", " ".repeat(4096));
        let parsed = parse_with_goal(&source, Goal::Module).expect("parses");
        let shapes = parsed.stores.shapes();
        for (name, reserved) in [
            ("exprs", source.len() / SOURCE_BYTES_PER_EXPRESSION),
            ("stmts", source.len() / SOURCE_BYTES_PER_STATEMENT),
            ("tys", source.len() / SOURCE_BYTES_PER_TYPE),
        ] {
            let shape = shapes
                .iter()
                .find(|shape| shape.name == name)
                .expect("a store of the stores");
            assert!(
                shape.len > 0 && 2 * shape.len <= reserved,
                "{name}: the parse writes {} rows of the {reserved} reserved",
                shape.len
            );
            assert!(
                shape.capacity >= reserved,
                "{name}: a one-shot parse's stores are reserved for its source, {} < {reserved}",
                shape.capacity
            );
        }
    }

    #[test]
    fn stores_for_no_source_allocate_nothing_and_the_reservation_is_bounded() {
        let empty = AstStores::for_source(0).shapes();
        assert!(empty.iter().all(|shape| shape.capacity == 0), "{empty:?}");
        assert_eq!(
            AstStores::for_source(usize::MAX).shapes(),
            AstStores::for_source(SIZED_SOURCE_BYTES_MAX).shapes(),
            "a source past the bound reserves what one at the bound does"
        );
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

    /// Where `stores` live: a take that hands back the allocation a park left reads the same
    /// address.
    fn address(stores: &AstStores) -> *const AstStores {
        std::ptr::from_ref(stores)
    }

    #[test]
    fn a_warm_handle_takes_and_parks_one_allocation() {
        let mut warm = WarmStores::default();
        let mut stores = warm.take();
        fill_every_field(&mut stores);
        let filled = stores.shapes();
        let at = address(&stores);
        warm.park(stores);
        let stores = warm.take();
        assert_eq!(
            address(&stores),
            at,
            "the park and the take move the box, never the stores"
        );
        assert_rewound(&filled, &stores.shapes());
        let fresh = warm.take();
        assert_ne!(
            address(&fresh),
            at,
            "the slot is empty while its stores are out"
        );
        assert!(fresh.shapes().iter().all(|shape| shape.capacity == 0));
    }

    #[test]
    fn the_thread_slot_takes_and_parks_one_allocation() {
        let mut stores = take_stores();
        fill_every_field(&mut stores);
        let filled = stores.shapes();
        let at = address(&stores);
        park_stores(stores);
        with_stores(|stores: &mut AstStores| {
            assert_eq!(
                address(stores),
                at,
                "`with_stores` runs over the box the park left"
            );
            assert_rewound(&filled, &stores.shapes());
            let fresh = take_stores();
            assert_ne!(
                address(&fresh),
                at,
                "the slot is empty while its stores are out"
            );
        });
        assert_eq!(
            address(&take_stores()),
            at,
            "the stores come back through the slot"
        );
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
