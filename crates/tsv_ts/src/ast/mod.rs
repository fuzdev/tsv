//! Internal AST - optimized for traversal and manipulation
//!
//! Identifier names are span-identity ([`IdentName`]): recovered from the
//! source slice, with a string of the stores' text pool as a rare escape hatch
//! (unicode-escaped names). This is the primary AST representation used by the
//! parser, formatter, and other tools.
//!
//! ## Storage
//!
//! Expressions, statements, types and the payloads too rare to sit inline in the node that
//! refers to them are rows of per-document stores ([`AstStores`]), named by id and read
//! through an [`AstView`]; the lists built by value are runs of rows in a store per element
//! type. The stores also hold the tree's decoded strings, in one text pool (a [`PoolStr`]
//! handle, read through [`AstView::text`]), and its comments, a program's a run of comment
//! rows (a [`CommentList`], read through [`AstView::comments`]) — so nothing of the tree
//! lives anywhere else, and a rewind of the stores frees all of it with no per-node `Drop`.
//! The rare escaped identifier name is one such string ([`IdentName::escaped`]) — the AST
//! holds no interner and needs no interner lifetime.

#[cfg(feature = "json")]
pub(crate) mod convert;
pub(crate) mod precedence;

mod classes;
mod declarations;
mod expressions;
mod modules;
mod patterns;
mod statements;
mod stores;
mod types;
mod view;

pub(crate) use stores::ScratchMarks;
pub use stores::{
    ArrowFn, ArrowFnId, AstStores, Case, Catch, CatchId, ClassDecl, ClassDeclId, ClassExpr,
    ClassExprId, ClassMem, CommentList, CtorType, CtorTypeId, Declarator, DeclareFn, DeclareFnId,
    Deco, EnumDecl, EnumDeclId, EnumMem, ExportAll, ExportAllId, ExportAssign, ExportAssignId,
    ExportDefault, ExportDefaultId, ExportNamed, ExportNamedId, ExportSpec, Expr, ExprId, FnDecl,
    FnDeclId, FnExpr, FnExprId, ForHead, ForHeadId, ForLeft, ForLeftId, Heritage, ImportAttr,
    ImportDecl, ImportDeclId, ImportEquals, ImportEqualsId, ImportSpec, ImportType, ImportTypeId,
    IndexParam, InferType, InferTypeId, InterfaceDecl, InterfaceDeclId, MetaProp, MetaPropId,
    ModuleDecl, ModuleDeclId, ObjProp, ParamExtra, ParamExtraId, Parsed, PatProp, PoolStr,
    QualName, QualNameId, Quasi, Stmt, StmtId, TaggedTpl, TaggedTplId, Text, Ty, TyId, TypeAlias,
    TypeAliasId, TypeMember, TypeParam, WarmStores, with_stores,
};
pub use tsv_lang::store::Run;
pub use tsv_lang::{Comment, Span};
pub use view::{AstView, ParsedRef};

//
// Re-exports from submodules
//

// Types
pub use types::{
    TSArrayType, TSCallSignatureDeclaration, TSConditionalType, TSConstructSignatureDeclaration,
    TSConstructorType, TSEntityName, TSFunctionType, TSImportType, TSIndexSignature,
    TSIndexedAccessType, TSInferType, TSIntersectionType, TSKeywordKind, TSKeywordType,
    TSLiteralType, TSMappedType, TSMappedTypeModifier, TSMappedTypeParameter, TSMethodSignature,
    TSNamedTupleMember, TSOptionalType, TSParenthesizedType, TSPropertySignature, TSQualifiedName,
    TSRestType, TSThisType, TSTupleType, TSType, TSTypeAliasDeclaration, TSTypeAnnotation,
    TSTypeElement, TSTypeLiteral, TSTypeOperator, TSTypeOperatorKind, TSTypeParameter,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeParameterModifier,
    TSTypeParameterModifiers, TSTypePredicate, TSTypeQuery, TSTypeQueryExprName, TSTypeReference,
    TSUnionType, TemplateLiteralType,
};

