//! Fragment analysis and child printing helpers

use super::Printer;
use crate::ast::internal::{AwaitBlock, EachBlock, Fragment, FragmentNode, IfBlock, KeyBlock};

/// Check if we're inside a template literal after processing this line.
///
/// Template literals span multiple lines when they contain actual newlines.
/// Lines that are inside a template literal should not be re-indented
/// because the whitespace is part of the template content.
///
/// This function counts unescaped backticks to track template literal state.
/// It also ignores backticks inside string literals (single/double quoted).
pub(super) fn is_inside_template_literal(line: &str, was_inside: bool) -> bool {
    let mut in_template = was_inside;
    let mut in_string: Option<char> = None;
    let mut chars = line.chars();

    while let Some(c) = chars.next() {
        // Handle escape sequences
        if c == '\\' {
            chars.next(); // Skip the escaped character
            continue;
        }

        // Handle string literals
        if in_string.is_none() && (c == '"' || c == '\'') {
            in_string = Some(c);
            continue;
        }
        if Some(c) == in_string {
            in_string = None;
            continue;
        }

        // Track template literals (only when not in a string)
        if in_string.is_none() && c == '`' {
            in_template = !in_template;
        }
    }

    in_template
}

impl<'a> Printer<'a> {
    /// Check if a fragment's content is inline (no newlines in source)
    pub(super) fn is_inline_fragment(&self, fragment: &Fragment) -> bool {
        let (Some(first), Some(last)) = (fragment.nodes.first(), fragment.nodes.last()) else {
            return true;
        };
        let first_start = first.span().start_usize();
        let last_end = last.span().end_usize();
        let content = &self.source[first_start..last_end];
        !content.contains('\n')
    }

    // =========================================================================
    // Suffix width calculation for width-aware expression wrapping
    // =========================================================================

    /// Calculate the suffix width for an {#each} block expression.
    ///
    /// The suffix includes everything after the expression that must fit on the same line:
    /// - ` as pattern` (if context exists)
    /// - `, index` (if index exists)
    /// - ` (key)` (if key exists)
    /// - `}`
    /// - inline body content (if body is inline)
    /// - `{/each}` (7 chars)
    pub(super) fn calculate_each_suffix_width(&self, block: &EachBlock) -> usize {
        let mut width = 0;

        // Pattern: " as pattern"
        if let Some(context) = &block.context {
            width += 4; // " as "
            // Use source span for pattern width (avoids re-formatting)
            let pattern_span = context.span();
            width += (pattern_span.end - pattern_span.start) as usize;
        }

        // Index: ", index"
        if let Some(idx) = &block.index {
            width += 2; // ", "
            width += idx.len();
        }

        // Key: " (key)"
        if let Some(key_span) = block.key_span {
            // key_span includes the parentheses, so use it directly
            width += 1; // space before "("
            width += (key_span.end - key_span.start) as usize;
        }

        // Closing brace: "}"
        width += 1;

        // If body is inline, add body width + closing tag
        if self.is_inline_fragment(&block.body) {
            width += self.estimate_inline_fragment_width(&block.body);
            width += 7; // "{/each}"
        }

        width
    }

