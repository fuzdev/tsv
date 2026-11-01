//! The answers Svelte's compiler and prettier-plugin-svelte give about HTML, which tsv's
//! Svelte support must reproduce exactly — pure functions over tag names and text, no AST.
//!
//! Each set here is a tool's, not the HTML spec's: [`is_block_element`] is
//! prettier-plugin-svelte's block list, [`preserves_whitespace`] and
//! [`collapses_child_whitespace`] are the sets Svelte's compiler applies,
//! [`is_line_break_element`] is the plugin's line-breaking set, [`is_void_element`],
//! [`is_svg_element`], [`is_mathml_element`] and [`is_foreign_element`] are Svelte's name
//! lists, [`is_svelte_tag_name_tail_char`] is the tail class of Svelte's tag-name pattern,
//! [`svelte_autoclosing_row`] is the table Svelte's parser closes an element by when its end
//! tag is omitted, and [`decode_character_references`] decodes as Svelte's parser decodes.
//! Each item's doc says where it departs from the spec. The spec's own data — the named
//! character references table — lives in `tsv_html`, which the decoder builds on.
//!
//! **Tag names are compared exactly, as authored.** Every predicate is case-sensitive and
//! matches a name in its canonical spelling — lowercase for HTML and MathML, SVG's own
//! camelCase where it has one (`clipPath`, `foreignObject`) — so `"DIV"` or `"Br"` is no
//! member of any set. A Svelte parser hands the name as written, since an uppercase initial
//! makes it a component.
//!
//! **Naming.** A verb predicate says what an element *does* — [`preserves_whitespace`],
//! [`collapses_child_whitespace`]; an `is_*` predicate says which *category* it belongs to —
//! [`is_void_element`], [`is_block_element`], [`is_line_break_element`]. Inline-ness is
//! derived by negation in the consumer (prettier-plugin-svelte's `isInline = !isBlock`); no
//! positive list exists.
//!
//! Private to the crate; `tsv_svelte_compile` and `tsv_debug` reach the items they use
//! through `__private`.

mod elements;
mod entities;
mod tags;
mod whitespace;

pub(crate) use elements::{
    collapses_child_whitespace, is_block_element, is_foreign_element, is_line_break_element,
    is_void_element,
};
pub use elements::{is_mathml_element, is_svg_element};
pub(crate) use entities::{ReferenceContext, decode_character_references};
pub(crate) use tags::{is_svelte_tag_name_tail_char, svelte_autoclosing_row};
pub use whitespace::preserves_whitespace;