// Declarations
pub use declarations::{
    TSDeclareFunction, TSEnumDeclaration, TSEnumMember, TSEnumMemberId, TSInterfaceBody,
    TSInterfaceDeclaration, TSInterfaceHeritage, TSModuleBlock, TSModuleDeclaration,
    TSModuleDeclarationBody, TSModuleDeclarationKind, TSModuleName,
};

// Modules (imports/exports)
pub use modules::{
    ExportAllDeclaration, ExportDefaultDeclaration, ExportDefaultValue, ExportFunctionDeclaration,
    ExportKind, ExportNamedDeclaration, ExportSpecifier, ImportAttribute, ImportAttributeKey,
    ImportDeclaration, ImportDefaultSpecifier, ImportKind, ImportNamedSpecifier,
    ImportNamespaceSpecifier, ImportPhase, ImportSpecifier, ModuleExportName, TSExportAssignment,
    TSExternalModuleReference, TSImportEqualsDeclaration, TSModuleReference,
    TSNamespaceExportDeclaration,
};

// Classes
pub use classes::{
    Accessibility, ClassBody, ClassDeclaration, ClassExpression, ClassMember, MethodDefinition,
    MethodKind, PropertyDefinition, PropertyModifier, StaticBlock, TSParameterProperty,
};

// Patterns
pub use patterns::{
    ArrayPattern, AssignmentPattern, ObjectPattern, ObjectPatternProperty, RestElement,
};

// Statements
pub use statements::{
    BlockStatement, BreakStatement, CatchClause, ContinueStatement, DebuggerStatement,
    DoWhileStatement, EmptyStatement, ExpressionStatement, ForInOfLeft, ForInStatement, ForInit,
    ForOfStatement, ForStatement, FunctionDeclaration, IfStatement, LabeledStatement,
    ReturnStatement, Statement, StatementKind, SwitchCase, SwitchStatement, ThrowStatement,
    TryStatement, VariableDeclaration, VariableDeclarationKind, VariableDeclarator, WhileStatement,
    WithStatement,
};

// Expressions
pub use expressions::{
    ArrayExpression, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AssignmentOperator, AwaitExpression, BinaryExpression, BinaryOperator, CallExpression,
    CommentBoundPair, CommentBoundPairKind, ConditionalExpression, Expression, ExpressionKind,
    FunctionExpression, ImportExpression, MemberExpression, MetaProperty, NewExpression,
    ObjectExpression, ObjectProperty, ParenthesizedExpression, Property, PropertyKind,
    RegexLiteral, SequenceExpression, SpreadElement, Super, TSAsExpression,
    TSInstantiationExpression, TSNonNullExpression, TSSatisfiesExpression, TSTypeAssertion,
    TaggedTemplateExpression, TemplateCooked, TemplateElement, TemplateLiteral, ThisExpression,
    UnaryExpression, UnaryOperator, UpdateExpression, UpdateOperator, YieldExpression,
};

