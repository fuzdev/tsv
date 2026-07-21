//! Internal AST - optimized for traversal and manipulation
//!
//! Identifier names are span-identity ([`IdentName`]): recovered from the
//! source slice, with an arena-allocated `&'arena str` as a rare escape hatch
//! (unicode-escaped names). This is the primary AST representation used by the
//! parser, formatter, and other tools.
//!
//! ## Arena allocation
//!
//! AST nodes are allocated in a per-parse [`bumpalo::Bump`] supplied by the
//! caller. Recursive children are `&'arena T<'arena>` (not `Box`), child
//! collections are `&'arena [T<'arena>]` (not `Vec`), and decoded strings are
//! `&'arena str` (not `String`) — so a whole parse is one bump-allocated graph,
//! freed wholesale when the `Bump` drops, with no per-node `Drop`. The rare
//! escaped identifier name is one such `&'arena str` ([`IdentName::escaped`]),
//! read directly at every consumer — the AST holds no interner and needs no
//! interner lifetime.

mod classes;
mod declarations;
mod expressions;
mod modules;
mod patterns;
mod statements;
mod types;

pub use tsv_lang::{Comment, Span};

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
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypePredicate, TSTypeQuery,
    TSTypeQueryExprName, TSTypeReference, TSUnionType, TemplateLiteralType,
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
    ReturnStatement, Statement, SwitchCase, SwitchStatement, ThrowStatement, TryStatement,
    VariableDeclaration, VariableDeclarationKind, VariableDeclarator, WhileStatement,
};

// Expressions
pub use expressions::{
    ArrayExpression, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AssignmentOperator, AwaitExpression, BinaryExpression, BinaryOperator, CallExpression,
    ConditionalExpression, Expression, FunctionExpression, ImportExpression, JsdocCast,
    MemberExpression, MetaProperty, NewExpression, ObjectExpression, ObjectProperty,
    ParenthesizedExpression, Property, PropertyKind, RegexLiteral, SequenceExpression,
    SpreadElement, Super, TSAsExpression, TSInstantiationExpression, TSNonNullExpression,
    TSSatisfiesExpression, TSTypeAssertion, TaggedTemplateExpression, TemplateCooked,
    TemplateElement, TemplateLiteral, ThisExpression, UnaryExpression, UnaryOperator,
    UpdateExpression, UpdateOperator, YieldExpression,
};

//
// Foundational Types (defined here, used everywhere)
//

/// Program node - the root of the AST
///
/// Returned by value from `parse`; `body` and `comments` point into the
/// caller-supplied `'arena` (the parser gathers comments directly in the bump,
/// so the warm binding loops never malloc for them; `Comment` is a `Copy` POD,
/// satisfying bumpalo's no-`Drop` rule).
#[derive(Debug, Clone)]
pub struct Program<'arena> {
    pub body: &'arena [Statement<'arena>],
    pub comments: &'arena [Comment],
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
pub struct Decorator<'arena> {
    /// The decorator expression (identifier, call, or member expression)
    pub expression: Expression<'arena>,
    pub span: Span,
}

/// Literal value type - supports numbers, strings, booleans, null, and undefined
#[derive(Debug, Clone)]
pub enum LiteralValue<'arena> {
    Number(f64),
    /// String literal. The decoded value is recovered via
    /// `StringCooked::resolve(span, source)` (no-escape = zero-copy inner slice;
    /// escaped = arena bytes); the quote char via `Literal::string_quote(source)`.
    String(StringCooked<'arena>),
    /// BigInt literal: `1n`, `100n`, `0xffn`. No stored payload — digits via
    /// `Literal::bigint_digits(source)` (span minus trailing `n`); the printer
    /// re-derives from source and convert reads the source slice.
    BigInt,
    Boolean(bool),
    Null,
}

/// The decoded value of a string literal, mirroring [`crate::ast::internal::TemplateCooked`].
///
/// `Verbatim` (the common no-escape case) carries **no allocation** — the decoded
/// value equals the inner source slice (the literal's `span` minus the two quote
/// bytes). Only escaped strings own arena bytes.
#[derive(Debug, Clone)]
pub enum StringCooked<'arena> {
    /// Decoded value == the inner source slice (no escapes to decode).
    Verbatim,
    /// Escapes were decoded into a value distinct from the raw inner text.
    Decoded(&'arena str),
}

