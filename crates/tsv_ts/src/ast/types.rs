//! TypeScript type system nodes
//!
//! Contains all TS type definitions: `TSType` enum, type annotations,
//! type literals, type operators, and related constructs.

use crate::lexer::KeywordKind;
use tsv_lang::Span;

use super::{
    CtorTypeId, Deco, Expr, ExprId, IdentName, Identifier, ImportTypeId, IndexParam, InferTypeId,
    Literal, MethodKind, QualNameId, Quasi, Run, TyId, TypeMember, TypeParam, UnaryExpression,
};

/// TypeScript type annotation node (e.g., `: number` in `const a: number = 5`)
///
/// Represents the full type annotation including the colon. The span covers the
/// entire annotation (`: number`), while the inner type covers just the type (`number`).
#[derive(Debug, Clone)]
pub struct TSTypeAnnotation {
    pub type_annotation: TyId,
    pub span: Span,
}

/// TypeScript type expression
///
/// Represents the various types in TypeScript's type system. Currently only
/// primitive keyword types are implemented. Complex types (unions, intersections,
/// generics, etc.) will be added incrementally.
///
/// Three variants are stored rather than inline — `Import`, `Constructor` and `Infer`,
/// each a row of its own payload store named by id — for the density reason `Statement`
/// and `Expression` take: each is wide enough to set the enum's size on its own and rare
/// enough that the extra row is free, while the width is paid by every row of the type
/// store. Each carries its span inline beside the id, so [`TSType::span`] reads every
/// variant without leaving the row.
/// A type-store row is the only place a `TSType` lives: no AST field holds one by value — every
/// holder, lists included (union / intersection members, tuple elements, type arguments,
/// template-literal types, as `Run<TyId>`), names by its [`TyId`] the row the type parser's
/// precedence ladder already pushed, since each level returns one (see `Parser::parse_type`).
/// The widest inline variants are `TSTypeReference` / `TSTypeQuery` / `TSMappedType` /
/// `TSFunctionType`, and `TSTypeReference` is the common case, so the ladder stops there.
#[derive(Debug, Clone)]
pub enum TSType {
    /// Primitive type keywords (number, string, boolean, etc.)
    Keyword(TSKeywordType),
    /// Literal types (template literals, string literals, number literals, etc.)
    Literal(TSLiteralType),
    /// Array types (number[], string[], etc.)
    Array(TSArrayType),
    /// Union types: `A | B | C`
    Union(TSUnionType),
    /// Intersection types: `A & B & C`
    Intersection(TSIntersectionType),
    /// Type references: `SomeType`, `Array<T>`
    TypeReference(TSTypeReference),
    /// Object/type literal: `{ prop: T }`
    TypeLiteral(TSTypeLiteral),
    /// Function types: `(x: T) => U`
    Function(TSFunctionType),
    /// Constructor types: `new () => T` or `abstract new <T>() => T`
    Constructor(Span, CtorTypeId),
    /// Tuple types: `[T, U]`
    Tuple(TSTupleType),
    /// Parenthesized types: `(T)`
    Parenthesized(TSParenthesizedType),
    /// Type predicates: `x is T` or `asserts x is T`
    TypePredicate(TSTypePredicate),
    /// Conditional types: `T extends U ? V : W`
    Conditional(TSConditionalType),
    /// Mapped types: `{ [K in keyof T]: V }`
    Mapped(TSMappedType),
    /// Type operators: `keyof T`, `unique symbol`, `readonly T`
    TypeOperator(TSTypeOperator),
    /// Import types: `import('module')` or `import('module').Foo<T>`
    Import(Span, ImportTypeId),
    /// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`
    TypeQuery(TSTypeQuery),
    /// Indexed access types: `T[K]`, `Obj["key"]`, `T[keyof T]`
    IndexedAccess(TSIndexedAccessType),
    /// Rest type in tuples: `...T`
    Rest(TSRestType),
    /// Optional type in tuples: `T?`
    Optional(TSOptionalType),
    /// Named tuple member: `label: T` or `label?: T`
    NamedTupleMember(TSNamedTupleMember),
    /// Infer type: `infer U` (in conditional types)
    Infer(Span, InferTypeId),
    /// This type: `this` (in type position)
    ThisType(TSThisType),
}