// The two node enums are wide enough that holding one BY VALUE across a recursive
// call is a stack-depth decision, not a style one: a frame is sized once for its
// widest arm, so a dispatcher with N arms that each hold one reserves N times these
// bytes at every recursion level, which is the stack the depth limits' margin is made
// of (`docs/cli.md` §Recursion Depth); the parser's expression ladder returns an id
// to keep `Expression` out of those frames.
//
// Both enums are also the ELEMENT WIDTH of the containers that hold them, which is
// the larger cost: an `Expression` slot is paid on every row of the expression store,
// every parameter of a parameter list's value run among them, so the enum's width
// multiplies through the whole tree. Both are held down the same way — the variants
// wide enough to set the size on their own, and rare enough that a row of their own is
// free, hold their payload by id into a store per payload type: `Expression`'s five
// widest (`ClassExpression` / `FunctionExpression` / `ArrowFunctionExpression` /
// `MetaProperty` / `TaggedTemplateExpression`, together ~3% of expressions, of which
// the widest two are ~0.02%), `TSType`'s three widest (`TSImportType`,
// `TSConstructorType`, `TSInferType`, so the inline variants set its pinned width below),
// `Statement`'s DECLARATION heads and its four loop / `try` heads one level down
// (`ast/statements.rs`). Two of those declaration heads — `ImportDeclaration` and
// `ExportNamedDeclaration` — are stored despite NOT being rare (6.8–11.7% and 2.4–4.0%
// of statements): they are simply the last two rungs of the ladder, and a stored head's
// construction copies the same bytes into its store that it would have moved into the
// enum, so the cost is one row push against 24 bytes off every other statement slot.
//
// The LIST-ELEMENT containers whose own width was inline `Expression`s —
// `Property` (an object literal's `key: value`, and a destructuring pattern's),
// `VariableDeclarator`, and every `Expression`-holding `Statement` head
// (`ExpressionStatement` 8 B, `IfStatement` 12, `SwitchStatement` 12, `SwitchCase` 20,
// `WhileStatement` / `DoWhileStatement` / `WithStatement` 8,
// `ReturnStatement` / `ThrowStatement` 4)
// — instead hold those slots by id, which is not the same trade: the parser's
// expression spine already returns a pushed row's id, so an inline slot would be a COPY
// OUT of the store rather than a place the node lives. Naming the slots by id removes
// that copy instead of adding a row, and takes the element every object-literal and
// declarator list moves from 160 B to 20 (`ObjectPatternProperty` 28, its `RestElement`
// arm setting it). `CatchClause` keeps its inline `param` deliberately: it is built by
// the parser as an owned value rather than through the spine, so an id there would ADD a
// row, and `CatchClause` is reached only through an `Option<CatchId>` so its width sets
// nothing. The expression LISTS whose elements come off the spine hold ids — call and
// `new` arguments and sequence and template expressions (`Run<ExprId>`), array-literal
// and array-pattern elements (`Run<Option<ExprId>>`) — and the parameter lists are value
// runs over the expression store itself (`Run<Expr>`), `CatchClause`'s case. The
// statement lists — program, block, `case`, static-block and namespace bodies — hold ids
// too (`Run<StmtId>`), as the single statement edges do (`StmtId`): every statement is a
// row of the statement store. So do the type lists — union and intersection members, tuple
// elements, template-literal interpolations, type arguments (`Run<TyId>`) — and the single
// type edges (`TyId`): the type ladder pushes every type it parses, so each is a row of the
// type store. The lists whose elements are built whole and stand nowhere else — object
// members and patterns' members, template quasis, declarators, `switch` cases, import and
// export specifiers and import attributes, class and type members, type parameters,
// heritage clauses, enum members, index-signature parameters, decorators — are value runs
// (`Run<ObjProp>`, `Run<Declarator>`, …): the run's rows ARE the elements, one store per
// element type (`ast/stores.rs`).
//
// `Expression` and `Statement` are each a header over their variant (`span` +
// `ExpressionKind` / `StatementKind`), so a span read is a field load rather than a
// dispatch; the variant payloads shed their own spans to pay for the header.
//
// Pinned so a variant that widens any of them shows up as a failed test build rather
// than as a silently thinner depth-limit margin and a fatter element slot. Each is rustc's
// layout, not a guarantee, so it compiles under `cfg(test)` only: a toolchain that lays them
// out differently fails `cargo test` here, never a consumer's build. The counts are
// pointer-width-relative and the doc's measurements are x86-64; every width here is also
// pinned on wasm32 (a 4 B pointer; `Expression`'s payload is 8 B aligned, `Statement`'s
// 4 B) in `tsv_wasm`, the crate compiled for wasm32, and `Expression`'s and `Statement`'s
// `Option` must stay niche-packed on both.
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<Expression>() == 48);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<ExpressionKind>() == 40);
#[cfg(test)]
const _: () = assert!(size_of::<Option<Expression>>() == size_of::<Expression>());
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<Statement>() == 48);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<StatementKind>() == 40);
#[cfg(test)]
const _: () = assert!(size_of::<Option<Statement>>() == size_of::<Statement>());
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<Property>() == 20);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<ObjectProperty>() == 20);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<ObjectPatternProperty>() == 28);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<VariableDeclarator>() == 20);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<SwitchCase>() == 20);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<TSType>() == 56);
// The type-edge holders every type annotation, type-argument list and type-parameter list
// rides inline in: their `Option`s fold into the id's niche and the run's, so an absent
// annotation or list costs no width in any of the nodes that hold one.
#[cfg(test)]
const _: () = assert!(
    tsv_lang::store::STORE_CHECK
        || size_of::<Option<TSTypeAnnotation>>() == size_of::<TSTypeAnnotation>()
);
#[cfg(test)]
const _: () = assert!(
    tsv_lang::store::STORE_CHECK
        || size_of::<Option<TSTypeParameterInstantiation>>()
            == size_of::<TSTypeParameterInstantiation>()
);
#[cfg(test)]
const _: () = assert!(
    tsv_lang::store::STORE_CHECK
        || size_of::<Option<TSTypeParameterDeclaration>>()
            == size_of::<TSTypeParameterDeclaration>()
);

