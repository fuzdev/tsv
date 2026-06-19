// Node-specific formatting for Svelte template nodes
//
// ## Module Organization
//
// The `_doc` suffix distinguishes doc-IR builders from printer entry points.
//
// - **element.rs** - Element entry points (print_element, print_special_element)
// - **fragment_doc.rs** - Core doc-based fragment formatting (text fill, node dispatch)
// - **blocks_doc.rs** - Doc-based formatting for control flow blocks ({#if}, {#each}, etc.)
// - **tags_doc.rs** - Doc-based formatting for template tags ({@html}, {@const}, etc.)
// - **element_doc.rs** - Doc-based formatting for regular HTML/component elements
// - **special_doc.rs** - Doc-based formatting for svelte:* special elements
// - **helpers.rs** - Shared utilities (expression tags, pattern/expression doc builders, source position tracking)
//
// Note: Control flow block and template tag entry points are in ../blocks.rs
// and ../tags.rs

mod blocks_doc;
mod element;
mod element_doc;
mod fragment_doc;
mod helpers;
mod special_doc;
mod tags_doc;

// Shared with the root-fragment printer (`crate::printer::mod`) for run detection.
pub(crate) use helpers::is_control_flow_block;
