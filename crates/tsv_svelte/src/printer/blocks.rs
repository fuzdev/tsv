//! Control flow block printing (if, each, await, key, snippet)
//!
//! Uses a unified approach for block body formatting:
//! - `format_block_body` handles inline vs multiline formatting for all block types
//! - Individual block methods handle their specific opening/continuation/closing tags

// Svelte template syntax uses `{:else}`, `{#if}`, etc. which look like format args
#![allow(clippy::literal_string_with_formatting_args)]

use std::rc::Rc;

use super::Printer;
use crate::ast::internal::{self, Fragment, FragmentNode};
use tsv_lang::doc;

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
        // For binary expressions (&&, ||), use doc-based formatting for proper wrapping
        // For other expressions (method chains, etc.), use imperative formatting
        let is_binary = matches!(
            &block.test,
            tsv_ts::Expression::BinaryExpression(b) if b.operator.is_logical()
        );

        if is_binary {
            let (opening_doc, render_config) = self.build_if_opening_doc(block);
            self.write_doc_with_config(&opening_doc, &render_config);
        } else {
            // Non-binary expressions: use original imperative approach
            // which handles method chains correctly
            self.write("{#if ");
            let suffix_width = self.calculate_if_suffix_width(block);
            self.print_ts_expression_with_suffix_width(
                &block.test,
                block.opening_tag_span.start,
                block.opening_tag_span.end,
                suffix_width,
            );
            self.write("}");
        }

        // Consequent body
        let is_inline = self.format_block_body(&block.consequent);

        // Handle alternate (else/else-if chain)
        if let Some(alt) = &block.alternate {
            self.print_if_alternate(alt, is_inline);
        }

        // Closing tag
        self.write_closing("{/if}", is_inline);
    }

    /// Build a doc for the {#if condition} opening tag with width-aware wrapping.
    ///
    /// Only called for binary expressions (&&, ||). Returns both the doc and the
    /// config to use for rendering (with first_line_offset and suffix_width set).
    ///
    /// - When fits: `{#if a && b && c}`
    /// - When breaks: `{#if a &&\n\tb &&\n\tc}` (first operand on same line)
    fn build_if_opening_doc(&self, block: &internal::IfBlock) -> (doc::Doc, tsv_lang::PrintConfig) {
        // Calculate first_line_offset and suffix_width for proper line width accounting
        let current_col = self.buffer.current_column(self.config.tab_width);
        let first_line_offset = current_col;

        // Calculate suffix_width to account for body and closing tag after the opening tag
        let suffix_width = self.calculate_if_suffix_width(block).saturating_sub(1);

        // Build condition doc (ungrouped so our group controls breaking)
        let condition_doc = tsv_ts::build_condition_doc(
            &block.test,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            self.comments,
        );

        // Wrap in group with indent for wrapped lines
        let opening_doc = doc::group(doc::concat(vec![
            doc::text("{#if "),
            doc::indent(condition_doc),
            doc::text("}"),
        ]));

        // Create render config with first_line_offset and suffix_width
        let render_config = tsv_lang::PrintConfig {
            first_line_offset,
            suffix_width,
            ..self.config
        };

        (opening_doc, render_config)
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
            // For binary expressions, use doc-based formatting for proper wrapping
            // For other expressions (method chains, etc.), use imperative formatting
            let is_binary = matches!(
                &else_if.test,
                tsv_ts::Expression::BinaryExpression(b) if b.operator.is_logical()
            );

            if is_binary {
                if !parent_inline {
                    self.write("\n");
                    self.write_indent();
                }
                let (opening_doc, render_config) = self.build_else_if_opening_doc(else_if);
                self.write_doc_with_config(&opening_doc, &render_config);
            } else {
                // Non-binary: use original imperative approach
                self.write_continuation("{:else if ", parent_inline);
                self.print_ts_expression_with_comments(
                    &else_if.test,
                    else_if.opening_tag_span.start,
                    else_if.opening_tag_span.end,
                );
                self.write("}");
            }

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

    /// Build a doc for the {:else if condition} tag with width-aware wrapping.
    ///
    /// Only called for binary expressions (&&, ||).
    fn build_else_if_opening_doc(
        &self,
        else_if: &internal::IfBlock,
    ) -> (doc::Doc, tsv_lang::PrintConfig) {
        let current_col = self.buffer.current_column(self.config.tab_width);
        let first_line_offset = current_col;

        // Build condition doc (ungrouped so our group controls breaking)
        let condition_doc = tsv_ts::build_condition_doc(
            &else_if.test,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            self.comments,
        );

        // Wrap in group with indent for wrapped lines
        let opening_doc = doc::group(doc::concat(vec![
            doc::text("{:else if "),
            doc::indent(condition_doc),
            doc::text("}"),
        ]));

        let render_config = tsv_lang::PrintConfig {
            first_line_offset,
            ..self.config
        };

        (opening_doc, render_config)
    }

    // =========================================================================
    // Each block
    // =========================================================================

    /// Format an each block: {#each expr as item}...{/each}
    pub(super) fn print_each_block(&mut self, block: &internal::EachBlock) {
        // Opening tag
        self.write("{#each ");

        // Calculate suffix width for width-aware expression wrapping
        let suffix_width = self.calculate_each_suffix_width(block);

        // For collection expression, only look for trailing comments up to before "as"
        // to avoid capturing comments that belong to the key expression
        let expr_comment_end = block
            .context
            .as_ref()
            .map_or(block.opening_tag_span.end, |c| c.span().start);
        self.print_ts_expression_with_suffix_width(
            &block.expression,
            block.opening_tag_span.start,
            expr_comment_end,
            suffix_width,
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

        // Calculate suffix width for width-aware expression wrapping
        let suffix_width = self.calculate_await_suffix_width(block);

        self.print_ts_expression_with_suffix_width(
            &block.expression,
            block.opening_tag_span.start,
            block.opening_tag_span.end,
            suffix_width,
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

        // Calculate suffix width for width-aware expression wrapping
        let suffix_width = self.calculate_key_suffix_width(block);

        self.print_ts_expression_with_suffix_width(
            &block.expression,
            block.opening_tag_span.start,
            block.opening_tag_span.end,
            suffix_width,
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
        // Build the opening tag with doc-based parameter wrapping
        let name = block.expression.span().extract(self.source);

        // Type parameters (generics)
        let type_params_part = block.type_parameters.as_ref().map_or_else(
            || doc::text(""),
            |tp| {
                doc::concat(vec![
                    doc::text("<"),
                    doc::text_owned(tp.clone()),
                    doc::text(">"),
                ])
            },
        );

        // Parameters - wrap when line exceeds print width
        // Format each parameter through the TypeScript printer for proper formatting
        let params_docs: Vec<_> = block.raw_parameters.as_ref().map_or_else(
            || {
                block
                    .parameters
                    .iter()
                    .map(|param| {
                        let formatted = tsv_ts::format_expression(
                            param,
                            self.source,
                            Rc::clone(&self.interner),
                        );
                        doc::text_owned(formatted)
                    })
                    .collect()
            },
            |raw| vec![doc::text_owned(raw.clone())],
        );

        // Build params doc: empty or comma-separated list (works for 0, 1, or N params)
        let params_doc = if params_docs.is_empty() {
            doc::text("")
        } else {
            let mut parts = Vec::new();
            for (i, param_doc) in params_docs.into_iter().enumerate() {
                if i > 0 {
                    parts.push(doc::text(","));
                    parts.push(doc::line());
                }
                parts.push(param_doc);
            }
            doc::concat(parts)
        };

        // Build the full opening tag with group for wrapping
        // When fits: {#snippet name(a, b, c)}
        // When wraps: {#snippet name(\n\ta,\n\tb,\n\tc,\n)}
        let opening_doc = doc::group(doc::concat(vec![
            doc::text("{#snippet "),
            doc::text_owned(name.to_string()),
            type_params_part,
            doc::text("("),
            doc::indent(doc::concat(vec![doc::softline(), params_doc])),
            doc::if_break(doc::text(","), doc::text("")),
            doc::softline(),
            doc::text(")}"),
        ]));

        self.write_doc(&opening_doc);

        let is_inline = self.format_block_body(&block.body);
        self.write_closing("{/snippet}", is_inline);
    }
}
