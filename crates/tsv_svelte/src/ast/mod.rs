// Svelte AST module
//
// - internal: the parsed AST (string interning, compact representation)
// - convert: emits the public JSON-compatible wire format (matches Svelte
//   parser output) directly from `internal` via the writer

#[cfg(feature = "convert")]
pub mod convert;
pub mod internal;

// Re-export commonly used types
pub use internal::{
    Attribute, AttributeValue, AwaitBlock, EachBlock, Element, ExpressionTag, Fragment,
    FragmentNode, IfBlock, KeyBlock, Root, Script, ScriptContext, Style, Text,
};