impl TSType {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            TSType::Keyword(kw) => kw.span,
            TSType::Literal(lit) => lit.span(),
            TSType::Array(arr) => arr.span,
            TSType::Union(u) => u.span,
            TSType::Intersection(i) => i.span,
            TSType::TypeReference(r) => r.span,
            TSType::TypeLiteral(t) => t.span,
            TSType::Function(f) => f.span,
            TSType::Constructor(span, _) => *span,
            TSType::Tuple(t) => t.span,
            TSType::Parenthesized(p) => p.span,
            TSType::TypePredicate(p) => p.span,
            TSType::Conditional(c) => c.span,
            TSType::Mapped(m) => m.span,
            TSType::TypeOperator(o) => o.span,
            TSType::Import(span, _) => *span,
            TSType::TypeQuery(q) => q.span,
            TSType::IndexedAccess(i) => i.span,
            TSType::Rest(r) => r.span,
            TSType::Optional(o) => o.span,
            TSType::NamedTupleMember(n) => n.span,
            TSType::Infer(span, _) => *span,
            TSType::ThisType(t) => t.span,
        }
    }
}

/// TypeScript array type: `number[]`, `string[]`, etc.
#[derive(Debug, Clone)]
pub struct TSArrayType {
    /// The element type of the array
    pub element_type: TyId,
    pub span: Span,
}

/// TypeScript indexed access type: `T[K]`, `Obj["key"]`, `T[keyof T]`
#[derive(Debug, Clone)]
pub struct TSIndexedAccessType {
    /// The object type being indexed
    pub object_type: TyId,
    /// The index type
    pub index_type: TyId,
    pub span: Span,
}

/// TypeScript primitive type keyword
///
/// Compact representation using a kind enum + span.
/// Memory: 1 byte (kind) + padding + 8 bytes (span) = 12 bytes total
#[derive(Debug, Clone, Copy)]
pub struct TSKeywordType {
    pub kind: TSKeywordKind,
    pub span: Span,
}

