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
//! This module uses doc builders where practical. The complex block child
//! handling with inline comments and blank line preservation is still
//! imperative for clarity.

use super::Printer;
use crate::ast::internal;

impl<'a> Printer<'a> {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn print_css_rule(&mut self, rule: &internal::CssRule<'_>) {
        // Format selector (uses selectors module)
        self.print_selector_list(&rule.selector);

        // Drop a hex escape's terminator whitespace (`.\1F600 ` before `{`) so it
        // doesn't double with the block separator written below.
        self.pop_selector_terminator();

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
