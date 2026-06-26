//! TypeScript type definitions for public AST

use serde::Serialize;

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
pub struct TSTypeAnnotation {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// TypeScript type expression
///
/// Uses serde's untagged enum to serialize each variant based on its structure.
/// Each variant serializes to a flat object with its own `type` field.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSType {
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
    TSLiteralType(TSLiteralType),
    TSArrayType(TSArrayType),
    TSUnionType(TSUnionType),
    TSIntersectionType(TSIntersectionType),
    TSTypeReference(TSTypeReference),
    TSTypeLiteral(TSTypeLiteral),
    TSFunctionType(TSFunctionType),
    TSConstructorType(TSConstructorType),
    TSTupleType(TSTupleType),
    TSParenthesizedType(TSParenthesizedType),
    TSTypePredicate(TSTypePredicate),
    TSConditionalType(TSConditionalType),
    TSMappedType(TSMappedType),
    TSTypeOperator(TSTypeOperator),
    TSImportType(TSImportType),
    TSTypeQuery(TSTypeQuery),
    TSIndexedAccessType(TSIndexedAccessType),
    TSRestType(TSRestType),
    TSOptionalType(TSOptionalType),
    TSNamedTupleMember(TSNamedTupleMember),
    TSInferType(TSInferType),
    TSThisType(TSThisType),
}

/// TypeScript array type: `number[]`, `string[]`, etc.
#[derive(Debug, Clone, Serialize)]
pub struct TSArrayType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "elementType")]
    pub element_type: Box<TSType>,
}

/// TypeScript indexed access type: `T[K]`, `Obj["key"]`
#[derive(Debug, Clone, Serialize)]
pub struct TSIndexedAccessType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "objectType")]
    pub object_type: Box<TSType>,
    #[serde(rename = "indexType")]
    pub index_type: Box<TSType>,
}

/// TypeScript `number` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSNumberKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `string` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSStringKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `boolean` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSBooleanKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `any` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSAnyKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `void` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSVoidKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `undefined` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSUndefinedKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `null` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSNullKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `never` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSNeverKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `unknown` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSUnknownKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `object` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSObjectKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `symbol` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSSymbolKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `bigint` type keyword
#[derive(Debug, Clone, Serialize)]
pub struct TSBigIntKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript type alias declaration: `type X = T`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeAliasDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: TSType,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
}

/// TypeScript literal type: `type X = 'hello'` or `type X = \`template\``
#[derive(Debug, Clone, Serialize)]
pub struct TSLiteralType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub literal: TSLiteralTypeLiteral,
}

/// The literal value inside a TSLiteralType
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSLiteralTypeLiteral {
    TemplateLiteral(TemplateLiteralType),
    /// Unary expression for negative numbers: `-1`, `-42n`
    UnaryExpression(super::UnaryExpression),
    /// Literal value (string, number, bigint)
    Literal(Literal),
}

/// Template literal used as a type (same structure as TemplateLiteral but expressions are TSType)
#[derive(Debug, Clone, Serialize)]
pub struct TemplateLiteralType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expressions: Vec<TSType>,
    pub quasis: Vec<TemplateElement>,
}

/// Entity name: `Foo` or `Foo.Bar.Baz`
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSEntityName {
    Identifier(Identifier),
    QualifiedName(TSQualifiedName),
}

/// Qualified name: `Foo.Bar`
#[derive(Debug, Clone, Serialize)]
pub struct TSQualifiedName {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<TSEntityName>,
    pub right: Identifier,
}

/// Type parameter instantiation: `<T, U>`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeParameterInstantiation {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub params: Vec<TSType>,
}

/// Type parameter declaration: `<T extends U = V>`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeParameterDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub params: Vec<TSTypeParameter>,
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
pub struct TSTypeParameter {
    #[serde(rename = "type")]
    pub node_type: String,
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
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Box<TSType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Box<TSType>>,
}

/// Type element - member of a type literal or interface
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSTypeElement {
    PropertySignature(TSPropertySignature),
    MethodSignature(TSMethodSignature),
    CallSignature(TSCallSignatureDeclaration),
    ConstructSignature(TSConstructSignatureDeclaration),
    IndexSignature(TSIndexSignature),
}

/// Interface body: `{ members }`
#[derive(Debug, Clone, Serialize)]
pub struct TSInterfaceBody {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<TSTypeElement>,
}

/// Property signature: `prop: T`
#[derive(Debug, Clone, Serialize)]
pub struct TSPropertySignature {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(skip_serializing_if = "is_false")]
    pub readonly: bool,
    /// acorn omits this field when key is `new` keyword
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed: Option<bool>,
    pub key: Expression,
    #[serde(skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}

/// Method signature: `method(): T` or `method<T>(x: T): T` or `get x(): T` or `set x(v: T)`
#[derive(Debug, Clone, Serialize)]
pub struct TSMethodSignature {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub computed: bool,
    pub key: Expression,
    /// Whether this is an optional method: `method?(): T`
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// Method kind: "get" or "set" for accessor signatures (omitted for regular methods)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub parameters: Vec<Expression>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Call signature: `(): T` or `<T>(): T`
#[derive(Debug, Clone, Serialize)]
pub struct TSCallSignatureDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "parameters")]
    pub params: Vec<Expression>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Construct signature: `new (): T` or `new <T>(): T`
#[derive(Debug, Clone, Serialize)]
pub struct TSConstructSignatureDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "parameters")]
    pub params: Vec<Expression>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Index signature: `[key: string]: T`