impl TSKeywordType {
    #[inline]
    pub const fn new(kind: TSKeywordKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Enumeration of TypeScript primitive type keywords
///
/// Compact representation (1 byte) for all built-in type keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TSKeywordKind {
    Number = 0,
    String = 1,
    Boolean = 2,
    Any = 3,
    Void = 4,
    Undefined = 5,
    Null = 6,
    Never = 7,
    Unknown = 8,
    Object = 9,
    Symbol = 10,
    BigInt = 11,
    True = 12,
    False = 13,
}

impl TSKeywordKind {
    /// Returns the string representation of this type keyword
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            TSKeywordKind::Number => "number",
            TSKeywordKind::String => "string",
            TSKeywordKind::Boolean => "boolean",
            TSKeywordKind::Any => "any",
            TSKeywordKind::Void => "void",
            TSKeywordKind::Undefined => "undefined",
            TSKeywordKind::Null => "null",
            TSKeywordKind::Never => "never",
            TSKeywordKind::Unknown => "unknown",
            TSKeywordKind::Object => "object",
            TSKeywordKind::Symbol => "symbol",
            TSKeywordKind::BigInt => "bigint",
            TSKeywordKind::True => "true",
            TSKeywordKind::False => "false",
        }
    }

    /// Returns the AST node type name for JSON serialization
    #[inline]
    pub const fn node_type_name(self) -> &'static str {
        match self {
            TSKeywordKind::Number => "TSNumberKeyword",
            TSKeywordKind::String => "TSStringKeyword",
            TSKeywordKind::Boolean => "TSBooleanKeyword",
            TSKeywordKind::Any => "TSAnyKeyword",
            TSKeywordKind::Void => "TSVoidKeyword",
            TSKeywordKind::Undefined => "TSUndefinedKeyword",
            TSKeywordKind::Null => "TSNullKeyword",
            TSKeywordKind::Never => "TSNeverKeyword",
            TSKeywordKind::Unknown => "TSUnknownKeyword",
            TSKeywordKind::Object => "TSObjectKeyword",
            TSKeywordKind::Symbol => "TSSymbolKeyword",
            TSKeywordKind::BigInt => "TSBigIntKeyword",
            TSKeywordKind::True => "TSLiteralType",
            TSKeywordKind::False => "TSLiteralType",
        }
    }

    /// Convert from lexer KeywordKind to AST TSKeywordKind
    /// Returns None for non-type keywords (const, let, var, etc.)
    #[inline]
    pub fn from_lexer_keyword(kw: KeywordKind) -> Option<Self> {
        match kw {
            KeywordKind::Number => Some(TSKeywordKind::Number),
            KeywordKind::String => Some(TSKeywordKind::String),
            KeywordKind::Boolean => Some(TSKeywordKind::Boolean),
            KeywordKind::Any => Some(TSKeywordKind::Any),
            KeywordKind::Void => Some(TSKeywordKind::Void),
            KeywordKind::Undefined => Some(TSKeywordKind::Undefined),
            KeywordKind::Null => Some(TSKeywordKind::Null),
            KeywordKind::Never => Some(TSKeywordKind::Never),
            KeywordKind::Unknown => Some(TSKeywordKind::Unknown),
            KeywordKind::Object => Some(TSKeywordKind::Object),
            KeywordKind::Symbol => Some(TSKeywordKind::Symbol),
            KeywordKind::Bigint => Some(TSKeywordKind::BigInt),
            KeywordKind::True => Some(TSKeywordKind::True),
            KeywordKind::False => Some(TSKeywordKind::False),
            // Non-type keywords
            KeywordKind::Const
            | KeywordKind::Let
            | KeywordKind::Var
            | KeywordKind::New
            | KeywordKind::Instanceof
            | KeywordKind::In
            | KeywordKind::Return
            | KeywordKind::Function
            | KeywordKind::Class
            | KeywordKind::Typeof
            | KeywordKind::Delete
            | KeywordKind::Async
            | KeywordKind::Await
            | KeywordKind::This
            | KeywordKind::Super
            | KeywordKind::Extends
            | KeywordKind::Export
            // Control flow keywords
            | KeywordKind::If
            | KeywordKind::Else
            | KeywordKind::For
            | KeywordKind::While
            | KeywordKind::Do
            | KeywordKind::Switch
            | KeywordKind::Case
            | KeywordKind::Default
            | KeywordKind::Break
            | KeywordKind::Continue
            | KeywordKind::Try
            | KeywordKind::Catch
            | KeywordKind::Finally
            | KeywordKind::Throw
            // Module keywords
            | KeywordKind::Import
            | KeywordKind::From
            | KeywordKind::As
            | KeywordKind::Satisfies
            // Generator keywords
            | KeywordKind::Yield
            // Declaration keywords (not type keywords)
            | KeywordKind::Enum
            | KeywordKind::Debugger
            | KeywordKind::With => None,
        }
    }
}

/// TypeScript type alias declaration: `type X = T`
///
/// Represents a type alias that creates a new name for an existing type.
/// Supports template literal types: `type X = \`hello\``
#[derive(Debug, Clone)]
pub struct TSTypeAliasDeclaration {
    pub id: Identifier,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub type_annotation: TyId,
    pub declare: bool,
}

/// TypeScript literal type: wraps a literal value as a type
///
/// Used for template literal types: `type X = \`hello\``
/// Also supports string, number, boolean, null, undefined literals as types.
#[derive(Debug, Clone)]
pub enum TSLiteralType {
    TemplateLiteral(TemplateLiteralType),
    /// String literal type: `"hello"`, `'world'`
    String(Literal),
    /// Number literal type: `1`, `42.5`
    Number(Literal),
    /// BigInt literal type: `1n`, `100n`
    BigInt(Literal),
    /// Unary expression for negative numbers: `-1`, `-42n`
    UnaryExpression(UnaryExpression),
}

impl TSLiteralType {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            TSLiteralType::TemplateLiteral(t) => t.span,
            TSLiteralType::String(lit) => lit.span,
            TSLiteralType::Number(lit) => lit.span,
            TSLiteralType::BigInt(lit) => lit.span,
            TSLiteralType::UnaryExpression(unary) => unary.span,
        }
    }
}

