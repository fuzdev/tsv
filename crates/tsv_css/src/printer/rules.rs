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
use super::boundary_ws::Edge;
use crate::ast::internal;

impl<'a> Printer<'a> {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn print_css_rule(&mut self, rule: &internal::CssRule<'_>) {
        // Format selector (uses selectors module). A trailing hex-escape terminator
        // (`.\1F600 ` before `{`) is dropped at the selector leaf, so the block
        // separator below can't double it.
        self.print_selector_list(&rule.selector);

        // A boundary run the parser skipped between the prelude and the `{` has no node to
        // ride out on, so this gap's claim prints it — and, where the gap holds a member, the
        // gap's comments with it, in place (`spell_gap_items`), since printed apart the two
        // land in whichever order their emitters run. Its right edge is flush, the block
        // opener writing the ` {` itself; its left edge keeps the author's separation from
        // the selector (`a <ZWNBSP>{` → `a <ZWNBSP> {`), and a name there forces it.
        let kept = self.spell_gap_items(
            rule.selector.span.end,
            rule.block_span.start,
            self.name_run_edge(rule.selector.span.end),
            Edge::Flush,
        );

        // Inline-print any comments between the selector and opening brace — unless the
        // claim above already printed them in place. block_span.start is the position of the
        // opening brace, so a leading child whose span precedes it is a pre-brace comment.
        let mut start_index = 0;
        while let Some(internal::CssBlockChild::Comment(comment)) =
            rule.declarations.get(start_index)
        {
            if comment.span.start >= rule.block_span.start {
                break;
            }
            if kept.is_empty() {
                // Always add space before comment for readability (normalize)
                // This is an intentional divergence from prettier (which preserves no-space)
                self.write(" ");
                self.print_css_comment(comment);
            }
            start_index += 1; // Skip this comment when processing declarations
        }
        if !kept.is_empty() {
            self.write(&kept);
        }

        self.write_block_open();

        // Format declarations and comments with indentation, via the shared
        // block-body routine (also used by at-rule blocks). `start_index` skips
        // the pre-brace comments consumed inline above.
        self.indent_level += 1;
        self.print_css_block_children(rule.declarations, start_index);
        self.indent_level -= 1;

        self.write_indent();
        // The block's own children only: a pre-brace comment is a child too, and flooring the
        // tail's sweep on one would reach back across the `{` and print the pre-brace run a
        // second time, inside the block, on every pass.
        self.write_block_tail_boundary_ws(&rule.declarations[start_index..], rule.block_span);
        self.write("}");
    }
}
