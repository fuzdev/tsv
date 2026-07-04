//! TypeScript type system nodes
//!
//! Contains all TS type definitions: `TSType` enum, type annotations,
//! type literals, type operators, and related constructs.

use crate::lexer::KeywordKind;
use tsv_lang::Span;

use super::{
    Expression, IdentName, Identifier, Literal, MethodKind, TemplateElement, UnaryExpression,
};

/// TypeScript type annotation node (e.g., `: number` in `const a: number = 5`)
///
/// Represents the full type annotation including the colon. The span covers the
/// entire annotation (`: number`), while the inner type covers just the type (`number`).
#[derive(Debug, Clone)]
pub struct TSTypeAnnotation<'arena> {
    pub type_annotation: &'arena TSType<'arena>,
    pub span: Span,
}

/// TypeScript type expression
///
/// Represents the various types in TypeScript's type system. Currently only
/// primitive keyword types are implemented. Complex types (unions, intersections,
/// generics, etc.) will be added incrementally.
#[derive(Debug, Clone)]
pub enum TSType<'arena> {
    /// Primitive type keywords (number, string, boolean, etc.)
    Keyword(TSKeywordType),
    /// Literal types (template literals, string literals, number literals, etc.)
    Literal(TSLiteralType<'arena>),
    /// Array types (number[], string[], etc.)
    Array(TSArrayType<'arena>),
    /// Union types: `A | B | C`
    Union(TSUnionType<'arena>),
    /// Intersection types: `A & B & C`
    Intersection(TSIntersectionType<'arena>),
    /// Type references: `SomeType`, `Array<T>`
    TypeReference(TSTypeReference<'arena>),
    /// Object/type literal: `{ prop: T }`
    TypeLiteral(TSTypeLiteral<'arena>),
    /// Function types: `(x: T) => U`
    Function(TSFunctionType<'arena>),
    /// Constructor types: `new () => T` or `abstract new <T>() => T`
    Constructor(TSConstructorType<'arena>),
    /// Tuple types: `[T, U]`
    Tuple(TSTupleType<'arena>),
    /// Parenthesized types: `(T)`
    Parenthesized(TSParenthesizedType<'arena>),
    /// Type predicates: `x is T` or `asserts x is T`
    TypePredicate(TSTypePredicate<'arena>),
    /// Conditional types: `T extends U ? V : W`
    Conditional(TSConditionalType<'arena>),
    /// Mapped types: `{ [K in keyof T]: V }`
    Mapped(TSMappedType<'arena>),
    /// Type operators: `keyof T`, `unique symbol`, `readonly T`
    TypeOperator(TSTypeOperator<'arena>),
    /// Import types: `import('module')` or `import('module').Foo<T>`
    Import(TSImportType<'arena>),
    /// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`
    TypeQuery(TSTypeQuery<'arena>),
    /// Indexed access types: `T[K]`, `Obj["key"]`, `T[keyof T]`
    IndexedAccess(TSIndexedAccessType<'arena>),
    /// Rest type in tuples: `...T`
    Rest(TSRestType<'arena>),
    /// Optional type in tuples: `T?`
    Optional(TSOptionalType<'arena>),
    /// Named tuple member: `label: T` or `label?: T`
    NamedTupleMember(TSNamedTupleMember<'arena>),
    /// Infer type: `infer U` (in conditional types)
    Infer(TSInferType<'arena>),
    /// This type: `this` (in type position)
    ThisType(TSThisType),
}

impl<'arena> TSType<'arena> {
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
            TSType::Constructor(c) => c.span,
            TSType::Tuple(t) => t.span,
            TSType::Parenthesized(p) => p.span,
            TSType::TypePredicate(p) => p.span,
            TSType::Conditional(c) => c.span,
            TSType::Mapped(m) => m.span,
            TSType::TypeOperator(o) => o.span,
            TSType::Import(i) => i.span,
            TSType::TypeQuery(q) => q.span,
            TSType::IndexedAccess(i) => i.span,
            TSType::Rest(r) => r.span,
            TSType::Optional(o) => o.span,
            TSType::NamedTupleMember(n) => n.span,
            TSType::Infer(i) => i.span,
            TSType::ThisType(t) => t.span,
        }
    }
}

/// TypeScript array type: `number[]`, `string[]`, etc.
#[derive(Debug, Clone)]
pub struct TSArrayType<'arena> {
    /// The element type of the array
    pub element_type: &'arena TSType<'arena>,
    pub span: Span,
}

/// TypeScript indexed access type: `T[K]`, `Obj["key"]`, `T[keyof T]`
#[derive(Debug, Clone)]
pub struct TSIndexedAccessType<'arena> {
    /// The object type being indexed
    pub object_type: &'arena TSType<'arena>,
    /// The index type
    pub index_type: &'arena TSType<'arena>,
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
            | KeywordKind::Debugger => None,
        }
    }
}

/// TypeScript type alias declaration: `type X = T`
///
/// Represents a type alias that creates a new name for an existing type.
/// Supports template literal types: `type X = \`hello\``
#[derive(Debug, Clone)]
pub struct TSTypeAliasDeclaration<'arena> {
    pub id: Identifier<'arena>,
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    pub type_annotation: TSType<'arena>,
    pub declare: bool,
    pub span: Span,
}

/// TypeScript literal type: wraps a literal value as a type
///
/// Used for template literal types: `type X = \`hello\``
/// Also supports string, number, boolean, null, undefined literals as types.
#[derive(Debug, Clone)]
pub enum TSLiteralType<'arena> {
    TemplateLiteral(TemplateLiteralType<'arena>),
    /// String literal type: `"hello"`, `'world'`
    String(Literal<'arena>),
    /// Number literal type: `1`, `42.5`
    Number(Literal<'arena>),
    /// BigInt literal type: `1n`, `100n`
    BigInt(Literal<'arena>),
    /// Unary expression for negative numbers: `-1`, `-42n`
    UnaryExpression(UnaryExpression<'arena>),
}

impl<'arena> TSLiteralType<'arena> {
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
pub struct TemplateLiteralType<'arena> {
    pub quasis: &'arena [TemplateElement<'arena>],
    pub types: &'arena [TSType<'arena>],
    pub span: Span,
}

//
// TypeScript Type Nodes
//

/// Union type: `A | B | C`
#[derive(Debug, Clone)]
pub struct TSUnionType<'arena> {
    pub types: &'arena [TSType<'arena>],
    pub span: Span,
}

/// Intersection type: `A & B & C`
#[derive(Debug, Clone)]
pub struct TSIntersectionType<'arena> {
    pub types: &'arena [TSType<'arena>],
    pub span: Span,
}

/// Type reference: `SomeType` or `Array<T>`
#[derive(Debug, Clone)]
pub struct TSTypeReference<'arena> {
    pub type_name: TSEntityName<'arena>,
    pub type_arguments: Option<TSTypeParameterInstantiation<'arena>>,
    pub span: Span,
}

/// Entity name: `Foo` or `Foo.Bar.Baz`
#[derive(Debug, Clone)]
pub enum TSEntityName<'arena> {
    Identifier(Identifier<'arena>),
    QualifiedName(&'arena TSQualifiedName<'arena>),
}

impl<'arena> TSEntityName<'arena> {
    pub fn span(&self) -> Span {
        match self {
            TSEntityName::Identifier(id) => id.span,
            TSEntityName::QualifiedName(qn) => qn.span,
        }
    }
}

/// Qualified name: `Foo.Bar`
#[derive(Debug, Clone)]
pub struct TSQualifiedName<'arena> {
    pub left: TSEntityName<'arena>,
    pub right: Identifier<'arena>,
    pub span: Span,
}

/// Type parameter instantiation: `<T, U>` (for type arguments)
#[derive(Debug, Clone)]
pub struct TSTypeParameterInstantiation<'arena> {
    pub params: &'arena [TSType<'arena>],
    pub span: Span,
}

/// Type parameter declaration: `<T, U>` (for declaring type parameters)
#[derive(Debug, Clone)]
pub struct TSTypeParameterDeclaration<'arena> {
    pub params: &'arena [TSTypeParameter<'arena>],
    /// Position of trailing comma if present (e.g., `<T,>`)
    pub trailing_comma: Option<u32>,
    pub span: Span,
}

/// Single type parameter: `T`, `T extends U`, or `T extends U = V`
/// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
#[derive(Debug, Clone)]
pub struct TSTypeParameter<'arena> {
    pub name: Identifier<'arena>,
    pub constraint: Option<&'arena TSType<'arena>>,
    pub default: Option<&'arena TSType<'arena>>,
    /// `const` modifier (TS 5.0): `<const T>`
    pub is_const: bool,
    /// `in` variance modifier (TS 4.7): `<in T>`
    pub is_in: bool,
    /// `out` variance modifier (TS 4.7): `<out T>`
    pub is_out: bool,
    pub span: Span,
}

/// Type literal (object type): `{ prop: T; method(): U }`
#[derive(Debug, Clone)]
pub struct TSTypeLiteral<'arena> {
    pub members: &'arena [TSTypeElement<'arena>],
    pub span: Span,
}

/// Type element - member of a type literal or interface
#[derive(Debug, Clone)]
pub enum TSTypeElement<'arena> {
    PropertySignature(TSPropertySignature<'arena>),
    MethodSignature(TSMethodSignature<'arena>),
    CallSignature(TSCallSignatureDeclaration<'arena>),
    ConstructSignature(TSConstructSignatureDeclaration<'arena>),
    IndexSignature(TSIndexSignature<'arena>),
}

impl<'arena> TSTypeElement<'arena> {
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
pub struct TSPropertySignature<'arena> {
    pub key: Expression<'arena>,
    pub computed: bool,
    pub optional: bool,
    pub readonly: bool,
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    pub span: Span,
}

/// Method signature: `method(): T` or `method<T>(x: T): T` or `get x(): T` or `set x(v: T)`
#[derive(Debug, Clone)]
pub struct TSMethodSignature<'arena> {
    pub key: Expression<'arena>,
    pub computed: bool,
    pub optional: bool,
    /// Method kind: method, get, or set (for accessor signatures in type literals)
    pub kind: MethodKind,
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    pub params: &'arena [Expression<'arena>],
    pub return_type: Option<TSTypeAnnotation<'arena>>,
    pub span: Span,
}

/// Call signature: `(): T` or `<T>(): T` or `(x: A): T`
#[derive(Debug, Clone)]
pub struct TSCallSignatureDeclaration<'arena> {
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    pub params: &'arena [Expression<'arena>],
    pub return_type: Option<TSTypeAnnotation<'arena>>,
    pub span: Span,
}

/// Construct signature: `new (): T` or `new <T>(): T` or `new (x: A): T`
#[derive(Debug, Clone)]
pub struct TSConstructSignatureDeclaration<'arena> {
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    pub params: &'arena [Expression<'arena>],
    pub return_type: Option<TSTypeAnnotation<'arena>>,
    pub span: Span,
}