/// Template literal used as a type: `\`hello ${string} world\``
///
/// Similar to TemplateLiteral but interpolations contain types, not expressions.
/// Used in TypeScript template literal types.
#[derive(Debug, Clone)]
pub struct TemplateLiteralType {
    pub quasis: Run<Quasi>,
    pub types: Run<TyId>,
    pub span: Span,
}

//
// TypeScript Type Nodes
//

/// Union type: `A | B | C`
#[derive(Debug, Clone)]
pub struct TSUnionType {
    pub types: Run<TyId>,
    pub span: Span,
}

/// Intersection type: `A & B & C`
#[derive(Debug, Clone)]
pub struct TSIntersectionType {
    pub types: Run<TyId>,
    pub span: Span,
}

/// Type reference: `SomeType` or `Array<T>`
#[derive(Debug, Clone)]
pub struct TSTypeReference {
    pub type_name: TSEntityName,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}

/// Entity name: `Foo` or `Foo.Bar.Baz`
#[derive(Debug, Clone)]
pub enum TSEntityName {
    Identifier(Identifier),
    /// The qualified name's span inline beside its id, so [`TSEntityName::span`] reads it
    /// without the hop.
    QualifiedName(Span, QualNameId),
}

impl TSEntityName {
    pub fn span(&self) -> Span {
        match self {
            TSEntityName::Identifier(id) => id.span,
            TSEntityName::QualifiedName(span, _) => *span,
        }
    }
}

/// Qualified name: `Foo.Bar`
#[derive(Debug, Clone)]
pub struct TSQualifiedName {
    pub left: TSEntityName,
    pub right: Identifier,
    pub span: Span,
}

/// Type parameter instantiation: `<T, U>` (for type arguments)
#[derive(Debug, Clone)]
pub struct TSTypeParameterInstantiation {
    pub params: Run<TyId>,
    pub span: Span,
}

/// Type parameter declaration: `<T, U>` (for declaring type parameters)
#[derive(Debug, Clone)]
pub struct TSTypeParameterDeclaration {
    pub params: Run<TypeParam>,
    /// Position of trailing comma if present (e.g., `<T,>`)
    pub trailing_comma: Option<u32>,
    pub span: Span,
}

/// A type parameter's modifier keyword — `const` (TS 5.0) and the `in`/`out`
/// variance pair (TS 4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TSTypeParameterModifier {
    Const,
    In,
    Out,
}

impl TSTypeParameterModifier {
    /// The wire's key for this modifier — also its source spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            TSTypeParameterModifier::Const => "const",
            TSTypeParameterModifier::In => "in",
            TSTypeParameterModifier::Out => "out",
        }
    }
}

/// The modifiers a type parameter carries, **in source order**.
///
/// ⚠️ The order is observable OUTPUT, not bookkeeping, and the two readers ask
/// opposite questions of it. acorn-typescript stamps each flag as it consumes the
/// keyword, so `<in const T>` emits `in` before `const` on the wire while
/// `<const in T>` emits it after — the WRITER must walk [`Self::iter`]. The PRINTER
/// is the reverse: it emits the canonical `const in out` however the source spells
/// it (as prettier does), so it asks [`Self::contains`] three times and never sees
/// the order. Storing a set plus a separate order field would give those two
/// answers room to disagree; one ordered value cannot.
#[derive(Debug, Clone, Copy, Default)]
pub struct TSTypeParameterModifiers {
    /// At most one of each of the three keywords, so three slots always suffice.
    order: [Option<TSTypeParameterModifier>; 3],
}

impl TSTypeParameterModifiers {
    /// Append `modifier`, or return `false` if it is already present — a repeat
    /// (`<const const T>`) is a syntax error the caller reports.
    pub fn push(&mut self, modifier: TSTypeParameterModifier) -> bool {
        if self.contains(modifier) {
            return false;
        }
        for slot in &mut self.order {
            if slot.is_none() {
                *slot = Some(modifier);
                return true;
            }
        }
        // Unreachable: there are three distinct modifiers and three slots, and the
        // duplicate check above already returned, so at most two slots are filled.
        // Reporting "already present" is the harmless reading if it ever were reached:
        // the caller rejects on a `false`, which is what a fourth modifier keyword
        // would deserve anyway.
        false
    }

