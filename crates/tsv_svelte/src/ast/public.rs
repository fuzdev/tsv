// Svelte public AST types
//
// JSON-compatible representation matching Svelte's official parser output.
// Used for serialization and external tool compatibility.

use serde::{Deserialize, Serialize};
use tsv_css::ast::public::StyleSheet;
use tsv_ts::ast::public::{Expression, Program};

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