impl<'arena> StringCooked<'arena> {
    /// The decoded string value. `span` is the owning [`Literal`]'s span (the
    /// quoted token); `source` is the host document. `Verbatim` slices the inner
    /// text (zero-copy); `Decoded` returns the arena bytes. Both share `'s`
    /// (`'arena: 's` via `&'s self`).
    ///
    /// Inside the parser use `Parser::resolve_cooked` instead — there
    /// `self.source` is the local (embedded) slice, not the host document,
    /// so the host-coordinate span must shift back by `base_offset` first.
    #[inline]
    pub fn resolve<'s>(&'s self, span: Span, source: &'s str) -> &'s str {
        match self {
            StringCooked::Verbatim => {
                let raw = span.extract(source);
                // The string token's source slice always includes both quote
                // delimiters (≥2 bytes), so stripping one from each end is in bounds.
                &raw[1..raw.len() - 1]
            }
            StringCooked::Decoded(decoded) => decoded,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Literal<'arena> {
    pub value: LiteralValue<'arena>,
    pub span: Span,
}

impl<'arena> Literal<'arena> {
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
/// an arena-allocated escape hatch for the rare names the source can't recover.
///
/// `escaped` is `Some` only when the name text differs from the leading
/// `raw_len` bytes at the node's span start — a `\u` unicode escape
/// (`foo` → `foo`), or a name too long for `raw_len` (> `u16::MAX`
/// bytes). It then carries the decoded name as an `&'arena str` (the parser's
/// `current_decoded`, already arena-allocated), read directly at every
/// consumer — no interner round-trip. Otherwise (`None`, >99.99% of
/// identifiers) the name is the raw source slice
/// `span.start .. span.start + raw_len` — nothing stored at all.
///
/// `raw_len` is the raw name-*token* byte length, fixed at token time: the
/// owning node's span may later extend past the name (`?`, `!`, `: Type` —
/// acorn parity), so the name is the leading `raw_len` bytes, never the whole
/// span. When `escaped` is `Some`, `raw_len` is 0 and unused — read the
/// `&'arena str` directly.
#[derive(Debug, Clone, Copy)]
pub struct IdentName<'arena> {
    pub escaped: Option<&'arena str>,
    pub raw_len: u16,
}

impl<'arena> IdentName<'arena> {
    /// A verbatim name covering `span` exactly (keyword/synthetic sites where
    /// the token has already been consumed — the span is the name token).
    #[inline]
    pub fn from_span(span: Span) -> Self {
        debug_assert!(u16::try_from(span.end - span.start).is_ok());
        Self {
            escaped: None,
            raw_len: (span.end - span.start) as u16,
        }
    }

    /// Resolve the name: the decoded `&'arena str` for an escaped name, else
    /// the leading `raw_len` bytes at `span_start`. `source` must be the host
    /// document the spans were recorded against. `IdentName` is `Copy`, so it
    /// is taken by value; the escaped arena string outlives `source`
    /// (`'arena: 's`), so both arms unify at the source lifetime.
    #[inline]
    pub fn resolve<'s>(self, span_start: u32, source: &'s str) -> &'s str
    where
        'arena: 's,
    {
        match self.escaped {
            Some(s) => s,
            None => &source[span_start as usize..span_start as usize + self.raw_len as usize],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Identifier<'arena> {
    /// The [`IdentName`] channel's escape hatch: the decoded name as an
    /// `&'arena str`, `Some` only for `\u`-escaped or `raw_len`-oversized
    /// names. Stored flattened (beside `name_len`) rather than as a nested
    /// [`IdentName`] — the nested struct's tail padding would grow `Identifier`,
    /// and it is an *inline* `Expression` variant. (`Expression` is dominated by
    /// far larger variants, so the fat-pointer field does not move
    /// `sizeof(Expression)`.) Read via [`Self::ident_name`].
    pub escaped_name: Option<&'arena str>,
    /// The [`IdentName`] channel's `raw_len`: the raw name-token byte length
    /// (the node span may extend past the name — `?` / `!` / `: Type`).
    pub name_len: u16,
    /// Whether this is an optional parameter (e.g., `a?` in `function fn(a?: number) {}`)
    pub optional: bool,
    /// Binding-only state (type annotation + parameter decorators), present only
    /// when this identifier is a *binding* — a parameter, `const x: T` declarator
    /// id, catch param, index-signature param, or `{#snippet}` param. `None` for
    /// every variable *reference* (the overwhelming majority). Folded behind one
    /// arena pointer so `Identifier` stays ~24 B: it is an *inline* `Expression`
    /// variant, so its size drives `sizeof(Expression)`. Read via the
    /// `type_annotation()` / `decorators()` accessors.
    pub extra: Option<&'arena IdentifierParamExtra<'arena>>,
    pub span: Span,
}

/// Binding-only extension of [`Identifier`] — the type annotation and parameter
/// decorators a binding identifier carries. Arena-allocated and pointed to from
/// `Identifier.extra` only at the few binding sites that set it; absent (one null
/// pointer) for every reference.
#[derive(Debug, Clone)]
pub struct IdentifierParamExtra<'arena> {
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    pub decorators: Option<&'arena [Decorator<'arena>]>,
}

impl<'arena> Identifier<'arena> {
    /// The type annotation, if this is a typed binding (`None` for a reference).
    #[inline]
    pub fn type_annotation(&self) -> Option<&TSTypeAnnotation<'arena>> {
        self.extra.and_then(|e| e.type_annotation.as_ref())
    }

