//! The answers Svelte's compiler and prettier-plugin-svelte give about HTML, which tsv's
//! Svelte support must reproduce exactly.
//!
//! Each set here is a tool's, not the HTML spec's: [`is_block_element`] is
//! prettier-plugin-svelte's block list, [`preserves_whitespace`] and
//! [`collapses_child_whitespace`] are the sets Svelte's compiler applies,
//! [`is_line_break_element`] is the plugin's line-breaking set, [`is_void_element`],
//! [`is_svg_element`], [`is_mathml_element`] and [`is_foreign_element`] are Svelte's name
//! lists, and
//! [`decode_character_references`] decodes as Svelte's parser decodes. Each item's doc says
//! where it departs from the spec.
//!
//! ```
//! use tsv_html::ReferenceContext;
//! use tsv_html::svelte::{decode_character_references, is_void_element, preserves_whitespace};
//!
//! assert!(is_void_element("br"));
//! assert!(preserves_whitespace("pre"));
//! assert_eq!(decode_character_references("&lt;p&gt;", ReferenceContext::Text), "<p>");
//! ```

pub use crate::elements::{
    collapses_child_whitespace, is_block_element, is_foreign_element, is_line_break_element,
    is_mathml_element, is_svg_element, is_void_element,
};
pub use crate::entities::decode_character_references;
pub use crate::whitespace::preserves_whitespace;