//
// Foundational Types (defined here, used everywhere)
//

/// Program node - the root of the AST
///
/// Returned by value from `parse`; `body` and `comments` are runs of the stores the parse
/// wrote (the parser gathers comments on the stores' comment scratch and flushes them once,
/// so the warm binding loops never malloc for them).
#[derive(Debug, Clone)]
pub struct Program {
    pub body: Run<StmtId>,
    pub comments: CommentList,
    pub span: Span,
    /// The goal symbol this program was parsed against. Drives the public AST's
    /// `sourceType` and (eventually) the goal-specific grammar gates.
    pub goal: crate::Goal,
}

/// Decorator: `@expression` applied to classes and class members
///
/// The expression can be an identifier (`@foo`), call expression (`@foo()`),
/// or member expression (`@foo.bar`).
#[derive(Debug, Clone)]
pub struct Decorator {
    /// The decorator expression (identifier, call, or member expression)
    pub expression: ExprId,
    pub span: Span,
}

/// Literal value type - supports numbers, strings, booleans, null, and undefined
#[derive(Debug, Clone)]
pub enum LiteralValue {
    Number(f64),
    /// String literal. The decoded value is recovered via
    /// `StringCooked::resolve(span, source, view)` (no-escape = zero-copy inner slice;
    /// escaped = pool text); the quote char via `Literal::string_quote(source)`.
    String(StringCooked),
    /// BigInt literal: `1n`, `100n`, `0xffn`. No stored payload — digits via
    /// `Literal::bigint_digits(source)` (span minus trailing `n`); the printer
    /// re-derives from source and convert reads the source slice.
    BigInt,
    Boolean(bool),
    Null,
}

/// The decoded value of a string literal, mirroring [`crate::ast::TemplateCooked`].
///
/// `Verbatim` (the common no-escape case) carries **no allocation** — the decoded
/// value equals the inner source slice (the literal's `span` minus the two quote
/// bytes). Only escaped strings hold text of their own, in the stores' text pool.
#[derive(Debug, Clone)]
pub enum StringCooked {
    /// Decoded value == the inner source slice (no escapes to decode).
    Verbatim,
    /// Escapes were decoded into a value distinct from the raw inner text.
    Decoded(PoolStr),
}

