// Svelte public AST types
//
// JSON-compatible representation matching Svelte's official parser output.
// Used for serialization and external tool compatibility.

use crate::ast::internal::ElementKind;
use serde::{Deserialize, Serialize};
use tsv_css::ast::public::StyleSheet;
use tsv_ts::ast::public::Expression;

/// Svelte Root node - top level of a .svelte file
///
/// Serializes to match Svelte's parser output exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    pub css: Option<StyleSheet>,
    pub js: Vec<serde_json::Value>, // empty array for now
    pub start: Option<u32>,
    pub end: Option<u32>,
    #[serde(rename = "type")]
    pub node_type: String,
    pub fragment: Fragment,
    pub options: Option<serde_json::Value>, // null for now
    pub comments: Vec<serde_json::Value>,   // empty array for now
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<Script>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<Script>,
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
    Component(Element),
    RegularElement(Element),
    ExpressionTag(ExpressionTag),
    Text(Text),
    Comment(Comment),
    IfBlock(IfBlock),
    EachBlock(EachBlock),
    AwaitBlock(AwaitBlock),
    KeyBlock(KeyBlock),
}

/// Svelte HTML Comment node: <!-- content -->
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub data: String,
}

/// Svelte Element - HTML/component tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    #[serde(skip_serializing)]
    pub kind: ElementKind,
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
    ExpressionTag(ExpressionTag),
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
    pub context: String,            // "default" or "module"
    pub content: serde_json::Value, // Program with leadingComments/trailingComments injected
    pub attributes: Vec<Attribute>,
}

/// Svelte IfBlock - conditional rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub elseif: bool,
    pub test: Expression,
    pub consequent: Fragment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternate: Option<Fragment>,
}

/// Svelte EachBlock - list iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EachBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    /// None when no `as` clause: {#each expr} or {#each expr, index}
    pub context: Option<Expression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Expression>,
    pub body: Fragment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Fragment>,
}

/// Svelte AwaitBlock - promise handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Expression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Expression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<Fragment>,
    #[serde(rename = "then", skip_serializing_if = "Option::is_none")]
    pub then_block: Option<Fragment>,
    #[serde(rename = "catch", skip_serializing_if = "Option::is_none")]
    pub catch_block: Option<Fragment>,
}

/// Svelte KeyBlock - keyed updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    pub fragment: Fragment,
}
