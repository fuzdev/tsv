//! Control flow block printing (if, each, await, key, snippet)
//!
//! All blocks use doc-based formatting via builders in nodes/blocks_doc.rs.

use super::Printer;
use crate::ast::internal;

impl<'a> Printer<'a> {
    //
    // If block
    //

    /// Format an if block: {#if test}...{:else}...{/if}
    ///
    /// Uses the doc-based builder which handles:
    /// - Comments in expressions (via build_expression_with_comments_doc)
    /// - Method chain wrapping (via first_line_offset-aware doc building)
    /// - Inline vs multiline formatting (via is_inline_fragment detection)
    /// - Else/else-if chains (via build_if_alternate_doc)
    /// - Nested style/script elements (via build_raw_content_element_doc)
    pub(super) fn print_if_block(&mut self, block: &internal::IfBlock) {
        let doc = self.build_if_block_doc(block);
        self.render_doc_immediate(doc);
    }

    //
    // Each block
    //

    /// Format an each block: {#each items as item, index (key)}...{:else}...{/each}
    ///
    /// Uses the doc-based builder which handles:
    /// - Comments in expressions (via build_expression_with_comments_doc)
    /// - Method chain wrapping (via first_line_offset-aware doc building)
    /// - Inline vs multiline formatting (via is_inline_fragment detection)
    /// - Nested style/script elements (via build_raw_content_element_doc)
    pub(super) fn print_each_block(&mut self, block: &internal::EachBlock) {
        let doc = self.build_each_block_doc(block);
        self.render_doc_immediate(doc);
    }

    //
    // Await block
    //

    /// Format an await block: {#await expr}...{:then value}...{:catch error}...{/await}
    ///
    /// Uses the doc-based builder which handles:
    /// - Comments in expressions (via build_expression_with_comments_doc)
    /// - Method chain wrapping (via first_line_offset-aware doc building)
    /// - Inline vs multiline formatting (via is_inline_fragment detection)
    /// - Shorthand forms ({#await expr then value}, {#await expr catch error})
    /// - Nested style/script elements (via build_raw_content_element_doc)
    pub(super) fn print_await_block(&mut self, block: &internal::AwaitBlock) {
        let doc = self.build_await_block_doc(block);
        self.render_doc_immediate(doc);
    }

    //
    // Key block
    //

    /// Format a key block: {#key expr}...{/key}
    ///
    /// Uses the doc-based builder which handles:
    /// - Comments in expressions (via build_expression_with_comments_doc)
    /// - Method chain wrapping (via first_line_offset-aware doc building)
    /// - Inline vs multiline formatting (via is_inline_fragment detection)
    /// - Nested style/script elements (via build_raw_content_element_doc)
    pub(super) fn print_key_block(&mut self, block: &internal::KeyBlock) {
        let doc = self.build_key_block_doc(block);
        self.render_doc_immediate(doc);
    }

    //
    // Snippet block
    //

    /// Format a snippet block: {#snippet name(params)}...{/snippet}
    ///
    /// Uses the doc-based builder which handles:
    /// - Parameter wrapping with proper indentation
    /// - Inline vs multiline formatting (via is_inline_fragment detection)
    /// - Nested style/script elements (via build_raw_content_element_doc)
    pub(super) fn print_snippet_block(&mut self, block: &internal::SnippetBlock) {
        let doc = self.build_snippet_block_doc(block);
        self.render_doc_immediate(doc);
    }
}
