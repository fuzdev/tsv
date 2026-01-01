// Node-specific formatting for Svelte template nodes
//
// ## Module Organization
//
// - **element.rs** - Element entry points (print_element, print_special_element)
// - **fragment_doc.rs** - Core doc-based fragment formatting, control flow blocks, template tags
// - **element_doc.rs** - Doc-based formatting for regular HTML/component elements
// - **special_doc.rs** - Doc-based formatting for svelte:* special elements
// - **helpers.rs** - Utilities (expression tags, patterns, source position tracking)
//
// Note: Control flow blocks ({#if}, {#each}, etc.) are in ../blocks.rs
// and template tags ({@html}, {@const}, etc.) are in ../tags.rs

mod element;
mod element_doc;
mod fragment_doc;
mod helpers;
mod special_doc;
