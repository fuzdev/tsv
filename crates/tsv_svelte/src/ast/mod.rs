// Svelte AST module
//
// Two-AST architecture:
// - internal: Optimized for manipulation (string interning, compact representation)
// - public: JSON-compatible (matches Svelte parser output, serde support)

#[cfg(feature = "convert")]
pub mod convert;
pub mod internal;
#[cfg(feature = "convert")]
pub mod public;

// Re-export commonly used types
pub use internal::{
    Attribute, AttributeValue, AwaitBlock, EachBlock, Element, ExpressionTag, Fragment,
    FragmentNode, IfBlock, KeyBlock, Root, Script, ScriptContext, Style, Text,
};
