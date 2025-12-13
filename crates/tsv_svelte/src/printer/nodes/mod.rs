// Node-specific formatting for Svelte template nodes
//
// Handles formatting of elements and their children in Svelte templates.
// Includes inline run grouping logic to preserve source layout and
// maintain semantic whitespace correctness.
//
// ## Module Organization
//
// - **element.rs** - Element formatting (print_element, should_format_multiline)
// - **children.rs** - Child formatting in multiline and compact modes
// - **inline_runs.rs** - Inline run detection and formatting
// - **helpers.rs** - Utilities (expression tags, patterns, source position tracking)
//
// Note: Control flow blocks ({#if}, {#each}, etc.) are in ../blocks.rs
// and template tags ({@html}, {@const}, etc.) are in ../tags.rs

// Submodules are compiled together to form the complete Printer implementation.
// Since we're implementing methods on Printer via impl blocks in each module,
// they don't need explicit re-exports - they're all part of the same impl.

mod children;
mod element;
mod helpers;
mod inline_runs;