/// Index signature: `[key: string]: T`
#[derive(Debug, Clone)]
pub struct TSIndexSignature<'arena> {
    pub parameters: &'arena [Identifier<'arena>],
    pub type_annotation: TSTypeAnnotation<'arena>,
    pub is_static: bool,
    pub readonly: bool,
    pub span: Span,
}

/// Function type: `(x: T) => U` or `<T>(x: T) => U`
#[derive(Debug, Clone)]
pub struct TSFunctionType<'arena> {
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    pub params: &'arena [Expression<'arena>],
    // Inline by value (16 B) rather than `&'arena`: `TSTypeAnnotation` is a small
    // `Copy` wrapper, held inline everywhere else (`Option<TSTypeAnnotation>`), and
    // it already indirects its own `&'arena TSType` — so a `&'arena` here would be a
    // double indirection. Inline avoids the extra pointer-chase on the format read path.
    pub return_type: TSTypeAnnotation<'arena>,
    pub span: Span,
}

/// Constructor type: `new () => T` or `abstract new <T>() => T`
#[derive(Debug, Clone)]
pub struct TSConstructorType<'arena> {
    pub abstract_: bool,
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    pub params: &'arena [Expression<'arena>],
    // Inline by value — see `TSFunctionType.return_type`.
    pub return_type: TSTypeAnnotation<'arena>,
    pub span: Span,
}

