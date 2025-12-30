//! Expression types for public AST

use serde::{Deserialize, Serialize};

use super::classes::{ClassExpression, FunctionExpression, TSParameterProperty};
use super::patterns::{ArrayPattern, AssignmentPattern, ObjectPattern, RestElement};
use super::statements::BlockStatement;
use super::types::{
    TSType, TSTypeAnnotation, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
};
use super::{Identifier, Literal, PrivateIdentifier, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Expression {
    Literal(Literal),
    Identifier(Identifier),
    PrivateIdentifier(PrivateIdentifier),
    ObjectExpression(ObjectExpression),
    ArrayExpression(ArrayExpression),
    UnaryExpression(UnaryExpression),
    UpdateExpression(UpdateExpression),
    BinaryExpression(BinaryExpression),
    CallExpression(CallExpression),
    NewExpression(NewExpression),
    MemberExpression(MemberExpression),
    ConditionalExpression(ConditionalExpression),
    ArrowFunctionExpression(ArrowFunctionExpression),
    FunctionExpression(FunctionExpression),
    ClassExpression(ClassExpression),
    SpreadElement(SpreadElement),
    TemplateLiteral(TemplateLiteral),
    TaggedTemplateExpression(TaggedTemplateExpression),
    AwaitExpression(AwaitExpression),
    YieldExpression(YieldExpression),
    SequenceExpression(SequenceExpression),
    RegexLiteral(RegexLiteral),
    Super(Super),
    // Assignment and patterns
    AssignmentExpression(AssignmentExpression),
    ObjectPattern(ObjectPattern),
    ArrayPattern(ArrayPattern),
    AssignmentPattern(AssignmentPattern),
    RestElement(RestElement),
    // TypeScript type assertions
    TSTypeAssertion(TSTypeAssertion),
    TSAsExpression(TSAsExpression),
    TSSatisfiesExpression(TSSatisfiesExpression),
    // TypeScript instantiation expression: f<T>
    TSInstantiationExpression(TSInstantiationExpression),
    // TypeScript non-null assertion: expr!
    TSNonNullExpression(TSNonNullExpression),
    // Dynamic import: import('...')
    ImportExpression(ImportExpression),
    // Meta property: import.meta, new.target
    MetaProperty(MetaProperty),
    // TypeScript parameter property: constructor(public x)
    TSParameterProperty(TSParameterProperty),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub properties: Vec<ObjectProperty>,
}

/// Object property - either a regular property or a spread element
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectProperty {
    Property(Property),
    SpreadElement(SpreadElement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrayExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub elements: Vec<Option<Expression>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: String,
    pub prefix: bool,
    pub argument: Box<Expression>,
}

/// Update expression: `++x`, `x++`, `--x`, `x--`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: String,
    pub prefix: bool,
    pub argument: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<Expression>,
    pub operator: String,
    pub right: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub callee: Box<Expression>,
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub arguments: Vec<Expression>,
    pub optional: bool,
}

/// New expression: `new Date()`, `new Map()`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub callee: Box<Expression>,
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub arguments: Vec<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub source: Box<Expression>,
    /// Optional second argument for import attributes: `{with: {type: 'json'}}`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Box<Expression>>,
}

/// Meta property: `import.meta`, `new.target`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaProperty {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The keyword: Identifier("import") or Identifier("new")
    pub meta: Identifier,
    /// The property: Identifier("meta") or Identifier("target")
    pub property: Identifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub object: Box<Expression>,
    pub property: Box<Expression>,
    pub computed: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Box<Expression>,
    pub consequent: Box<Expression>,
    pub alternate: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrowFunctionExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Option<()>, // always null for arrow functions
    pub expression: bool,
    pub generator: bool,
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    pub body: ArrowFunctionBody,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Arrow function body - either expression or block statement
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArrowFunctionBody {
    Expression(Box<Expression>),
    BlockStatement(BlockStatement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadElement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression>,
}

/// Template literal expression: `hello ${name}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateLiteral {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub quasis: Vec<TemplateElement>,
    pub expressions: Vec<Expression>,
}

/// Template element - a static string part of a template literal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateElement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub value: TemplateElementValue,
    pub tail: bool,
}

/// Value field of a template element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateElementValue {
    pub raw: String,
    /// Cooked value is null for invalid escape sequences in tagged templates
    pub cooked: Option<String>,
}

/// Tagged template expression: tag`content ${expr}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaggedTemplateExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub tag: Box<Expression>,
    pub quasi: TemplateLiteral,
}

/// Await expression: `await promise`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression>,
}

/// Yield expression: `yield value` or `yield* iterable`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The value to yield (None for `yield` with no argument)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument: Option<Box<Expression>>,
    /// Whether this is a delegating yield: `yield*`
    pub delegate: bool,
}

/// Sequence expression: `a, b, c`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expressions: Vec<Expression>,
}

/// Regular expression literal.
/// Serializes with type "Literal" to match acorn/Svelte AST.
/// Example: `/hello/gi` becomes `{type: "Literal", value: {}, raw: "/hello/gi", regex: {pattern: "hello", flags: "gi"}}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexLiteral {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Always serializes as empty object {} since regex can't be represented in JSON
    pub value: serde_json::Value,
    /// The full raw source text including slashes: /pattern/flags
    pub raw: String,
    /// Pattern and flags extracted for convenience
    pub regex: RegexValue,
}

/// Regex pattern and flags for the AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexValue {
    pub pattern: String,
    pub flags: String,
}

/// Super expression: `super`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Super {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// Assignment expression: `x = value`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: String,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub method: bool,
    pub shorthand: bool,
    pub computed: bool,
    pub key: Box<Expression>,
    pub value: Box<Expression>,
    pub kind: String,
}

// TypeScript expression nodes

/// TypeScript angle-bracket type assertion: `<Type>expr`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeAssertion {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The target type
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
    /// The expression being type-asserted
    pub expression: Box<Expression>,
}

/// TypeScript `as` type assertion: `expr as Type` or `expr as const`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSAsExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being type-asserted
    pub expression: Box<Expression>,
    /// The target type
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// TypeScript `satisfies` expression: `expr satisfies Type`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSSatisfiesExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being checked
    pub expression: Box<Expression>,
    /// The type to satisfy
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// TypeScript instantiation expression: `f<T>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSInstantiationExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being instantiated
    pub expression: Box<Expression>,
    /// The type arguments
    #[serde(rename = "typeArguments")]
    pub type_arguments: TSTypeParameterInstantiation,
}

/// TypeScript non-null assertion expression: `expr!`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSNonNullExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being asserted non-null
    pub expression: Box<Expression>,
}
