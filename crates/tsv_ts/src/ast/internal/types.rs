//! TypeScript type system nodes
//!
//! Contains all TS type definitions: `TSType` enum, type annotations,
//! type literals, type operators, and related constructs.

use crate::lexer::KeywordKind;
use tsv_lang::Span;

use super::{Expression, Identifier, Literal, MethodKind, TemplateElement, UnaryExpression};

/// TypeScript type annotation node (e.g., `: number` in `const a: number = 5`)
///
/// Represents the full type annotation including the colon. The span covers the
/// entire annotation (`: number`), while the inner type covers just the type (`number`).
///
/// # Memory Layout
/// Uses `Box<TSType>` to avoid bloating `Identifier` size. The indirection is acceptable
/// since type annotations are relatively rare and accessed infrequently during traversal.
#[derive(Debug, Clone)]
pub struct TSTypeAnnotation {
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// TypeScript type expression
///
/// Represents the various types in TypeScript's type system. Currently only
/// primitive keyword types are implemented. Complex types (unions, intersections,
/// generics, etc.) will be added incrementally.
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
    Constructor(TSConstructorType),
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
    Import(TSImportType),
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
    Infer(TSInferType),
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
pub struct TSArrayType {
    /// The element type of the array
    pub element_type: Box<TSType>,
    pub span: Span,
}

/// TypeScript indexed access type: `T[K]`, `Obj["key"]`, `T[keyof T]`
#[derive(Debug, Clone)]
pub struct TSIndexedAccessType {
    /// The object type being indexed
    pub object_type: Box<TSType>,
    /// The index type
    pub index_type: Box<TSType>,
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
pub struct TSTypeAliasDeclaration {
    pub id: Identifier,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub type_annotation: TSType,
    pub declare: bool,
    pub span: Span,
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
    pub quasis: Vec<TemplateElement>,
    pub types: Vec<TSType>,
    pub span: Span,
}

//
// TypeScript Type Nodes
//

/// Union type: `A | B | C`
#[derive(Debug, Clone)]
pub struct TSUnionType {
    pub types: Vec<TSType>,
    pub span: Span,
}

/// Intersection type: `A & B & C`
#[derive(Debug, Clone)]
pub struct TSIntersectionType {
    pub types: Vec<TSType>,
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
    QualifiedName(Box<TSQualifiedName>),
}

impl TSEntityName {
    pub fn span(&self) -> Span {
        match self {
            TSEntityName::Identifier(id) => id.span,
            TSEntityName::QualifiedName(qn) => qn.span,
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
    pub params: Vec<TSType>,
    pub span: Span,
}

/// Type parameter declaration: `<T, U>` (for declaring type parameters)
#[derive(Debug, Clone)]
pub struct TSTypeParameterDeclaration {
    pub params: Vec<TSTypeParameter>,
    /// Position of trailing comma if present (e.g., `<T,>`)
    pub trailing_comma: Option<u32>,
    pub span: Span,
}

/// Single type parameter: `T`, `T extends U`, or `T extends U = V`
/// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
#[derive(Debug, Clone)]
pub struct TSTypeParameter {
    pub name: Identifier,
    pub constraint: Option<Box<TSType>>,
    pub default: Option<Box<TSType>>,
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
pub struct TSTypeLiteral {
    pub members: Vec<TSTypeElement>,
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
    pub key: Expression,
    pub computed: bool,
    pub optional: bool,
    pub readonly: bool,
    pub type_annotation: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Method signature: `method(): T` or `method<T>(x: T): T` or `get x(): T` or `set x(v: T)`
#[derive(Debug, Clone)]
pub struct TSMethodSignature {
    pub key: Expression,
    pub computed: bool,
    pub optional: bool,
    /// Method kind: method, get, or set (for accessor signatures in type literals)
    pub kind: MethodKind,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Call signature: `(): T` or `<T>(): T` or `(x: A): T`
#[derive(Debug, Clone)]
pub struct TSCallSignatureDeclaration {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Construct signature: `new (): T` or `new <T>(): T` or `new (x: A): T`
#[derive(Debug, Clone)]
pub struct TSConstructSignatureDeclaration {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Index signature: `[key: string]: T`
#[derive(Debug, Clone)]
pub struct TSIndexSignature {
    pub parameters: Vec<Identifier>,
    pub type_annotation: TSTypeAnnotation,
    pub is_static: bool,
    pub readonly: bool,
    pub span: Span,
}

/// Function type: `(x: T) => U` or `<T>(x: T) => U`
#[derive(Debug, Clone)]
pub struct TSFunctionType {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Box<TSTypeAnnotation>,
    pub span: Span,
}

/// Constructor type: `new () => T` or `abstract new <T>() => T`
#[derive(Debug, Clone)]
pub struct TSConstructorType {
    pub abstract_: bool,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Box<TSTypeAnnotation>,
    pub span: Span,
}

/// Tuple type: `[T, U, V]`
#[derive(Debug, Clone)]
pub struct TSTupleType {
    pub element_types: Vec<TSType>,
    pub span: Span,
}

/// Rest type in tuples: `...T`
#[derive(Debug, Clone)]
pub struct TSRestType {
    /// The type being spread
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// Optional type in tuples: `T?`
#[derive(Debug, Clone)]
pub struct TSOptionalType {
    /// The type that is optional
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// Named tuple member: `label: T` or `label?: T`
#[derive(Debug, Clone)]
pub struct TSNamedTupleMember {
    /// The label identifier
    pub label: Identifier,
    /// The element type
    pub element_type: Box<TSType>,
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
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// Conditional type: `T extends U ? V : W`
#[derive(Debug, Clone)]
pub struct TSConditionalType {
    pub check_type: Box<TSType>,
    pub extends_type: Box<TSType>,
    pub true_type: Box<TSType>,
    pub false_type: Box<TSType>,
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
    pub type_annotation: Option<Box<TSType>>,
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
    pub name_type: Option<Box<TSType>>,
    /// The value type
    pub type_annotation: Option<Box<TSType>>,
    /// Readonly modifier: `readonly`, `+readonly`, `-readonly`, or absent
    pub readonly: Option<TSMappedTypeModifier>,
    /// Optional modifier: `?`, `+?`, `-?`, or absent
    pub optional: Option<TSMappedTypeModifier>,
    pub span: Span,
}

/// Type parameter in a mapped type: `K in keyof T`
#[derive(Debug, Clone)]
pub struct TSMappedTypeParameter {
    /// The parameter name (just the string, not an Identifier)
    pub name: String,
    /// The constraint type (e.g., `keyof T`)
    pub constraint: Box<TSType>,
    pub span: Span,
}

/// Type operator: `keyof T`, `unique symbol`, `readonly T`
#[derive(Debug, Clone)]
pub struct TSTypeOperator {
    /// The operator: "keyof", "unique", "readonly"
    pub operator: TSTypeOperatorKind,
    /// The type being operated on
    pub type_annotation: Box<TSType>,
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
    pub options: Option<Box<Expression>>,
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
    /// Import type: `typeof import("module")`
    Import(Box<TSImportType>),
}

impl TSTypeQueryExprName {
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
pub struct TSTypeQuery {
    /// The expression whose type is being queried
    pub expr_name: TSTypeQueryExprName,
    /// Optional type arguments: `<T, U>` (e.g., `typeof Array<string>`)
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}