    /// Whether `modifier` is present, order-blind — the printer's question.
    pub fn contains(self, modifier: TSTypeParameterModifier) -> bool {
        self.order.iter().flatten().any(|&m| m == modifier)
    }

    /// The modifiers in the order the author wrote them — the wire's question.
    pub fn iter(self) -> impl Iterator<Item = TSTypeParameterModifier> {
        self.order.into_iter().flatten()
    }
}

/// Single type parameter: `T`, `T extends U`, or `T extends U = V`
/// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
#[derive(Debug, Clone)]
pub struct TSTypeParameter {
    pub name: Identifier,
    pub constraint: Option<TyId>,
    pub default: Option<TyId>,
    /// `const` / `in` / `out`, in source order — see [`TSTypeParameterModifiers`].
    pub modifiers: TSTypeParameterModifiers,
    pub span: Span,
}

/// Type literal (object type): `{ prop: T; method(): U }`
#[derive(Debug, Clone)]
pub struct TSTypeLiteral {
    pub members: Run<TypeMember>,
    pub span: Span,
}

/// Type element - member of a type literal or interface
#[derive(Debug, Clone)]
pub enum TSTypeElement {
    PropertySignature(TSPropertySignature),
    MethodSignature(TSMethodSignature),
    CallSignature(TSCallSignatureDeclaration),
    ConstructSignature(TSConstructSignatureDeclaration),
    IndexSignature(TSIndexSignature),
}

impl TSTypeElement {
    pub fn span(&self) -> Span {
        match self {
            TSTypeElement::PropertySignature(p) => p.span,
            TSTypeElement::MethodSignature(m) => m.span,
            TSTypeElement::CallSignature(c) => c.span,
            TSTypeElement::ConstructSignature(c) => c.span,
            TSTypeElement::IndexSignature(i) => i.span,
        }
    }

    /// Get the end position of the member's content (before any trailing separator).
    ///
    /// The `span().end` may include a trailing `;` or `,` separator (to match acorn's
    /// output). This method returns the end of the actual content, which is needed for
    /// comment detection in the printer.
    pub fn content_end(&self, source: &str) -> u32 {
        let end = self.span().end;
        if end > 0 {
            let last_byte = source.as_bytes()[(end - 1) as usize];
            if last_byte == b';' || last_byte == b',' {
                return end - 1;
            }
        }
        end
    }

    /// Extend the element's span end to include a trailing separator (`;` or `,`)
    pub fn extend_span_to(&mut self, new_end: u32) {
        match self {
            TSTypeElement::PropertySignature(p) => p.span = Span::new(p.span.start, new_end),
            TSTypeElement::MethodSignature(m) => m.span = Span::new(m.span.start, new_end),
            TSTypeElement::CallSignature(c) => c.span = Span::new(c.span.start, new_end),
            TSTypeElement::ConstructSignature(c) => c.span = Span::new(c.span.start, new_end),
            TSTypeElement::IndexSignature(i) => i.span = Span::new(i.span.start, new_end),
        }
    }
}

