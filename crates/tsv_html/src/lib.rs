//! HTML-specific classification and whitespace rules
//!
//! This crate provides pure functions for HTML element classification
//! and whitespace preservation rules. These language-level utilities are
//! independent of any specific tool (printer, linter, type-checker, etc.)

mod elements;
mod entities;
mod whitespace;

// Re-export public API
pub use elements::{
    closing_tag_omitted, is_block_element, is_foreign_element, is_mathml_element, is_svg_element,
    is_void_element,
};
pub use entities::decode_character_references;
pub use whitespace::preserves_whitespace;