/// Tuple type: `[T, U, V]`
#[derive(Debug, Clone)]
pub struct TSTupleType<'arena> {
    pub element_types: &'arena [TSType<'arena>],
    pub span: Span,
}

/// Rest type in tuples: `...T`
#[derive(Debug, Clone)]
pub struct TSRestType<'arena> {
    /// The type being spread
    pub type_annotation: &'arena TSType<'arena>,
    pub span: Span,
}

/// Optional type in tuples: `T?`
#[derive(Debug, Clone)]
pub struct TSOptionalType<'arena> {
    /// The type that is optional
    pub type_annotation: &'arena TSType<'arena>,
    pub span: Span,
}

/// Named tuple member: `label: T` or `label?: T`
#[derive(Debug, Clone)]
pub struct TSNamedTupleMember<'arena> {
    /// The label identifier
    pub label: Identifier<'arena>,
    /// The element type
    pub element_type: &'arena TSType<'arena>,
    /// Whether this element is optional (label?: T)
    pub optional: bool,
    pub span: Span,
}

/// Infer type: `infer U` (in conditional types)
///
/// Used in the extends clause of conditional types to introduce a type variable
/// that can be inferred from the matched type.
#[derive(Debug, Clone)]
pub struct TSInferType<'arena> {
    /// The type parameter being inferred
    pub type_parameter: TSTypeParameter<'arena>,
    pub span: Span,
}