#[derive(Debug, Clone, Serialize)]
pub struct TSIndexSignature {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "static")]
    #[serde(skip_serializing_if = "is_false")]
    pub is_static: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub readonly: bool,
    pub parameters: Vec<Identifier>,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: TSTypeAnnotation,
}

/// Union type: `A | B | C`
#[derive(Debug, Clone, Serialize)]
pub struct TSUnionType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub types: Vec<TSType>,
}

/// Intersection type: `A & B & C`
#[derive(Debug, Clone, Serialize)]
pub struct TSIntersectionType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub types: Vec<TSType>,
}

/// Type reference: `SomeType` or `Array<T>`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeReference {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeName")]
    pub type_name: TSEntityName,
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
}

/// Type literal (object type): `{ prop: T }`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeLiteral {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub members: Vec<TSTypeElement>,
}

/// Function type: `(x: T) => U` or `<T>(x: T) => U`
#[derive(Debug, Clone, Serialize)]
pub struct TSFunctionType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "parameters")]
    pub params: Vec<Expression>,
    #[serde(rename = "typeAnnotation")]
    pub return_type: Box<TSTypeAnnotation>,
}

/// Constructor type: `new () => T` or `abstract new <T>() => T`
#[derive(Debug, Clone, Serialize)]
pub struct TSConstructorType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "abstract")]
    pub abstract_: bool,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "parameters")]
    pub params: Vec<Expression>,
    #[serde(rename = "typeAnnotation")]
    pub return_type: Box<TSTypeAnnotation>,
}

/// Tuple type: `[T, U, V]`
#[derive(Debug, Clone, Serialize)]
pub struct TSTupleType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "elementTypes")]
    pub element_types: Vec<TSType>,
}

/// Rest type in tuples: `...T`
#[derive(Debug, Clone, Serialize)]
pub struct TSRestType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// Optional type in tuples: `T?`
#[derive(Debug, Clone, Serialize)]
pub struct TSOptionalType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// Named tuple member: `label: T` or `label?: T`
#[derive(Debug, Clone, Serialize)]
pub struct TSNamedTupleMember {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub optional: bool,
    pub label: Identifier,
    #[serde(rename = "elementType")]
    pub element_type: Box<TSType>,
}

/// Infer type: `infer U` (in conditional types)
#[derive(Debug, Clone, Serialize)]
pub struct TSInferType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameter")]
    pub type_parameter: TSTypeParameter,
}

/// This type: `this` in type position
#[derive(Debug, Clone, Serialize)]
pub struct TSThisType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// Parenthesized type: `(T)`
#[derive(Debug, Clone, Serialize)]
pub struct TSParenthesizedType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// TypeScript type predicate: `x is T` or `asserts x is T`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypePredicate {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "parameterName")]
    pub parameter_name: TSTypePredicateParameterName,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Option<Box<TSTypeAnnotation>>,
    pub asserts: bool,
}

/// Either an Identifier or TSThisType for the parameter name in a type predicate
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSTypePredicateParameterName {
    Identifier(Identifier),
    TSThisType(TSThisType),
}

/// TypeScript conditional type: `T extends U ? V : W`
#[derive(Debug, Clone, Serialize)]
pub struct TSConditionalType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "checkType")]
    pub check_type: Box<TSType>,
    #[serde(rename = "extendsType")]
    pub extends_type: Box<TSType>,
    #[serde(rename = "trueType")]
    pub true_type: Box<TSType>,
    #[serde(rename = "falseType")]
    pub false_type: Box<TSType>,
}

/// Mapped type: `{ [K in keyof T]: V }`
#[derive(Debug, Clone, Serialize)]
pub struct TSMappedType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Readonly modifier: true, "+", "-", or absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<TSMappedTypeModifier>,
    #[serde(rename = "typeParameter")]
    pub type_parameter: TSMappedTypeParameter,
    /// Optional key remapping: `as NewK`
    #[serde(rename = "nameType")]
    pub name_type: Option<Box<TSType>>,
    /// Optional modifier: true, "+", "-", or absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<TSMappedTypeModifier>,
    /// The value type
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Option<Box<TSType>>,
}

/// Type parameter in a mapped type: `K in keyof T`
#[derive(Debug, Clone, Serialize)]
pub struct TSMappedTypeParameter {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The parameter name (just the string, not an Identifier in mapped types)
    pub name: String,
    /// The constraint type (e.g., `keyof T`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Box<TSType>>,
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
pub struct TSTypeOperator {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The operator: "keyof", "unique", "readonly"
    pub operator: String,
    /// The type being operated on
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// Import type: `import('module')` or `import('module', {with: {...}}).Qualifier<T>`
#[derive(Debug, Clone, Serialize)]
pub struct TSImportType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The module specifier (string literal)
    pub argument: Literal,
    /// Optional options object: `{with: {type: 'json'}}`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Box<Expression>>,
    /// Optional qualifier: `.Foo` or `.Foo.Bar` after the import
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<TSEntityName>,
    /// Optional type arguments: `<T, U>`
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
}

/// Type query expression name: Identifier, QualifiedName, or ImportType
///
/// The `exprName` field of `TSTypeQuery` can be:
/// - `Identifier` for `typeof x`
/// - `TSQualifiedName` for `typeof Foo.bar`
/// - `TSImportType` for `typeof import("module")`
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSTypeQueryExprName {
    Identifier(Identifier),
    QualifiedName(TSQualifiedName),
    Import(TSImportType),
}

/// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`, `typeof Array<T>`
///
/// Gets the type of a value expression.
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeQuery {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression whose type is being queried
    #[serde(rename = "exprName")]
    pub expr_name: TSTypeQueryExprName,
    /// Optional type arguments: `<T, U>` (e.g., `typeof Array<string>`)
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
}
