//! HTML element classification, whitespace rules and character-reference decoding — part
//! of [tsv](https://github.com/fuzdev/tsv), published for tsv's Svelte support
//! ([`tsv_svelte`](https://docs.rs/tsv_svelte)).
//!
//! The crate has two layers:
//!
//! - **The root** is reserved for answers that follow the HTML spec: today the
//!   optional-end-tag table ([`optional_end_tag`], [`OptionalEndTag`]), the custom-element name
//!   class ([`is_custom_element_name_char`]), and [`ReferenceContext`], the context a
//!   character-reference decoder reads. The long-term target is a spec-faithful HTML library.
//! - **[`svelte`]** holds the answers Svelte's compiler and prettier-plugin-svelte give —
//!   element classification, the whitespace sets, Svelte's character-reference decoder — which
//!   tsv's Svelte support must reproduce exactly. Each item's doc says where it departs from
//!   the spec.
//!
//! Pure functions over tag names and text, independent of any one tool (formatter, linter,
//! type checker).
//!
//! **Tag names are compared exactly, as authored.** Every predicate is case-sensitive and
//! matches a name in its canonical spelling — lowercase for HTML and MathML, SVG's own
//! camelCase where it has one (`clipPath`, `foreignObject`) — so `"DIV"` or `"Br"` is no
//! member of any set. A caller holding a name from case-insensitive HTML lowercases it first
//! (Svelte's compiler lowercases an html-namespace tag before asking `is_void`); a Svelte
//! parser hands the name as written, since an uppercase initial makes it a component.
//!
//! **Naming.** A verb predicate says what an element *does* —
//! [`svelte::preserves_whitespace`], [`svelte::collapses_child_whitespace`]; an `is_*`
//! predicate says which *category* it belongs to — [`svelte::is_void_element`],
//! [`svelte::is_block_element`], [`svelte::is_line_break_element`].
//!
//! ```
//! use tsv_html::optional_end_tag;
//!
//! // `<li>` has an optional end tag, which a following `<li>` closes
//! let li = optional_end_tag("li").expect("an optional end tag");
//! assert!(li.is_closed_by("li"));
//! assert!(optional_end_tag("div").is_none());
//! ```
//!
//! ## Stability
//!
//! The public signatures follow 0.x semver like every tsv crate: a breaking change ships only
//! in a minor release. What the functions answer, and the wording of anything they return,
//! may change in any release, tracking what tsv needs. `#[doc(hidden)]` items are outside
//! semver.
//!
//! ## License
//!
//! The named-reference table is generated from WHATWG's `entities.json` and compiled in
//! under BSD-3-Clause, beside tsv's own 0BSD (the crate's license is `0BSD AND
//! BSD-3-Clause`); the crate's `NOTICE` file carries the attribution and both licenses.

#![warn(missing_docs)]

mod elements;
mod entities;
#[cfg(test)]
mod entities_gen;
mod entities_table;
pub mod svelte;
mod whitespace;

pub use elements::{OptionalEndTag, is_custom_element_name_char, optional_end_tag};
pub use entities::ReferenceContext;
