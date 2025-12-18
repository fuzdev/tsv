//! Control flow block printing (if, each, await, key, snippet)
//!
//! Uses a unified approach for block body formatting:
//! - `format_block_body` handles inline vs multiline formatting for all block types
//! - Individual block methods handle their specific opening/continuation/closing tags

// Svelte template syntax uses `{:else}`, `{#if}`, etc. which look like format args
#![allow(clippy::literal_string_with_formatting_args)]

use super::Printer;
use crate::ast::internal::{self, Fragment, FragmentNode};

impl<'a> Printer<'a> {
    // =========================================================================
    // Unified block body formatting
    // =========================================================================

    /// Format a block body (fragment) with inline or multiline formatting
    ///
    /// This is the unified entry point for all block body formatting.
    /// Returns whether the body was formatted inline (for continuation/closing decisions).
    fn format_block_body(&mut self, fragment: &Fragment) -> bool {
        let is_inline = self.is_inline_fragment(fragment);
        if is_inline {
            self.print_inline_children(fragment);
        } else {
            self.print_block_children(fragment);
        }
        is_inline
    }

    /// Write a continuation tag ({:else}, {:then}, etc.) with proper formatting
    fn write_continuation(&mut self, keyword: &str, parent_inline: bool) {
        if !parent_inline {
            self.write("\n");
            self.write_indent();
        }
        self.write(keyword);
    }

    /// Write a closing tag ({/if}, {/each}, etc.) with proper formatting
    fn write_closing(&mut self, keyword: &str, is_inline: bool) {
        if is_inline {
            self.write(keyword);
        } else {
            self.write("\n");
            self.write_indent();
            self.write(keyword);
        }
    }

    // =========================================================================
    // If block
    // =========================================================================

    /// Format an if block: {#if test}...{:else}...{/if}
    pub(super) fn print_if_block(&mut self, block: &internal::IfBlock) {
        // Opening tag
        self.write("{#if ");
        self.print_ts_expression_with_comments(
            &block.test,
            block.opening_tag_span.start,
            block.opening_tag_span.end,
        );
        self.write("}");

        // Consequent body
        let is_inline = self.format_block_body(&block.consequent);

        // Handle alternate (else/else-if chain)
        if let Some(alt) = &block.alternate {
            self.print_if_alternate(alt, is_inline);
        }

        // Closing tag
        self.write_closing("{/if}", is_inline);
    }

    /// Print the alternate branch of an if block (recursive for else-if chains)
    pub(super) fn print_if_alternate(&mut self, alt: &Fragment, parent_inline: bool) {
        // Check if this is an else-if or plain else
        // Normalize {:else}{#if} to {:else if} when else contains only the if block
        // (ignoring whitespace-only text nodes which get stripped anyway)
        let is_only_if_block = alt.nodes.iter().all(|n| match n {
            FragmentNode::IfBlock(_) => true,
            FragmentNode::Text(t) => t.raw.trim().is_empty(),
            _ => false,
        });
        if let Some(FragmentNode::IfBlock(else_if)) = alt
            .nodes
            .iter()
            .find(|n| matches!(n, FragmentNode::IfBlock(_)))
            && (else_if.elseif || is_only_if_block)
        {
            // {:else if condition}
            self.write_continuation("{:else if ", parent_inline);
            self.print_ts_expression_with_comments(
                &else_if.test,
                else_if.opening_tag_span.start,
                else_if.opening_tag_span.end,
            );
            self.write("}");

            let is_inline = self.format_block_body(&else_if.consequent);

            // Recurse for nested alternates
            if let Some(nested_alt) = &else_if.alternate {
                self.print_if_alternate(nested_alt, is_inline);
            }
            return;
        }

        // Plain {:else}
        self.write_continuation("{:else}", parent_inline);
        self.format_block_body(alt);
    }

    // =========================================================================
    // Each block
    // =========================================================================

