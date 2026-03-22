// Public AST types - with serde, for JSON output
// Uses u32 for positions (max 4GB file size) for memory efficiency

use serde::{Deserialize, Serialize};

/// StyleSheet - CSS content with parsed AST
///
/// Represents a <style> tag's parsed CSS content.
/// Used when CSS is embedded in Svelte components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleSheet {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub attributes: Vec<serde_json::Value>, // Attributes from <style> tag
    pub children: Vec<serde_json::Value>,   // CSS AST nodes (Rules, etc.)
    pub content: StyleContent,
}

/// StyleSheet content - raw CSS text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleContent {
    pub start: u32,
    pub end: u32,
    pub styles: String,
    pub comment: Option<serde_json::Value>,
}
