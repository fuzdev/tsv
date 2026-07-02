//! TypeScript type definitions for public AST

use serde::Serialize;
use std::borrow::Cow;

use super::{Expression, Identifier, Literal, SourceLocation, TemplateElement, is_false};

/// Public AST representation of TypeScript type annotation
///
/// Serializes to JSON matching Svelte's/acorn-typescript's format:
/// ```json
/// {
///   "type": "TSTypeAnnotation",
///   "start": 7,
///   "end": 15,
///   "loc": { "start": { "line": 1, "column": 7 }, ... },
///   "typeAnnotation": { "type": "TSNumberKeyword", ... }
/// }
/// ```
///
/// Note the nested `typeAnnotation` field uses camelCase for JSON compatibility.
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeAnnotation<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType<'src>>,
}

/// TypeScript type expression
///
/// Uses serde's untagged enum to serialize each variant based on its structure.
/// Each variant serializes to a flat object with its own `type` field.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSType<'src> {
    TSNumberKeyword(TSNumberKeyword),
    TSStringKeyword(TSStringKeyword),
    TSBooleanKeyword(TSBooleanKeyword),
    TSAnyKeyword(TSAnyKeyword),
    TSVoidKeyword(TSVoidKeyword),
    TSUndefinedKeyword(TSUndefinedKeyword),
    TSNullKeyword(TSNullKeyword),
    TSNeverKeyword(TSNeverKeyword),
    TSUnknownKeyword(TSUnknownKeyword),
    TSObjectKeyword(TSObjectKeyword),
    TSSymbolKeyword(TSSymbolKeyword),
    TSBigIntKeyword(TSBigIntKeyword),
    TSLiteralType(TSLiteralType<'src>),
    TSArrayType(TSArrayType<'src>),
    TSUnionType(TSUnionType<'src>),
    TSIntersectionType(TSIntersectionType<'src>),
    TSTypeReference(TSTypeReference<'src>),
    TSTypeLiteral(TSTypeLiteral<'src>),
    TSFunctionType(TSFunctionType<'src>),
    TSConstructorType(TSConstructorType<'src>),
    TSTupleType(TSTupleType<'src>),
    TSParenthesizedType(TSParenthesizedType<'src>),
    TSTypePredicate(TSTypePredicate<'src>),
    TSConditionalType(TSConditionalType<'src>),
    TSMappedType(TSMappedType<'src>),
    TSTypeOperator(TSTypeOperator<'src>),
    TSImportType(TSImportType<'src>),
    TSTypeQuery(TSTypeQuery<'src>),
    TSIndexedAccessType(TSIndexedAccessType<'src>),
    TSRestType(TSRestType<'src>),
    TSOptionalType(TSOptionalType<'src>),
    TSNamedTupleMember(TSNamedTupleMember<'src>),
    TSInferType(TSInferType<'src>),
    TSThisType(TSThisType),
}

/// TypeScript array type: `number[]`, `string[]`, etc.
#[derive(Debug, Clone, Serialize)]
pub struct TSArrayType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "elementType")]
    pub element_type: Box<TSType<'src>>,
}

/// TypeScript indexed access type: `T[K]`, `Obj["key"]`
#[derive(Debug, Clone, Serialize)]
pub struct TSIndexedAccessType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "objectType")]
    pub object_type: Box<TSType<'src>>,
    #[serde(rename = "indexType")]
    pub index_type: Box<TSType<'src>>,
}

/// TypeScript `number` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSNumberKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `string` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSStringKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `boolean` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSBooleanKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `any` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSAnyKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `void` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSVoidKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `undefined` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSUndefinedKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `null` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSNullKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `never` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSNeverKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `unknown` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSUnknownKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `object` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSObjectKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `symbol` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSSymbolKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `bigint` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSBigIntKeyword {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript type alias declaration: `type X = T`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeAliasDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier<'src>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: TSType<'src>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
}

/// TypeScript literal type: `type X = 'hello'` or `type X = \`template\``
#[derive(Debug, Clone, Serialize)]
pub struct TSLiteralType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub literal: TSLiteralTypeLiteral<'src>,
}

/// The literal value inside a TSLiteralType
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSLiteralTypeLiteral<'src> {
    TemplateLiteral(TemplateLiteralType<'src>),
    /// Unary expression for negative numbers: `-1`, `-42n`
    UnaryExpression(super::UnaryExpression<'src>),
    /// Literal value (string, number, bigint)
    Literal(Literal<'src>),
}

/// Template literal used as a type (same structure as TemplateLiteral but expressions are TSType)
#[derive(Debug, Clone, Serialize)]
pub struct TemplateLiteralType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expressions: Vec<TSType<'src>>,
    pub quasis: Vec<TemplateElement<'src>>,
}

/// Entity name: `Foo` or `Foo.Bar.Baz`
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSEntityName<'src> {
    Identifier(Identifier<'src>),
    QualifiedName(TSQualifiedName<'src>),
}

/// Qualified name: `Foo.Bar`
#[derive(Debug, Clone, Serialize)]
pub struct TSQualifiedName<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<TSEntityName<'src>>,
    pub right: Identifier<'src>,
}

/// Type parameter instantiation: `<T, U>`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeParameterInstantiation<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub params: Vec<TSType<'src>>,
}

/// Type parameter declaration: `<T extends U = V>`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeParameterDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub params: Vec<TSTypeParameter<'src>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<TSTypeParameterExtra>,
}

/// Extra metadata for type parameter declarations (trailing comma position)
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeParameterExtra {
    #[serde(rename = "trailingComma")]
    pub trailing_comma: u32,
}

/// Single type parameter: `T extends U = V`
/// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeParameter<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// `const` modifier (TS 5.0): `<const T>`
    #[serde(rename = "const", skip_serializing_if = "is_false")]
    pub is_const: bool,
    /// `in` variance modifier (TS 4.7): `<in T>`
    #[serde(rename = "in", skip_serializing_if = "is_false")]
    pub is_in: bool,
    /// `out` variance modifier (TS 4.7): `<out T>`
    #[serde(rename = "out", skip_serializing_if = "is_false")]
    pub is_out: bool,
    pub name: Cow<'src, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Box<TSType<'src>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Box<TSType<'src>>>,
}

/// Type element - member of a type literal or interface
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSTypeElement<'src> {
    PropertySignature(TSPropertySignature<'src>),
    MethodSignature(TSMethodSignature<'src>),
    CallSignature(TSCallSignatureDeclaration<'src>),
    ConstructSignature(TSConstructSignatureDeclaration<'src>),
    IndexSignature(TSIndexSignature<'src>),
}

/// Interface body: `{ members }`
#[derive(Debug, Clone, Serialize)]
pub struct TSInterfaceBody<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<TSTypeElement<'src>>,
}

/// Property signature: `prop: T`
#[derive(Debug, Clone, Serialize)]
pub struct TSPropertySignature<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(skip_serializing_if = "is_false")]
    pub readonly: bool,
    /// acorn omits this field when key is `new` keyword
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed: Option<bool>,
    pub key: Expression<'src>,
    #[serde(skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation<'src>>,
}

/// Method signature: `method(): T` or `method<T>(x: T): T` or `get x(): T` or `set x(v: T)`
#[derive(Debug, Clone, Serialize)]
pub struct TSMethodSignature<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub computed: bool,
    pub key: Expression<'src>,
    /// Whether this is an optional method: `method?(): T`
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// Method kind: "get" or "set" for accessor signatures (omitted for regular methods)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    pub parameters: Vec<Expression<'src>>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation<'src>>,
}

/// Call signature: `(): T` or `<T>(): T`
#[derive(Debug, Clone, Serialize)]
pub struct TSCallSignatureDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "parameters")]
    pub params: Vec<Expression<'src>>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation<'src>>,
}

/// Construct signature: `new (): T` or `new <T>(): T`
#[derive(Debug, Clone, Serialize)]
pub struct TSConstructSignatureDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "parameters")]
    pub params: Vec<Expression<'src>>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation<'src>>,
}

/// Index signature: `[key: string]: T`
#[derive(Debug, Clone, Serialize)]
pub struct TSIndexSignature<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "static")]
    #[serde(skip_serializing_if = "is_false")]
    pub is_static: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub readonly: bool,
    pub parameters: Vec<Identifier<'src>>,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: TSTypeAnnotation<'src>,
}

/// Union type: `A | B | C`
#[derive(Debug, Clone, Serialize)]
pub struct TSUnionType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub types: Vec<TSType<'src>>,
}

