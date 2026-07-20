//! The compiler test suite, split one concern per file.
//!
//! Shared assertion helpers live in `support`; a new test belongs in the file
//! matching the feature it exercises.

mod support;

mod assignment;
mod attributes;
mod bind;
mod blocks;
mod boundary;
mod canonicalize;
mod class_directives;
mod comments;
mod components;
mod context_wrapper;
mod css_scope;
mod dollar_bindings;
mod dropped_and_special;
mod element_refusals;
mod element_spread;
mod errors;
mod module_script;
mod refusal_buckets;
mod rune_store_collision;
mod runes_derived;
mod runes_misc;
mod runes_state;
mod script_rewrite;
mod slots_and_head;
mod snippets;
mod stores;
mod style_directives;
mod text_emission;
mod transitions;
mod typescript;
mod validate;
