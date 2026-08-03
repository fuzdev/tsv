// Node-specific formatting for Svelte template nodes
//
// ## Module Organization
//
// The `_doc` suffix distinguishes doc-IR builders; the whole template fragment
// (root and nested) renders through these builders — there is no separate buffer
// printer (the root-fragment renderer lives in `crate::printer::mod`).
//
// - **fragment_doc.rs** - Core doc-based fragment formatting (sibling walk, node dispatch)
// - **fragment_glue_doc.rs** - Byte-glue predicates + glued/welded run docs (unbreakable units)
// - **fragment_text_doc.rs** - Text-child handling + word-fill doc construction
// - **blocks_doc.rs** - Doc-based formatting for control flow blocks ({#if}, {#each}, etc.)
// - **tags_doc.rs** - Doc-based formatting for template tags ({@html}, {@const}, etc.)
// - **element_doc.rs** - Doc-based formatting for regular HTML/component elements
// - **element_analysis.rs** - Element analysis/classification predicates (layout, multiline, boundary modes)
// - **element_ws_sensitive_doc.rs** - Doc-based formatting for whitespace-sensitive elements (pre, textarea)
// - **special_doc.rs** - Doc-based formatting for svelte:* special elements
// - **helpers.rs** - Shared utilities (node classification, pattern/expression doc builders, source position tracking)

mod blocks_doc;
mod element_analysis;
mod element_doc;
mod element_ws_sensitive_doc;
mod fragment_doc;
mod fragment_glue_doc;
mod fragment_text_doc;
mod helpers;
mod special_doc;
mod tags_doc;

// Shared with the root-fragment printer (`crate::printer::mod`) for run detection.
pub(crate) use helpers::is_control_flow_block;

// Shared with the `<svelte:options>` printer (`crate::printer::mod`) — the one tag head
// printed outside the element pipeline, but through the same attribute-list emitter.
pub(in crate::printer) use element_doc::AttrGaps;