    /// The parameter decorators, if any (`None` for a reference).
    #[inline]
    pub fn decorators(&self) -> Option<&'arena [Decorator<'arena>]> {
        self.extra.and_then(|e| e.decorators)
    }

    /// The name channel, reassembled from the flattened fields.
    #[inline]
    pub fn ident_name(&self) -> IdentName<'arena> {
        IdentName {
            escaped: self.escaped_name,
            raw_len: self.name_len,
        }
    }

    /// The name's sub-span: the leading `name_len` bytes at the span start (the
    /// node span may extend over `?` / `!` / `: Type`). Only meaningful when
    /// `escaped_name` is `None` — an escaped name reads its `&'arena str` directly.
    #[inline]
    pub fn name_span(&self) -> Span {
        Span::new(self.span.start, self.span.start + self.name_len as u32)
    }

    /// Resolve the identifier's name: the raw source slice (span-identity), or
    /// the decoded `&'arena str` for escaped names. `source` must be the host
    /// document the spans were recorded against.
    #[inline]
    pub fn name<'s>(&self, source: &'s str) -> &'s str
    where
        'arena: 's,
    {
        self.ident_name().resolve(self.span.start, source)
    }

    /// Create a simple identifier (a reference): no optional flag, no binding extra.
    ///
    /// Use this for identifiers in expression context (not parameters). For a
    /// binding that carries `?` / a type annotation / decorators, construct
    /// directly with `extra: Some(arena.alloc(IdentifierParamExtra { … }))`.
    #[inline]
    pub fn simple(name: IdentName<'arena>, span: Span) -> Self {
        Self {
            escaped_name: name.escaped,
            name_len: name.raw_len,
            optional: false,
            extra: None,
            span,
        }
    }
}

/// Private identifier: `#foo` in class fields and methods
///
/// Used for truly private class members (ES2022 private class fields).
/// The name does NOT include the `#` prefix, while the span DOES include the
/// `#` character — so the verbatim name is the span minus its leading byte.
#[derive(Debug, Clone)]
pub struct PrivateIdentifier<'arena> {
    /// The name channel (name excludes the `#`; `raw_len` covers the name
    /// bytes after the `#`).
    pub name: IdentName<'arena>,
    pub span: Span,
}

impl<'arena> PrivateIdentifier<'arena> {
    /// The name's sub-span: the trailing `raw_len` bytes of the span (the name
    /// token ends the span; anchoring at the end stays correct even if the
    /// parser ever tolerated separation after the `#`).
    #[inline]
    pub fn name_span(&self) -> Span {
        Span::new(self.span.end - self.name.raw_len as u32, self.span.end)
    }

    /// Resolve the name (without `#`): the raw source slice, or the decoded
    /// `&'arena str` for escaped names.
    #[inline]
    pub fn name<'s>(&self, source: &'s str) -> &'s str
    where
        'arena: 's,
    {
        self.name
            .resolve(self.span.end - self.name.raw_len as u32, source)
    }
}

// No `size_of` guards on the hot AST enums: the arena layout deliberately favors
// traversal locality over node size, keeping recursive children that the parser
// reads constantly inline (`Expression`/`Statement`/`TSType` fields and the fat
// variants) rather than arena-boxing them for a smaller enum. Boxing them shrank
// the slice element but added a pointer-chase on hot format-read paths that cost
// more than the density win, so the inline form stands.
