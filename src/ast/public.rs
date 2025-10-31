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

// ============= Svelte Nodes =============

/// Svelte Root node - top level of a .svelte file
///
/// Serializes to match Svelte's parser output exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: Option<u32>,
    pub end: Option<u32>,
    pub fragment: Fragment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<Script>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<Script>,
    pub css: Option<StyleSheet>,
    pub js: Vec<serde_json::Value>,         // empty array for now
    pub options: Option<serde_json::Value>, // null for now
    pub comments: Vec<serde_json::Value>,   // empty array for now
}

/// Svelte Fragment - container for template nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fragment {
    #[serde(rename = "type")]
    pub node_type: String,
    pub nodes: Vec<FragmentNode>,
}

/// Svelte template node types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FragmentNode {
    RegularElement(Element),
    ExpressionTag(ExpressionTag),
    Text(Text),
}

/// Svelte Element - HTML/component tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub fragment: Fragment,
}

/// Svelte Attribute - element attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

/// Svelte Attribute value part
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    Text(Text),
    // TODO(Future sprint): ExpressionTag
    // For dynamic attribute values like: <div class={expr}>
    // Sprint 6 implemented template-level expression tags only.
    // Note: Using untagged here because Text already has its own "type" field
}

/// Svelte Text node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Text {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub raw: String,
    pub data: String,
}

/// Svelte ExpressionTag - {expression} in template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionTag {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
}

/// Svelte Script block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub context: String, // "default" or "module"
    pub content: Program,
    pub attributes: Vec<Attribute>,
}

/// Svelte StyleSheet - CSS in <style> tags
///
/// For minimal CSS support, we use serde_json::Value for children
/// to avoid implementing a full CSS parser yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleSheet {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub attributes: Vec<Attribute>,
    pub children: Vec<serde_json::Value>, // CSS AST nodes (placeholder for now)
    pub content: StyleContent,
}

/// StyleSheet content - raw CSS text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleContent {
    pub start: u32,
    pub end: u32,
    pub styles: String,
    pub comment: Option<String>,
}