impl StringCooked {
    /// The decoded string value. `span` is the owning [`Literal`]'s span (the
    /// quoted token); `source` is the host document. `Verbatim` slices the inner
    /// text (zero-copy); `Decoded` reads the tree's text through `view`.
    ///
    /// Inside the parser use `Parser::resolve_cooked` instead — there
    /// `self.source` is the local (embedded) slice, not the host document,
    /// so the host-coordinate span must shift back by `base_offset` first.
    #[inline]
    pub fn resolve<'s>(&'s self, span: Span, source: &'s str, view: AstView<'s>) -> &'s str {
        match *self {
            StringCooked::Verbatim => {
                let raw = span.extract(source);
                // The string token's source slice always includes both quote
                // delimiters (≥2 bytes), so stripping one from each end is in bounds.
                &raw[1..raw.len() - 1]
            }
            StringCooked::Decoded(decoded) => view.text(decoded),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Literal {
    pub value: LiteralValue,
    pub span: Span,
}

impl Literal {
    /// The quote character of a string literal — the byte at the span start.
    /// (Recovered from source rather than stored.)
    #[inline]
    pub fn string_quote(&self, source: &str) -> u8 {
        source.as_bytes()[self.span.start as usize]
    }

    /// The BigInt digits — the literal source minus the trailing `n`.
    #[inline]
    pub fn bigint_digits<'s>(&self, source: &'s str) -> &'s str {
        let raw = self.span.extract(source);
        &raw[..raw.len() - 1]
    }
}

/// The name channel of an identifier-like node: span-identity by default, with
/// a pool-text escape hatch for the rare names the source can't recover.
///
/// `escaped` is `Some` only when the name text differs from the leading
/// `raw_len` bytes at the node's span start — a `\u` unicode escape
/// (`\u0066oo` → `foo`), or a name too long for `raw_len` (> `u16::MAX`
/// bytes) — or when those bytes need a JSON escape, which only a name the grammar
/// does not vouch for can hold (a Svelte directive name like `a\b`; see below).
/// It then carries the name as a [`PoolStr`], read through [`AstView::text`] at every
/// consumer — no interner round-trip: for an escape, the decoded name (the parser's
/// `current_decoded`, already in the pool); for an oversized name, its raw text
/// copied into the pool; for a Svelte-synthesized name, its text copied into the pool
/// verbatim. Otherwise (`None`, >99.99% of
/// identifiers) the name is the raw source slice
/// `span.start .. span.start + raw_len` — nothing stored at all.
///
/// `raw_len` is the raw name-*token* byte length, fixed at token time: the
/// owning node's span may later extend past the name (`?`, `!`, `: Type` —
/// acorn parity), so the name is the leading `raw_len` bytes, never the whole
/// span. When `escaped` is `Some`, `raw_len` is 0 and unused — read the
/// pool text instead.
///
/// ⚠️ **`escaped: None` is also a claim about the bytes**: the raw slice holds no
/// `"`, no `\` and no control byte, so the wire writer emits it without a JSON
/// escape scan (`write_name_field`). A lexed `IdentifierName` satisfies it by the
/// grammar — its characters are `ID_Start` / `ID_Continue`, `$`, ZWNJ and ZWJ,
/// an escaped spelling always carries its decoded form, and a keyword token is
/// never escaped. A constructor over a slice the grammar does not vouch for (a
/// Svelte directive name, which may hold `\`) must check the bytes and fall back
/// to `escaped: Some`; debug builds assert the claim at the write.
#[derive(Debug, Clone, Copy)]
pub struct IdentName {
    pub escaped: Option<PoolStr>,
    pub raw_len: u16,
    /// Whether the raw name is **plain one-column ASCII** — no byte at or above
    /// `0x80`. Answered by the lexer, which walked those bytes anyway and records the
    /// rare tokens that are NOT plain (`Lexer::nonplain_ident_starts`); the parser asks
    /// it by the token's start offset. Like `raw_len`, it describes the *source slice*,
    /// so it is meaningless (and unread) when `escaped` is `Some`; those sites write
    /// `false` the way they write `raw_len: 0`.
    ///
    /// The claim it licenses: an identifier can hold no `\t` and no `\n`
    /// (`IdentifierPart`'s ASCII subset is `[A-Za-z0-9_$]`), so a plain-ASCII name's
    /// **visual width IS `raw_len`** and the printer's width scan has nothing to find
    /// — see `DocArena::source_span_plain`. `false` is always safe: it costs the scan,
    /// never a wrong width, so a site that cannot prove the property says `false`.
    pub plain_ascii: bool,
}

