// HTML language-level semantics and classification
//
// This module contains language-level utilities for HTML/Svelte that are
// independent of any specific tool (formatter, linter, type-checker, etc.)
//
// These pure functions operate on tag names and provide the foundation for
// tool-specific implementations (formatter, linter, type-checker, language server).

mod elements;
mod whitespace;

// Re-export public API
pub use elements::{is_block_element, is_inline_element, is_void_element};
pub use whitespace::preserves_whitespace;