/// Intersection type: `A & B & C`
#[derive(Debug, Clone, Serialize)]
pub struct TSIntersectionType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub types: Vec<TSType<'src>>,
}

/// Type reference: `SomeType` or `Array<T>`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeReference<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeName")]
    pub type_name: TSEntityName<'src>,
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation<'src>>,
}

/// Type literal (object type): `{ prop: T }`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeLiteral<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub members: Vec<TSTypeElement<'src>>,
}

/// Function type: `(x: T) => U` or `<T>(x: T) => U`
#[derive(Debug, Clone, Serialize)]
pub struct TSFunctionType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "parameters")]
    pub params: Vec<Expression<'src>>,
    #[serde(rename = "typeAnnotation")]
    pub return_type: Box<TSTypeAnnotation<'src>>,
}

/// Constructor type: `new () => T` or `abstract new <T>() => T`
#[derive(Debug, Clone, Serialize)]
pub struct TSConstructorType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "abstract")]
    pub abstract_: bool,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "parameters")]
    pub params: Vec<Expression<'src>>,
    #[serde(rename = "typeAnnotation")]
    pub return_type: Box<TSTypeAnnotation<'src>>,
}

/// Tuple type: `[T, U, V]`
#[derive(Debug, Clone, Serialize)]
pub struct TSTupleType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "elementTypes")]
    pub element_types: Vec<TSType<'src>>,
}