    /// Calculate the suffix width for an {#await} block expression.
    ///
    /// The suffix includes everything after the expression that must fit on the same line:
    /// - For shorthand `then`: ` then VALUE}` + inline body + `{/await}`
    /// - For shorthand `catch`: ` catch ERROR}` + inline body + `{/await}`
    /// - For full form: `}` + inline body + continuation tags
    pub(super) fn calculate_await_suffix_width(&self, block: &AwaitBlock) -> usize {
        let mut width = 0;

        // Detect shorthand syntax
        let is_shorthand_then = block.pending.is_none() && block.value.is_some();
        let is_shorthand_catch =
            block.pending.is_none() && block.value.is_none() && block.error.is_some();

        if is_shorthand_then {
            // " then VALUE}"
            width += 6; // " then "
            if let Some(value) = &block.value {
                let span = value.span();
                width += (span.end - span.start) as usize;
            }
            width += 1; // "}"

            // Inline body + closing tag
            if let Some(then_block) = &block.then
                && self.is_inline_fragment(then_block)
            {
                width += self.estimate_inline_fragment_width(then_block);
                width += 8; // "{/await}"
            }
        } else if is_shorthand_catch {
            // " catch ERROR}"
            width += 7; // " catch "
            if let Some(error) = &block.error {
                let span = error.span();
                width += (span.end - span.start) as usize;
            }
            width += 1; // "}"

            // Inline body + closing tag
            if let Some(catch_block) = &block.catch
                && self.is_inline_fragment(catch_block)
            {
                width += self.estimate_inline_fragment_width(catch_block);
                width += 8; // "{/await}"
            }
        } else {
            // Full form: "}"
            width += 1;

            // Inline pending body + continuation tags
            if let Some(pending) = &block.pending
                && self.is_inline_fragment(pending)
            {
                width += self.estimate_inline_fragment_width(pending);

                // Add {:then VALUE} if present
                if let Some(then_block) = &block.then {
                    width += 7; // "{:then}"
                    if let Some(value) = &block.value {
                        let span = value.span();
                        width += 1 + (span.end - span.start) as usize; // " VALUE"
                    }
                    // Add inline then body
                    if self.is_inline_fragment(then_block) {
                        width += self.estimate_inline_fragment_width(then_block);
                    }
                    // Add {:catch} or {/await}
                    width += 8; // "{:catch}" or "{/await}" (same length)
                } else {
                    width += 8; // "{/await}"
                }
            }
        }

        width
    }

    /// Calculate the suffix width for an {#if} block expression.
    ///
    /// The suffix includes:
    /// - `}`
    /// - inline body content (if body is inline)
    /// - `{/if}` or continuation tags
    pub(super) fn calculate_if_suffix_width(&self, block: &IfBlock) -> usize {
        let mut width = 1; // "}"

        // If body is inline, add body width + closing tag
        if self.is_inline_fragment(&block.consequent) {
            width += self.estimate_inline_fragment_width(&block.consequent);
            // Check for else branch
            if block.alternate.is_some() {
                // Conservative: assume {:else} or {:else if} adds more
                width += 7; // "{:else}"
            } else {
                width += 5; // "{/if}"
            }
        }

        width
    }

    /// Calculate the suffix width for an {#key} block expression.
    ///
    /// The suffix includes:
    /// - `}`
    /// - inline body content (if body is inline)
    /// - `{/key}` (6 chars)
    pub(super) fn calculate_key_suffix_width(&self, block: &KeyBlock) -> usize {
        let mut width = 1; // "}"

        // If body is inline, add body width + closing tag
        if self.is_inline_fragment(&block.fragment) {
            width += self.estimate_inline_fragment_width(&block.fragment);
            width += 6; // "{/key}"
        }

        width
    }

    /// Estimate the formatted width of an inline fragment.
    ///
    /// Uses source span lengths as a reasonable approximation since inline
    /// fragments typically don't change much during formatting.
    fn estimate_inline_fragment_width(&self, fragment: &Fragment) -> usize {
        let (Some(first), Some(last)) = (fragment.nodes.first(), fragment.nodes.last()) else {
            return 0;
        };

        // For inline fragments, source length is a good estimate
        // (formatting mostly preserves length for simple content)
        let start = first.span().start_usize();
        let end = last.span().end_usize();

        // Trim leading/trailing whitespace from the estimate
        let content = &self.source[start..end];
        content.trim().len()
    }

    /// Print children inline (no newlines added)
    pub(super) fn print_inline_children(&mut self, fragment: &Fragment) {
        for node in &fragment.nodes {
            match node {
                FragmentNode::Text(text) => {
                    // For inline, preserve trimmed text
                    let trimmed = text.raw.trim();
                    if !trimmed.is_empty() {
                        self.write(trimmed);
                    }
                }
                _ => {
                    self.print_fragment_node(node, false, false);
                }
            }
        }
    }

    /// Helper to print children of a block with proper indentation
    pub(super) fn print_block_children(&mut self, fragment: &Fragment) {
        // For now, just print each child on a new line with indentation
        if fragment.nodes.is_empty() {
            return;
        }
        self.indent_level += 1;
        for node in &fragment.nodes {
            match node {
                FragmentNode::Text(text) => {
                    if !text.raw.trim().is_empty() {
                        self.write("\n");
                        self.write_indent();
                        self.write(text.raw.trim());
                    }
                }
                _ => {
                    self.write("\n");
                    self.write_indent();
                    self.print_fragment_node(node, true, false);
                }
            }
        }
        self.indent_level -= 1;
    }
}