impl IdentName {
    /// A verbatim name covering `span` exactly (keyword/synthetic sites where
    /// the token has already been consumed — the span is the name token).
    ///
    /// ⚠️ **`plain_ascii` is `true`, and that is a CONTRACT on the caller**: this
    /// constructor is handed a span, not the bytes under it, so the span must cover a
    /// reserved or contextual keyword — ASCII by the grammar. Every caller today is one
    /// of the two meta-properties (`new.target`, `import.meta`). A caller whose span is
    /// an arbitrary name must build the channel from the token instead
    /// (`Parser::current_raw_ident_name`); the printer's name seam asserts the property
    /// in debug builds, so a violation fails the suite rather than shifting a width.
    #[inline]
    pub fn from_span(span: Span) -> Self {
        debug_assert!(u16::try_from(span.end - span.start).is_ok());
        Self {
            escaped: None,
            raw_len: (span.end - span.start) as u16,
            plain_ascii: true,
        }
    }

    /// Resolve the name: an escaped name's text, read through `view`, else the
    /// leading `raw_len` bytes at `span_start`. `source` must be the host
    /// document the spans were recorded against. `IdentName` is `Copy`, so it
    /// is taken by value.
    #[inline]
    pub fn resolve<'s>(self, span_start: u32, source: &'s str, view: AstView<'s>) -> &'s str {
        match self.escaped {
            Some(s) => view.text(s),
            None => &source[span_start as usize..span_start as usize + self.raw_len as usize],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Identifier {
    /// The [`IdentName`] channel's escape hatch: the decoded name as pool text,
    /// `Some` only for `\u`-escaped or `raw_len`-oversized
    /// names, and for a Svelte-synthesized name holding a JSON-escaped byte (see
    /// [`IdentName`]). Stored flattened (beside `name_len`) rather than as a nested
    /// [`IdentName`] — the nested struct's tail padding would grow `Identifier`,
    /// and it is an *inline* `Expression` variant. (The handle is the widest field
    /// here; `Expression` is set by wider variants, so it does not move
    /// `sizeof(Expression)`, but it sets `Identifier`'s own width, which every node
    /// holding one inline pays, a type name or a statement label among them.) Read via
    /// [`Self::ident_name`].
    pub escaped_name: Option<PoolStr>,
    /// The [`IdentName`] channel's `raw_len`: the raw name-token byte length
    /// (the node span may extend past the name — `?` / `!` / `: Type`).
    pub name_len: u16,
    /// The [`IdentName`] channel's `plain_ascii` — see it for what the flag claims and
    /// who spends it. Flattened beside `name_len` for the same reason `escaped_name`
    /// is, and free: it sits in tail padding the struct already had.
    pub name_plain_ascii: bool,
    /// Whether this is an optional parameter (e.g., `a?` in `function fn(a?: number) {}`)
    pub optional: bool,
    /// Binding-only state (type annotation + parameter decorators), present only
    /// when this identifier is a *binding* — a parameter, `const x: T` declarator
    /// id, catch param, index-signature param, or `{#snippet}` param. `None` for
    /// every variable *reference* (the overwhelming majority). Folded behind one id
    /// into the extras' store, so a reference pays 4 B for it: `Identifier` is an
    /// *inline* `Expression` variant, held inline by type names and statement labels
    /// too. Read through a view, via the [`Self::type_annotation`] / [`Self::decorators`]
    /// accessors.
    ///
    /// ⚠️ Every field of `Identifier` beyond the name is `optional` or behind `extra`: the
    /// three expression-position fast paths (the printer's `build_identifier_expression_doc`,
    /// the wire's `write_expression_inner`, the type strip's `Walk::expr`) test just those two
    /// (the wire's also its `force_optional` context flag), and `build_identifier_doc_inner`'s
    /// fast path tests `optional` plus each accessor — a new `Identifier` field must join all
    /// four, a new `IdentifierParamExtra` field the last.
    pub extra: Option<ParamExtraId>,
    pub span: Span,
}

/// Binding-only extension of [`Identifier`] — the type annotation and parameter
/// decorators a binding identifier carries. A row of its own store, named by
/// `Identifier.extra` only at the few binding sites that set it; absent (`None`, folded
/// into the id's niche) for every reference.
#[derive(Debug, Clone)]
pub struct IdentifierParamExtra {
    pub type_annotation: Option<TSTypeAnnotation>,
    pub decorators: Option<Run<Deco>>,
}

impl Identifier {
    /// The type annotation, if this is a typed binding (`None` for a reference), its extras
    /// read through `view`.
    #[inline]
    pub fn type_annotation<'a>(&self, view: AstView<'a>) -> Option<&'a TSTypeAnnotation> {
        view.opt_param_extra(self.extra)
            .and_then(|e| e.type_annotation.as_ref())
    }

    /// The parameter decorators, if any (`None` for a reference), its extras read through
    /// `view`.
    #[inline]
    pub fn decorators<'a>(&self, view: AstView<'a>) -> Option<&'a [Decorator]> {
        view.opt_param_extra(self.extra)
            .and_then(|e| e.decorators)
            .map(|list| view.decorators(list))
    }

    /// The name channel, reassembled from the flattened fields.
    #[inline]
    pub fn ident_name(&self) -> IdentName {
        IdentName {
            escaped: self.escaped_name,
            raw_len: self.name_len,
            plain_ascii: self.name_plain_ascii,
        }
    }

    /// The name's sub-span: the leading `name_len` bytes at the span start (the
    /// node span may extend over `?` / `!` / `: Type`). Only meaningful when
    /// `escaped_name` is `None` — an escaped name reads its pool text instead.
    #[inline]
    pub fn name_span(&self) -> Span {
        Span::new(self.span.start, self.span.start + self.name_len as u32)
    }

    /// Resolve the identifier's name: the raw source slice (span-identity), or an
    /// escaped name's text, read through `view`. `source` must be the host document
    /// the spans were recorded against.
    #[inline]
    pub fn name<'s>(&self, source: &'s str, view: AstView<'s>) -> &'s str {
        self.ident_name().resolve(self.span.start, source, view)
    }

    /// Create a simple identifier (a reference): no optional flag, no binding extra.
    ///
    /// Use this for identifiers in expression context (not parameters). For a
    /// binding that carries `?` / a type annotation / decorators, construct
    /// directly with `extra` naming a pushed [`IdentifierParamExtra`] row.
    #[inline]
    pub fn simple(name: IdentName, span: Span) -> Self {
        Self {
            escaped_name: name.escaped,
            name_len: name.raw_len,
            name_plain_ascii: name.plain_ascii,
            optional: false,
            extra: None,
            span,
        }
    }
}