/// This type: `this` used as a type
#[derive(Debug, Clone)]
pub struct TSThisType {
    pub span: Span,
}

/// Parenthesized type: `(T)`
#[derive(Debug, Clone)]
pub struct TSParenthesizedType<'arena> {
    pub type_annotation: &'arena TSType<'arena>,
    pub span: Span,
}

/// Conditional type: `T extends U ? V : W`
#[derive(Debug, Clone)]
pub struct TSConditionalType<'arena> {
    pub check_type: &'arena TSType<'arena>,
    pub extends_type: &'arena TSType<'arena>,
    pub true_type: &'arena TSType<'arena>,
    pub false_type: &'arena TSType<'arena>,
    pub span: Span,
}

/// Type predicate: `x is T` or `asserts x is T`
///
/// Used for type guards and assertion functions.
#[derive(Debug, Clone)]
pub struct TSTypePredicate<'arena> {
    /// The parameter name being checked (e.g., `x` in `x is string`)
    pub parameter_name: Identifier<'arena>,
    /// The type being asserted (e.g., `string` in `x is string`)
    /// None for `asserts x` without `is T`
    pub type_annotation: Option<&'arena TSType<'arena>>,
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
pub struct TSMappedType<'arena> {
    /// The type parameter with constraint: `K in keyof T`
    pub type_parameter: TSMappedTypeParameter<'arena>,
    /// Optional key remapping: `as NewK`
    pub name_type: Option<&'arena TSType<'arena>>,
    /// The value type
    pub type_annotation: Option<&'arena TSType<'arena>>,
    /// Readonly modifier: `readonly`, `+readonly`, `-readonly`, or absent
    pub readonly: Option<TSMappedTypeModifier>,
    /// Optional modifier: `?`, `+?`, `-?`, or absent
    pub optional: Option<TSMappedTypeModifier>,
    pub span: Span,
}

/// Type parameter in a mapped type: `K in keyof T`
#[derive(Debug, Clone)]
pub struct TSMappedTypeParameter<'arena> {
    /// The parameter name (just the string, not an `Identifier`) — span-identity
    /// with the escaped-interned fallback, like every identifier name. `span`
    /// covers exactly the name token (`K`; the constraint is a sibling field),
    /// so the verbatim name is the leading `raw_len` bytes at `span.start`.
    pub name: IdentName,
    /// The constraint type (e.g., `keyof T`)
    pub constraint: &'arena TSType<'arena>,
    pub span: Span,
}

/// Type operator: `keyof T`, `unique symbol`, `readonly T`
#[derive(Debug, Clone)]
pub struct TSTypeOperator<'arena> {
    /// The operator: "keyof", "unique", "readonly"
    pub operator: TSTypeOperatorKind,
    /// The type being operated on
    pub type_annotation: &'arena TSType<'arena>,
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
pub struct TSImportType<'arena> {
    /// The module specifier (string literal)
    pub argument: Literal<'arena>,
    /// Optional options object: `{with: {type: 'json'}}`
    pub options: Option<&'arena Expression<'arena>>,
    /// Optional qualifier: `.Foo` or `.Foo.Bar` after the import
    pub qualifier: Option<TSEntityName<'arena>>,
    /// Optional type arguments: `<T, U>`
    pub type_arguments: Option<TSTypeParameterInstantiation<'arena>>,
    pub span: Span,
}

/// Type query expression name: Identifier, QualifiedName, or ImportType
///
/// The `exprName` field of `TSTypeQuery` can be:
/// - `Identifier` for `typeof x`
/// - `TSQualifiedName` for `typeof Foo.bar`
/// - `TSImportType` for `typeof import("module")`
#[derive(Debug, Clone)]
pub enum TSTypeQueryExprName<'arena> {
    /// Entity name (Identifier or QualifiedName): `typeof x`, `typeof Foo.bar`
    EntityName(TSEntityName<'arena>),
    /// Import type: `typeof import("module")`
    Import(&'arena TSImportType<'arena>),
}

impl<'arena> TSTypeQueryExprName<'arena> {
    pub fn span(&self) -> Span {
        match self {
            TSTypeQueryExprName::EntityName(e) => e.span(),
            TSTypeQueryExprName::Import(i) => i.span,
        }
    }
}

/// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`, `typeof Array<T>`
///
/// Gets the type of a value expression.
#[derive(Debug, Clone)]
pub struct TSTypeQuery<'arena> {
    /// The expression whose type is being queried
    pub expr_name: TSTypeQueryExprName<'arena>,
    /// Optional type arguments: `<T, U>` (e.g., `typeof Array<string>`)
    pub type_arguments: Option<TSTypeParameterInstantiation<'arena>>,
    pub span: Span,
}