/// Property signature: `prop: T` or `prop?: T` or `readonly prop: T`
#[derive(Debug, Clone)]
pub struct TSPropertySignature {
    pub key: ExprId,
    pub computed: bool,
    pub optional: bool,
    pub readonly: bool,
    pub type_annotation: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Method signature: `method(): T` or `method<T>(x: T): T` or `get x(): T` or `set x(v: T)`
#[derive(Debug, Clone)]
pub struct TSMethodSignature {
    pub key: ExprId,
    pub computed: bool,
    pub optional: bool,
    /// A `readonly` modifier (`readonly x(): void`): tsc's parser accepts it on a method
    /// signature and its checker rejects it (TS1024), so it is kept rather than dropped.
    pub readonly: bool,
    /// Method kind: method, get, or set (for accessor signatures in type literals)
    pub kind: MethodKind,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Run<Expr>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Call signature: `(): T` or `<T>(): T` or `(x: A): T`
#[derive(Debug, Clone)]
pub struct TSCallSignatureDeclaration {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Run<Expr>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Construct signature: `new (): T` or `new <T>(): T` or `new (x: A): T`
#[derive(Debug, Clone)]
pub struct TSConstructSignatureDeclaration {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Run<Expr>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Index signature: `[key: string]: T`, or typeless `[key: string]` (the value
/// type annotation is optional — a typeless form is a deferred tsc checker error
/// (TS1021), not a parse error; acorn parses it and omits `typeAnnotation`).
#[derive(Debug, Clone)]
pub struct TSIndexSignature {
    pub parameters: Run<IndexParam>,
    pub type_annotation: Option<TSTypeAnnotation>,
    /// Decorators in front of a class index signature (`@a [k: string]: T;`).
    /// No decorator grammar reaches one (tsc's checker reports TS1206); acorn keeps
    /// the list and tsv defers the error. Always `None` in a type literal or interface.
    pub decorators: Option<Run<Deco>>,
    pub is_static: bool,
    pub readonly: bool,
    pub span: Span,
}

/// Function type: `(x: T) => U` or `<T>(x: T) => U`
#[derive(Debug, Clone)]
pub struct TSFunctionType {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Run<Expr>,
    // Inline by value rather than behind a reference: `TSTypeAnnotation` is a span and the id
    // of its type's row, held inline everywhere else (`Option<TSTypeAnnotation>`), so a
    // reference here would put a pointer-chase ahead of the store read on the format read
    // path.
    pub return_type: TSTypeAnnotation,
    pub span: Span,
}

/// Constructor type: `new () => T` or `abstract new <T>() => T`
#[derive(Debug, Clone)]
pub struct TSConstructorType {
    pub abstract_: bool,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Run<Expr>,
    // Inline by value — see `TSFunctionType.return_type`.
    pub return_type: TSTypeAnnotation,
    pub span: Span,
}

/// Tuple type: `[T, U, V]`
#[derive(Debug, Clone)]
pub struct TSTupleType {
    pub element_types: Run<TyId>,
    pub span: Span,
}

/// Rest type in tuples: `...T`
#[derive(Debug, Clone)]
pub struct TSRestType {
    /// The type being spread
    pub type_annotation: TyId,
    pub span: Span,
}

/// Optional type in tuples: `T?`
#[derive(Debug, Clone)]
pub struct TSOptionalType {
    /// The type that is optional
    pub type_annotation: TyId,
    pub span: Span,
}

/// Named tuple member: `label: T` or `label?: T`
#[derive(Debug, Clone)]
pub struct TSNamedTupleMember {
    /// The label identifier
    pub label: Identifier,
    /// The element type
    pub element_type: TyId,
    /// Whether this element is optional (label?: T)
    pub optional: bool,
    pub span: Span,
}

/// Infer type: `infer U` (in conditional types)
///
/// Used in the extends clause of conditional types to introduce a type variable
/// that can be inferred from the matched type.
#[derive(Debug, Clone)]
pub struct TSInferType {
    /// The type parameter being inferred
    pub type_parameter: TSTypeParameter,
    pub span: Span,
}

/// This type: `this` used as a type
#[derive(Debug, Clone)]
pub struct TSThisType {
    pub span: Span,
}

/// Parenthesized type: `(T)`
#[derive(Debug, Clone)]
pub struct TSParenthesizedType {
    pub type_annotation: TyId,
    pub span: Span,
}

/// Conditional type: `T extends U ? V : W`
#[derive(Debug, Clone)]
pub struct TSConditionalType {
    pub check_type: TyId,
    pub extends_type: TyId,
    pub true_type: TyId,
    pub false_type: TyId,
    pub span: Span,
}

/// Type predicate: `x is T` or `asserts x is T`
///
/// Used for type guards and assertion functions.
#[derive(Debug, Clone)]
pub struct TSTypePredicate {
    /// The parameter name being checked (e.g., `x` in `x is string`)
    pub parameter_name: Identifier,
    /// The type being asserted (e.g., `string` in `x is string`)
    /// None for `asserts x` without `is T`
    pub type_annotation: Option<TyId>,
    /// Whether this is an assertion predicate (`asserts x is T`)
    pub asserts: bool,
    pub span: Span,
}

/// Mapped type: `{ [K in keyof T]: V }`
///
/// Transforms properties from one type to another.
/// Modifier for mapped type `readonly` and `?`: bare, `+`, or `-`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TSMappedTypeModifier {
    /// Bare modifier: `readonly` or `?`
    True,
    /// Explicit plus: `+readonly` or `+?`
    Plus,
    /// Explicit minus: `-readonly` or `-?`
    Minus,
}

#[derive(Debug, Clone)]
pub struct TSMappedType {
    /// The type parameter with constraint: `K in keyof T`
    pub type_parameter: TSMappedTypeParameter,
    /// Optional key remapping: `as NewK`
    pub name_type: Option<TyId>,
    /// The value type
    pub type_annotation: Option<TyId>,
    /// Readonly modifier: `readonly`, `+readonly`, `-readonly`, or absent
    pub readonly: Option<TSMappedTypeModifier>,
    /// Optional modifier: `?`, `+?`, `-?`, or absent
    pub optional: Option<TSMappedTypeModifier>,
    pub span: Span,
}

/// Type parameter in a mapped type: `K in keyof T`
#[derive(Debug, Clone)]
pub struct TSMappedTypeParameter {
    /// The parameter name (just the string, not an `Identifier`) — span-identity
    /// with the arena-string escaped fallback, like every identifier name. `span`
    /// covers exactly the name token (`K`; the constraint is a sibling field),
    /// so the verbatim name is the leading `raw_len` bytes at `span.start`.
    pub name: IdentName,
    /// The constraint type (e.g., `keyof T`)
    pub constraint: TyId,
    pub span: Span,
}

/// Type operator: `keyof T`, `unique symbol`, `readonly T`
#[derive(Debug, Clone)]
pub struct TSTypeOperator {
    /// The operator: "keyof", "unique", "readonly"
    pub operator: TSTypeOperatorKind,
    /// The type being operated on
    pub type_annotation: TyId,
    pub span: Span,
}

/// Type operator kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TSTypeOperatorKind {
    Keyof,
    Unique,
    Readonly,
}

impl TSTypeOperatorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            TSTypeOperatorKind::Keyof => "keyof",
            TSTypeOperatorKind::Unique => "unique",
            TSTypeOperatorKind::Readonly => "readonly",
        }
    }
}

