// Node-specific formatting for Svelte template nodes
//
// Handles formatting of different node types in Svelte templates:
// - Elements (HTML tags and Svelte components)
// - Expression tags ({expr})
// - Future: Control flow ({#if}, {#each}, etc.)
// - Future: Special tags ({@html}, {@debug}, etc.)
//
// Includes inline run grouping logic to preserve source layout and
// maintain semantic whitespace correctness.
//
// ## Module Organization
//
// This module is split into focused submodules:
// - **element.rs** - Element formatting (format_element, should_format_multiline)
// - **children.rs** - Child formatting in multiline and compact modes
// - **inline_runs.rs** - Inline run detection and formatting
// - **helpers.rs** - Utilities (expression tags, source position tracking)

// Submodules are compiled together to form the complete Printer implementation.
// Since we're implementing methods on Printer via impl blocks in each module,
// they don't need explicit re-exports - they're all part of the same impl.

mod children;
mod element;
mod helpers;
mod inline_runs;
