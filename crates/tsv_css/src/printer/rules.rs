//! CSS rule printing
//!
//! Handles formatting of:
//! - CSS rules (selector + declarations block)
//! - Comment placement within rules
//! - Blank line preservation between declarations
//!
//! Declaration and value printing are handled by separate modules.
//!
//! ## Architecture
//!
//! This module handles a rule's selector, the pre-brace comments, and the braces.
//! The block body itself — declarations/comments/nested rules with inline comments,
//! blank-line preservation, and format-ignore — is iterated by the shared
//! `print_css_block_children` (see `mod.rs`), also used by at-rule blocks.

use super::Printer;
use crate::ast::internal;

impl<'a> Printer<'a> {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn print_css_rule(&mut self, rule: &internal::CssRule<'_>) {
        // Format selector (uses selectors module). A trailing hex-escape terminator
        // (`.\1F600 ` before `{`) is dropped at the selector leaf, so the block
        // separator below can't double it.
        self.print_selector_list(&rule.selector);

        // Inline-print any comments between the selector and opening brace.
        // block_span.start is the position of the opening brace, so a leading
        // child whose span precedes it is a pre-brace comment.
        let mut start_index = 0;
        while let Some(internal::CssBlockChild::Comment(comment)) =
            rule.declarations.get(start_index)
        {
            if comment.span.start >= rule.block_span.start {
                break;
            }
            // Always add space before comment for readability (normalize)
            // This is an intentional divergence from prettier (which preserves no-space)
            self.write(" ");
            self.print_css_comment(comment);
            start_index += 1; // Skip this comment when processing declarations
        }

        self.write(" {\n");

        // Format declarations and comments with indentation, via the shared
        // block-body routine (also used by at-rule blocks). `start_index` skips
        // the pre-brace comments consumed inline above.
        self.indent_level += 1;
        self.print_css_block_children(rule.declarations, start_index);
        self.indent_level -= 1;

        self.write_indent();
        self.write("}");
    }
}
