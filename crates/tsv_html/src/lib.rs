//! HTML spec data and the answers the HTML spec gives — part of
//! [tsv](https://github.com/fuzdev/tsv), published for tsv's Svelte support
//! ([`tsv_svelte`](https://docs.rs/tsv_svelte)). The long-term target is a spec-faithful HTML
//! library; today the crate holds what tsv's Svelte parser reads from the spec:
//!
//! - the [named character references][table] table, matched as the tokenizer matches it
//!   ([`match_named_character_reference`]);
//! - the optional-end-tag table ([`optional_end_tag`], [`OptionalEndTag`]);
//! - the custom-element name class ([`is_custom_element_name_char`]).
//!
//! Pure functions over names and text, independent of any one parser or tool. Where an item
//! takes Svelte's reading of the spec rather than the spec's own (a subset of a table, a wider
//! class), its doc says so.
//!
//! **Tag names are compared exactly, as authored.** Every lookup is case-sensitive and
//! matches a name in its canonical lowercase spelling, so `"LI"` has no row; a caller holding
//! a name from case-insensitive HTML lowercases it first.
//!
//! ```
//! use tsv_html::{match_named_character_reference, optional_end_tag};
//!
//! // `<li>` has an optional end tag, which a following `<li>` closes
//! let li = optional_end_tag("li").expect("an optional end tag");
//! assert!(li.is_closed_by("li"));
//! assert!(optional_end_tag("div").is_none());
//!
//! // `&lt;` names `<`, its `;` included
//! assert_eq!(match_named_character_reference("lt;"), Some((3, "<")));
//! ```
//!
//! ## Stability
//!
//! The public signatures follow 0.x semver like every tsv crate: a breaking change ships only
//! in a minor release. What the functions answer may change in any release, tracking the spec
//! and what tsv needs.
//!
//! ## License
//!
//! The named-reference table is generated from WHATWG's `entities.json` and compiled in
//! under BSD-3-Clause, beside tsv's own 0BSD (the crate's license is `0BSD AND
//! BSD-3-Clause`); the crate's `NOTICE` file carries the attribution and both licenses.
//!
//! [table]: https://html.spec.whatwg.org/multipage/named-characters.html#named-character-references

#![warn(missing_docs)]

mod elements;
mod entities;
#[cfg(test)]
mod entities_gen;
mod entities_table;

pub use elements::{OptionalEndTag, is_custom_element_name_char, optional_end_tag};
pub use entities::match_named_character_reference;
