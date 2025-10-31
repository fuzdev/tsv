//! HTML-specific classification and whitespace rules
//!
//! This crate provides pure functions for HTML element classification
//! and whitespace preservation rules. These language-level utilities are
//! independent of any specific tool (formatter, linter, type-checker, etc.)

mod elements;
mod whitespace;

// Re-export public API
pub use elements::{is_block_element, is_inline_element, is_void_element};
pub use whitespace::preserves_whitespace;