    /// Format an each block: {#each expr as item}...{/each}
    pub(super) fn print_each_block(&mut self, block: &internal::EachBlock) {
        // Opening tag
        self.write("{#each ");

        // For collection expression, only look for trailing comments up to before "as"
        // to avoid capturing comments that belong to the key expression
        let expr_comment_end = block
            .context
            .as_ref()
            .map_or(block.opening_tag_span.end, |c| c.span().start);
        self.print_ts_expression_with_comments(
            &block.expression,
            block.opening_tag_span.start,
            expr_comment_end,
        );

        if let Some(context) = &block.context {
            self.write(" as ");
            self.print_ts_pattern(context);
            if let Some(idx) = &block.index {
                self.write(", ");
                self.write(idx);
            }
            if let Some(key) = &block.key {
                self.write(" (");
                // Use key_span for comment lookup (includes parentheses)
                if let Some(key_span) = block.key_span {
                    self.print_ts_expression_with_comments(key, key_span.start, key_span.end);
                } else {
                    self.print_ts_expression(key);
                }
                self.write(")");
            }
        } else if let Some(idx) = &block.index {
            self.write(", ");
            self.write(idx);
        }
        self.write("}");

        // Body
        let is_inline = self.format_block_body(&block.body);

        // Fallback
        if let Some(fallback) = &block.fallback {
            self.write_continuation("{:else}", is_inline);
            self.format_block_body(fallback);
        }

        // Closing tag
        self.write_closing("{/each}", is_inline);
    }

    // =========================================================================
    // Await block
    // =========================================================================

    /// Format an await block: {#await expr}...{:then}...{:catch}...{/await}
    pub(super) fn print_await_block(&mut self, block: &internal::AwaitBlock) {
        // Detect shorthand syntax
        let is_shorthand_then = block.pending.is_none() && block.value.is_some();
        let is_shorthand_catch =
            block.pending.is_none() && block.value.is_none() && block.error.is_some();

        // Opening tag
        self.write("{#await ");
        self.print_ts_expression_with_comments(
            &block.expression,
            block.opening_tag_span.start,
            block.opening_tag_span.end,
        );

        // Determine main fragment for inline detection
        let main_fragment = if is_shorthand_then {
            block.then.as_ref()
        } else if is_shorthand_catch {
            block.catch.as_ref()
        } else {
            block.pending.as_ref()
        };
        let is_inline = main_fragment.is_none_or(|f| self.is_inline_fragment(f));

        if is_shorthand_then {
            // {#await expr then value}
            if let Some(value) = &block.value {
                self.write(" then ");
                self.write(value.span().extract(self.source));
            }
            self.write("}");
            if let Some(then_block) = &block.then {
                self.format_block_body(then_block);
            }
        } else if is_shorthand_catch {
            // {#await expr catch error}
            if let Some(error) = &block.error {
                self.write(" catch ");
                self.write(error.span().extract(self.source));
            }
            self.write("}");
            if let Some(catch_block) = &block.catch {
                self.format_block_body(catch_block);
            }
        } else {
            // Full form: {#await expr}...{:then value}...{:catch error}
            self.write("}");
            if let Some(pending) = &block.pending {
                self.format_block_body(pending);
            }

            if let Some(then_block) = &block.then {
                self.write_continuation("{:then", is_inline);
                if let Some(value) = &block.value {
                    self.write(" ");
                    self.write(value.span().extract(self.source));
                }
                self.write("}");
                self.format_block_body(then_block);
            }

            if let Some(catch_block) = &block.catch {
                self.write_continuation("{:catch", is_inline);
                if let Some(error) = &block.error {
                    self.write(" ");
                    self.write(error.span().extract(self.source));
                }
                self.write("}");
                self.format_block_body(catch_block);
            }
        }

        // Closing tag
        self.write_closing("{/await}", is_inline);
    }

    // =========================================================================
    // Key block
    // =========================================================================

    /// Format a key block: {#key expr}...{/key}
    pub(super) fn print_key_block(&mut self, block: &internal::KeyBlock) {
        self.write("{#key ");
        self.print_ts_expression_with_comments(
            &block.expression,
            block.opening_tag_span.start,
            block.opening_tag_span.end,
        );
        self.write("}");

        let is_inline = self.format_block_body(&block.fragment);
        self.write_closing("{/key}", is_inline);
    }

    // =========================================================================
    // Snippet block
    // =========================================================================

    /// Format a snippet block: {#snippet name(params)}...{/snippet}
    pub(super) fn print_snippet_block(&mut self, block: &internal::SnippetBlock) {
        self.write("{#snippet ");
        self.write(block.expression.span().extract(self.source));

        // Type parameters (generics)
        if let Some(ref type_params) = block.type_parameters {
            self.write("<");
            self.write(type_params);
            self.write(">");
        }

        // Parameters
        self.write("(");
        if let Some(ref raw_params) = block.raw_parameters {
            self.write(raw_params);
        } else {
            for (i, param) in block.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.print_ts_expression(param);
            }
        }
        self.write(")}");

        let is_inline = self.format_block_body(&block.body);
        self.write_closing("{/snippet}", is_inline);
    }
}