/// Private identifier: `#foo` in class fields and methods
///
/// Used for truly private class members (ES2022 private class fields).
/// The name does NOT include the `#` prefix, while the holding `Expression`'s span
/// DOES include the `#` character — so the verbatim name is the span minus its
/// leading byte.
#[derive(Debug, Clone)]
pub struct PrivateIdentifier {
    /// The name channel (name excludes the `#`; `raw_len` covers the name
    /// bytes after the `#`).
    pub name: IdentName,
}

impl PrivateIdentifier {
    /// The name's sub-span: the trailing `raw_len` bytes of `span`, the span of the
    /// `Expression` holding this node (the name token ends the span; anchoring at the
    /// end stays correct even if the parser ever tolerated separation after the `#`).
    #[inline]
    pub fn name_span(&self, span: Span) -> Span {
        Span::new(span.end - self.name.raw_len as u32, span.end)
    }

    /// Resolve the name (without `#`): the raw source slice, or an escaped name's
    /// text, read through `view`. `span` is the holding `Expression`'s.
    #[inline]
    pub fn name<'s>(&self, span: Span, source: &'s str, view: AstView<'s>) -> &'s str {
        self.name
            .resolve(span.end - self.name.raw_len as u32, source, view)
    }
}

// The size pins at the top of this file are the other half of the layout trade: an inline
// variant pays its width on every row of its store and at every recursion level that holds
// one by value (`docs/cli.md` §Recursion Depth), so a variant that widens an enum fails the
// test build instead of silently eating the depth limits' margin.
