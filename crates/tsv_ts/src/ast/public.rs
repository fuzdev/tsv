// Public AST types - with serde, matches Svelte's JSON structure exactly
// Uses u32 for positions (max 4GB file size) for memory efficiency

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<Statement>,
    #[serde(rename = "sourceType")]
    pub source_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Statement {
    ExpressionStatement(ExpressionStatement),
    VariableDeclaration(VariableDeclaration),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Expression {
    Literal(Literal),
    Identifier(Identifier),
    ObjectExpression(ObjectExpression),
    // TODO: BinaryExpression, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Literal {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(serialize_with = "serialize_literal_value")]
    pub value: serde_json::Value,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub name: String,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub properties: Vec<Property>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub declarations: Vec<VariableDeclarator>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDeclarator {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier,
    pub init: Option<Expression>,
}

// TypeScript type annotation nodes

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
/// Uses serde's `tag = "type"` to serialize enum variants with a `type` field.
/// Each variant serializes to a flat object with its own fields plus `"type": "VariantName"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSType {
    /// The `number` type keyword
    TSNumberKeyword(TSNumberKeyword),
    // TODO: TSStringKeyword, TSBooleanKeyword, etc.
}

/// TypeScript `number` type keyword
///
/// Serializes to:
/// ```json
/// {
///   "type": "TSNumberKeyword",
///   "start": 9,
///   "end": 15,
///   "loc": { ... }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSNumberKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

// Serialize numbers as integers if they have no fractional part
fn serialize_literal_value<S>(value: &serde_json::Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // For numbers, ensure integers are serialized without decimals
    if let Some(f) = value.as_f64()
        && f.fract() == 0.0
        && f.is_finite()
    {
        return serializer.serialize_i64(f as i64);
    }
    value.serialize(serializer)
}
