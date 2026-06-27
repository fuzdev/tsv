//! Expression types for public AST

use serde::Serialize;
use std::borrow::Cow;

use super::classes::{ClassExpression, FunctionExpression, TSParameterProperty};
use super::patterns::{ArrayPattern, AssignmentPattern, ObjectPattern, RestElement};
use super::statements::BlockStatement;
use super::types::{
    TSType, TSTypeAnnotation, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
};
use super::{Identifier, Literal, PrivateIdentifier, SourceLocation};

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Expression<'src> {
    Literal(Literal<'src>),
    Identifier(Identifier<'src>),
    PrivateIdentifier(PrivateIdentifier<'src>),
    ObjectExpression(ObjectExpression<'src>),
    ArrayExpression(ArrayExpression<'src>),
    UnaryExpression(UnaryExpression<'src>),
    UpdateExpression(UpdateExpression<'src>),
    BinaryExpression(BinaryExpression<'src>),
    CallExpression(CallExpression<'src>),
    NewExpression(NewExpression<'src>),
    MemberExpression(MemberExpression<'src>),
    ConditionalExpression(ConditionalExpression<'src>),
    ArrowFunctionExpression(ArrowFunctionExpression<'src>),
    FunctionExpression(FunctionExpression<'src>),
    ClassExpression(ClassExpression<'src>),
    SpreadElement(SpreadElement<'src>),
    TemplateLiteral(TemplateLiteral<'src>),
    TaggedTemplateExpression(TaggedTemplateExpression<'src>),
    AwaitExpression(AwaitExpression<'src>),
    YieldExpression(YieldExpression<'src>),
    SequenceExpression(SequenceExpression<'src>),
    RegexLiteral(RegexLiteral<'src>),
    ThisExpression(ThisExpression),
    Super(Super),
    // Assignment and patterns
    AssignmentExpression(AssignmentExpression<'src>),
    ObjectPattern(ObjectPattern<'src>),
    ArrayPattern(ArrayPattern<'src>),
    AssignmentPattern(AssignmentPattern<'src>),
    RestElement(RestElement<'src>),
    // TypeScript type assertions
    TSTypeAssertion(TSTypeAssertion<'src>),
    TSAsExpression(TSAsExpression<'src>),
    TSSatisfiesExpression(TSSatisfiesExpression<'src>),
    // TypeScript instantiation expression: f<T>
    TSInstantiationExpression(TSInstantiationExpression<'src>),
    // TypeScript non-null assertion: expr!
    TSNonNullExpression(TSNonNullExpression<'src>),
    // Dynamic import: import('...')
    ImportExpression(ImportExpression<'src>),
    // Meta property: import.meta, new.target
    MetaProperty(MetaProperty<'src>),
    // TypeScript parameter property: constructor(public x)
    TSParameterProperty(TSParameterProperty<'src>),
    // Optional chaining wrapper: a?.b, a?.b(), a?.b.c
    ChainExpression(ChainExpression<'src>),
}

impl Expression<'_> {
    /// Returns the byte offset of the start of this expression.
    pub fn start(&self) -> u32 {
        match self {
            Self::Literal(n) => n.start,
            Self::Identifier(n) => n.start,
            Self::PrivateIdentifier(n) => n.start,
            Self::ObjectExpression(n) => n.start,
            Self::ArrayExpression(n) => n.start,
            Self::UnaryExpression(n) => n.start,
            Self::UpdateExpression(n) => n.start,
            Self::BinaryExpression(n) => n.start,
            Self::CallExpression(n) => n.start,
            Self::NewExpression(n) => n.start,
            Self::MemberExpression(n) => n.start,
            Self::ConditionalExpression(n) => n.start,
            Self::ArrowFunctionExpression(n) => n.start,
            Self::FunctionExpression(n) => n.start,
            Self::ClassExpression(n) => n.start,
            Self::SpreadElement(n) => n.start,
            Self::TemplateLiteral(n) => n.start,
            Self::TaggedTemplateExpression(n) => n.start,
            Self::AwaitExpression(n) => n.start,
            Self::YieldExpression(n) => n.start,
            Self::SequenceExpression(n) => n.start,
            Self::RegexLiteral(n) => n.start,
            Self::ThisExpression(n) => n.start,
            Self::Super(n) => n.start,
            Self::AssignmentExpression(n) => n.start,
            Self::ObjectPattern(n) => n.start,
            Self::ArrayPattern(n) => n.start,
            Self::AssignmentPattern(n) => n.start,
            Self::RestElement(n) => n.start,
            Self::TSTypeAssertion(n) => n.start,
            Self::TSAsExpression(n) => n.start,
            Self::TSSatisfiesExpression(n) => n.start,
            Self::TSInstantiationExpression(n) => n.start,
            Self::TSNonNullExpression(n) => n.start,
            Self::ImportExpression(n) => n.start,
            Self::MetaProperty(n) => n.start,
            Self::TSParameterProperty(n) => n.start,
            Self::ChainExpression(n) => n.start,
        }
    }

    /// Inject `character` (byte offset) into the top-level `loc` of this expression.
    ///
    /// Svelte's parser includes `character` in `loc` for certain Identifier nodes it creates
    /// directly (not through acorn). This only sets `character` on `Identifier` nodes since
    /// that's the only node type Svelte's `read_identifier()` creates.
    pub fn inject_loc_character(&mut self) {
        if let Expression::Identifier(id) = self {
            id.loc = id.loc.clone().with_character(id.start, id.end);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ObjectExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub properties: Vec<ObjectProperty<'src>>,
}

/// Object property - either a regular property or a spread element
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ObjectProperty<'src> {
    Property(Property<'src>),
    SpreadElement(SpreadElement<'src>),
}

#[derive(Debug, Clone, Serialize)]
pub struct ArrayExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub elements: Vec<Option<Expression<'src>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnaryExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: &'static str,
    pub prefix: bool,
    pub argument: Box<Expression<'src>>,
}

/// Update expression: `++x`, `x++`, `--x`, `x--`
#[derive(Debug, Clone, Serialize)]
pub struct UpdateExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: &'static str,
    pub prefix: bool,
    pub argument: Box<Expression<'src>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BinaryExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<Expression<'src>>,
    pub operator: &'static str,
    pub right: Box<Expression<'src>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub callee: Box<Expression<'src>>,
    pub arguments: Vec<Expression<'src>>,
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation<'src>>,
    /// acorn-typescript omits `optional` when `typeArguments` is present or in decorator contexts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

/// New expression: `new Date()`, `new Map()`
#[derive(Debug, Clone, Serialize)]
pub struct NewExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub callee: Box<Expression<'src>>,
    pub arguments: Vec<Expression<'src>>,
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation<'src>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub source: Box<Expression<'src>>,
    /// Import arguments for import attributes: `import('mod', {with: {type: 'json'}})`
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<Expression<'src>>,
}

/// Meta property: `import.meta`, `new.target`
#[derive(Debug, Clone, Serialize)]
pub struct MetaProperty<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The keyword: Identifier("import") or Identifier("new")
    pub meta: Identifier<'src>,
    /// The property: Identifier("meta") or Identifier("target")
    pub property: Identifier<'src>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub object: Box<Expression<'src>>,
    pub property: Box<Expression<'src>>,
    pub computed: bool,
    /// acorn omits `optional` in certain contexts (e.g., decorator expressions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

/// Optional chaining wrapper: `a?.b`, `a?.b()`, `a?.b.c.d()`
///
/// Wraps the outermost MemberExpression/CallExpression in a chain that
/// contains at least one `?.` operator. Matches acorn's ChainExpression node.
#[derive(Debug, Clone, Serialize)]
pub struct ChainExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Box<Expression<'src>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConditionalExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Box<Expression<'src>>,
    pub consequent: Box<Expression<'src>>,
    pub alternate: Box<Expression<'src>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArrowFunctionExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Option<()>, // always null for arrow functions
    pub expression: bool,
    pub generator: bool,
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression<'src>>,
    pub body: ArrowFunctionBody<'src>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation<'src>>,
}

/// Arrow function body - either expression or block statement
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ArrowFunctionBody<'src> {
    Expression(Box<Expression<'src>>),
    BlockStatement(BlockStatement<'src>),
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadElement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression<'src>>,
}

/// Template literal expression: `hello ${name}`
#[derive(Debug, Clone, Serialize)]
pub struct TemplateLiteral<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expressions: Vec<Expression<'src>>,
    pub quasis: Vec<TemplateElement<'src>>,
}

/// Template element - a static string part of a template literal
#[derive(Debug, Clone, Serialize)]
pub struct TemplateElement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub value: TemplateElementValue<'src>,
    pub tail: bool,
}

/// Value field of a template element
#[derive(Debug, Clone, Serialize)]
pub struct TemplateElementValue<'src> {
    pub raw: Cow<'src, str>,
    /// Cooked value is null for invalid escape sequences in tagged templates
    pub cooked: Option<Cow<'src, str>>,
}

/// Tagged template expression: tag`content ${expr}`
#[derive(Debug, Clone, Serialize)]
pub struct TaggedTemplateExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub tag: Box<Expression<'src>>,
    pub quasi: TemplateLiteral<'src>,
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation<'src>>,
}

/// Await expression: `await promise`
#[derive(Debug, Clone, Serialize)]
pub struct AwaitExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression<'src>>,
}

/// Yield expression: `yield value` or `yield* iterable`
#[derive(Debug, Clone, Serialize)]
pub struct YieldExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Whether this is a delegating yield: `yield*`
    pub delegate: bool,
    /// The value to yield (None for `yield` with no argument)
    pub argument: Option<Box<Expression<'src>>>,
}

/// Sequence expression: `a, b, c`
#[derive(Debug, Clone, Serialize)]
pub struct SequenceExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expressions: Vec<Expression<'src>>,
}

/// Regular expression literal.
/// Serializes with type "Literal" to match acorn/Svelte AST.
/// Example: `/hello/gi` becomes `{type: "Literal", value: {}, raw: "/hello/gi", regex: {pattern: "hello", flags: "gi"}}`
#[derive(Debug, Clone, Serialize)]
pub struct RegexLiteral<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Always serializes as empty object {} since regex can't be represented in JSON
    pub value: serde_json::Value,
    /// The full raw source text including slashes: /pattern/flags
    pub raw: Cow<'src, str>,
    /// Pattern and flags extracted for convenience
    pub regex: RegexValue<'src>,
}

/// Regex pattern and flags for the AST.
#[derive(Debug, Clone, Serialize)]
pub struct RegexValue<'src> {
    pub pattern: Cow<'src, str>,
    pub flags: Cow<'src, str>,
}

/// This expression: `this`
#[derive(Debug, Clone, Serialize)]
pub struct ThisExpression {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// Super expression: `super`
#[derive(Debug, Clone, Serialize)]
pub struct Super {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// Assignment expression: `x = value`
#[derive(Debug, Clone, Serialize)]
pub struct AssignmentExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: &'static str,
    pub left: Box<Expression<'src>>,
    pub right: Box<Expression<'src>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Property<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub method: bool,
    pub shorthand: bool,
    pub computed: bool,
    pub key: Box<Expression<'src>>,
    pub kind: &'static str,
    pub value: Box<Expression<'src>>,
}

// TypeScript expression nodes

/// TypeScript angle-bracket type assertion: `<Type>expr`
#[derive(Debug, Clone, Serialize)]
pub struct TSTypeAssertion<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The target type
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType<'src>>,
    /// The expression being type-asserted
    pub expression: Box<Expression<'src>>,
}

/// TypeScript `as` type assertion: `expr as Type` or `expr as const`
#[derive(Debug, Clone, Serialize)]
pub struct TSAsExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being type-asserted
    pub expression: Box<Expression<'src>>,
    /// The target type
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType<'src>>,
}

/// TypeScript `satisfies` expression: `expr satisfies Type`
#[derive(Debug, Clone, Serialize)]
pub struct TSSatisfiesExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being checked
    pub expression: Box<Expression<'src>>,
    /// The type to satisfy
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType<'src>>,
}

/// TypeScript instantiation expression: `f<T>`
#[derive(Debug, Clone, Serialize)]
pub struct TSInstantiationExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being instantiated
    pub expression: Box<Expression<'src>>,
    /// The type arguments
    #[serde(rename = "typeArguments")]
    pub type_arguments: TSTypeParameterInstantiation<'src>,
}

/// TypeScript non-null assertion expression: `expr!`
#[derive(Debug, Clone, Serialize)]
pub struct TSNonNullExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being asserted non-null
    pub expression: Box<Expression<'src>>,
}