/// Import type: `import('module')` or `import('module', {with: {...}}).Qualifier<T>`
#[derive(Debug, Clone)]
pub struct TSImportType {
    /// The module specifier (string literal)
    pub argument: Literal,
    /// Optional options object: `{with: {type: 'json'}}`
    pub options: Option<ExprId>,
    /// Optional qualifier: `.Foo` or `.Foo.Bar` after the import
    pub qualifier: Option<TSEntityName>,
    /// Optional type arguments: `<T, U>`
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}

/// Type query expression name: Identifier, QualifiedName, or ImportType
///
/// The `exprName` field of `TSTypeQuery` can be:
/// - `Identifier` for `typeof x`
/// - `TSQualifiedName` for `typeof Foo.bar`
/// - `TSImportType` for `typeof import("module")`
#[derive(Debug, Clone)]
pub enum TSTypeQueryExprName {
    /// Entity name (Identifier or QualifiedName): `typeof x`, `typeof Foo.bar`
    EntityName(TSEntityName),
    /// Import type: `typeof import("module")`, its span inline beside its id.
    Import(Span, ImportTypeId),
}

impl TSTypeQueryExprName {
    pub fn span(&self) -> Span {
        match self {
            TSTypeQueryExprName::EntityName(e) => e.span(),
            TSTypeQueryExprName::Import(span, _) => *span,
        }
    }
}

/// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`, `typeof Array<T>`
///
/// Gets the type of a value expression.
#[derive(Debug, Clone)]
pub struct TSTypeQuery {
    /// The expression whose type is being queried
    pub expr_name: TSTypeQueryExprName,
    /// Optional type arguments: `<T, U>` (e.g., `typeof Array<string>`)
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}