/// Rest type in tuples: `...T`
#[derive(Debug, Clone, Serialize)]
pub struct TSRestType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType<'src>>,
}

/// Optional type in tuples: `T?`
#[derive(Debug, Clone, Serialize)]
pub struct TSOptionalType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType<'src>>,
}

/// Named tuple member: `label: T` or `label?: T`
#[derive(Debug, Clone, Serialize)]
pub struct TSNamedTupleMember<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub optional: bool,
    pub label: Identifier<'src>,
    #[serde(rename = "elementType")]
    pub element_type: Box<TSType<'src>>,
}

/// Infer type: `infer U` (in conditional types)
#[derive(Debug, Clone, Serialize)]
pub struct TSInferType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameter")]
    pub type_parameter: TSTypeParameter<'src>,
}

/// This type: `this` in type position
#[derive(Debug, Clone, Serialize)]
pub struct TSThisType {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// Parenthesized type: `(T)`
#[derive(Debug, Clone, Serialize)]
pub struct TSParenthesizedType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType<'src>>,
}

/// TypeScript type predicate: `x is T` or `asserts x is T`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypePredicate<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "parameterName")]
    pub parameter_name: TSTypePredicateParameterName<'src>,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Option<Box<TSTypeAnnotation<'src>>>,
    pub asserts: bool,
}

/// Either an Identifier or TSThisType for the parameter name in a type predicate
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSTypePredicateParameterName<'src> {
    Identifier(Identifier<'src>),
    TSThisType(TSThisType),
}

/// TypeScript conditional type: `T extends U ? V : W`
#[derive(Debug, Clone, Serialize)]
pub struct TSConditionalType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "checkType")]
    pub check_type: Box<TSType<'src>>,
    #[serde(rename = "extendsType")]
    pub extends_type: Box<TSType<'src>>,
    #[serde(rename = "trueType")]
    pub true_type: Box<TSType<'src>>,
    #[serde(rename = "falseType")]
    pub false_type: Box<TSType<'src>>,
}

/// Mapped type: `{ [K in keyof T]: V }`
#[derive(Debug, Clone, Serialize)]
pub struct TSMappedType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Readonly modifier: true, "+", "-", or absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<TSMappedTypeModifier>,
    #[serde(rename = "typeParameter")]
    pub type_parameter: TSMappedTypeParameter<'src>,
    /// Optional key remapping: `as NewK`
    #[serde(rename = "nameType")]
    pub name_type: Option<Box<TSType<'src>>>,
    /// Optional modifier: true, "+", "-", or absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<TSMappedTypeModifier>,
    /// The value type — omitted entirely when absent (`{ [K in T] }`), matching acorn
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<Box<TSType<'src>>>,
}

/// Type parameter in a mapped type: `K in keyof T`
#[derive(Debug, Clone, Serialize)]
pub struct TSMappedTypeParameter<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The parameter name (just the string, not an Identifier in mapped types)
    pub name: Cow<'src, str>,
    /// The constraint type (e.g., `keyof T`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Box<TSType<'src>>>,
}

/// Mapped type modifier value: true, "+", or "-"
#[derive(Debug, Clone, Copy)]
pub enum TSMappedTypeModifier {
    True,
    Plus,
    Minus,
}

impl Serialize for TSMappedTypeModifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            TSMappedTypeModifier::True => serializer.serialize_bool(true),
            TSMappedTypeModifier::Plus => serializer.serialize_str("+"),
            TSMappedTypeModifier::Minus => serializer.serialize_str("-"),
        }
    }
}

/// Type operator: `keyof T`, `unique symbol`, `readonly T`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeOperator<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The operator: "keyof", "unique", "readonly"
    pub operator: &'static str,
    /// The type being operated on
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType<'src>>,
}

/// Import type: `import('module')` or `import('module', {with: {...}}).Qualifier<T>`
#[derive(Debug, Clone, Serialize)]
pub struct TSImportType<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The module specifier (string literal)
    pub argument: Literal<'src>,
    /// Optional options object: `{with: {type: 'json'}}`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Box<Expression<'src>>>,
    /// Optional qualifier: `.Foo` or `.Foo.Bar` after the import
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<TSEntityName<'src>>,
    /// Optional type arguments: `<T, U>`
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation<'src>>,
}

/// Type query expression name: Identifier, QualifiedName, or ImportType
///
/// The `exprName` field of `TSTypeQuery` can be:
/// - `Identifier` for `typeof x`
/// - `TSQualifiedName` for `typeof Foo.bar`
/// - `TSImportType` for `typeof import("module")`
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSTypeQueryExprName<'src> {
    Identifier(Identifier<'src>),
    QualifiedName(TSQualifiedName<'src>),
    Import(TSImportType<'src>),
}

/// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`, `typeof Array<T>`
///
/// Gets the type of a value expression.
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeQuery<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression whose type is being queried
    #[serde(rename = "exprName")]
    pub expr_name: TSTypeQueryExprName<'src>,
    /// Optional type arguments: `<T, U>` (e.g., `typeof Array<string>`)
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation<'src>>,
}
