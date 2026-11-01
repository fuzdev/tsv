//! HTML spec data and the answers the HTML spec gives — part of
//! [tsv](https://github.com/fuzdev/tsv), published for tsv's Svelte support
//! ([`tsv_svelte`](https://docs.rs/tsv_svelte)). The long-term target is a spec-faithful HTML
//! library; today the crate holds what tsv's Svelte parser reads from the spec: the
//! [named character references][table] table, matched as the tokenizer matches it
//! ([`match_named_character_reference`], answering a [`NamedCharacterReference`]).
//!
//! The lookup is a pure function over text, independent of any one parser or tool. Names are
//! matched exactly, as authored: the lookup is case-sensitive, as the spec's table is
//! (`&Aacute;` and `&aacute;` are different references).
//!
//! ```
//! use tsv_html::match_named_character_reference;
//!
//! // `&lt;` names `<`, its `;` included
//! let lt = match_named_character_reference("lt;").unwrap();
//! assert_eq!((lt.name(), lt.characters()), ("lt;", "<"));
//! // the longest name that starts the text wins: `&notin;` over `&not`
//! let notin = match_named_character_reference("notin;").unwrap();
//! assert_eq!((notin.name(), notin.characters()), ("notin;", "∉"));
//! ```
//!
//! ## Stability
//!
//! The public signatures follow 0.x semver like every tsv crate: a breaking change ships only
//! in a minor release. What the lookup answers may change in any release, tracking the spec
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

mod entities;
#[cfg(test)]
mod entities_gen;
mod entities_table;

pub use entities::{NamedCharacterReference, match_named_character_reference};
