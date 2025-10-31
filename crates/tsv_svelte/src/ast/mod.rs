// Svelte AST module
//
// Two-AST architecture:
// - internal: Optimized for manipulation (string interning, compact representation)
// - public: JSON-compatible (matches Svelte parser output, serde support)

pub mod convert;
pub mod internal;
pub mod public;

// Re-export commonly used types
pub use internal::{
    Attribute, AttributeValue, Element, ExpressionTag, Fragment, FragmentNode, Root, Script,
    ScriptContext, Style, Text,
};
