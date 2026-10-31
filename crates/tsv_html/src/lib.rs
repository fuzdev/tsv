//! HTML element classification, whitespace rules and character-reference decoding — part
//! of [tsv](https://github.com/fuzdev/tsv).
//!
//! **Support crate for tsv**, used by [`tsv_svelte`](https://docs.rs/tsv_svelte) — no
//! stability promise yet. Its answers follow Svelte's reading of HTML rather than the
//! spec's; the goal is to grow it into a standalone HTML library later.
//!
//! Pure functions over tag names and text, independent of any one tool (formatter, linter,
//! type checker). The whitespace sets are the ones Svelte's compiler and
//! prettier-plugin-svelte apply — [`collapses_child_whitespace`] is Svelte's exact set — so
//! they answer for a Svelte-compatible formatter, not for every HTML tool.
//!
//! **Tag names are compared exactly, as authored.** Every predicate is case-sensitive and
//! matches a name in its canonical spelling — lowercase for HTML and MathML, SVG's own
//! camelCase where it has one (`clipPath`, `foreignObject`) — so `"DIV"` or `"Br"` is no
//! member of any set. A caller holding a name from case-insensitive HTML lowercases it first
//! (Svelte's compiler lowercases an html-namespace tag before asking `is_void`); a Svelte
//! parser hands the name as written, since an uppercase initial makes it a component.
//!
//! **Naming.** A verb predicate says what an element *does* —
//! [`preserves_whitespace`], [`collapses_child_whitespace`]; an `is_*` predicate says which
//! *category* it belongs to — [`is_void_element`], [`is_block_element`],
//! [`is_line_break_element`].
//!
//! ```
//! use tsv_html::{
//!     ReferenceContext, decode_character_references, is_void_element, preserves_whitespace,
//! };
//!
//! assert!(is_void_element("br"));
//! assert!(preserves_whitespace("pre"));
//! assert_eq!(decode_character_references("&lt;p&gt;", ReferenceContext::Text), "<p>");
//! ```
//!
//! The named-reference table is generated from WHATWG's `entities.json`; the crate's
//! `NOTICE` file carries its license.

#![warn(missing_docs)]

mod elements;
mod entities;
#[cfg(test)]
mod entities_gen;
mod entities_table;
mod whitespace;

// Re-export public API
pub use elements::{
    OptionalEndTag, collapses_child_whitespace, is_block_element, is_custom_element_name_char,
    is_foreign_element, is_line_break_element, is_mathml_element, is_svg_element, is_void_element,
    optional_end_tag,
};
pub use entities::{ReferenceContext, decode_character_references};
pub use whitespace::preserves_whitespace;
