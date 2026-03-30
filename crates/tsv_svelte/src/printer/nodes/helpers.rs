// Helper utilities for node formatting
//
// Provides utilities for expression tag formatting and source position tracking
// used in inline run grouping and multiline formatting decisions.

use crate::ast::internal::FragmentNode;
use crate::printer::Printer;

impl<'a> Printer<'a> {
    /// Format an ExpressionTag
    ///
    /// Expression tags are Svelte-specific syntax for embedding TypeScript/JS
    /// expressions in the template: `{expression}`
    pub fn print_expression_tag(&mut self, tag: &crate::ast::internal::ExpressionTag) {
        self.write("{");
        // Assignment expressions need parens in expression tags: {(a = b)}
        let needs_parens = matches!(tag.expression, tsv_ts::Expression::AssignmentExpression(_));
        if needs_parens {
            self.write("(");
        }
        // Format the expression - comments are looked up from Root.comments by span position
        self.print_ts_expression_with_comments(&tag.expression, tag.span.start, tag.span.end);
        if needs_parens {
            self.write(")");
        }
        self.write("}");
    }

    /// Format a TypeScript expression
    ///
    /// Delegates to the TypeScript printer for correct parenthesization and formatting.
    /// This ensures consistency with the TypeScript formatter's rules for:
    /// - Operator precedence (clarifying parentheses)
    /// - Nested ternary wrapping
    /// - IIFE parenthesization
    /// - Mixed logical operator grouping (&&, ||, ??)
    pub fn print_ts_expression(&mut self, expr: &tsv_ts::Expression) {
        let formatted = self.format_ts_expression(expr);
        self.write(&formatted);
    }

    /// Write a JS comment as a leading comment (before content)
    ///
    /// Block comments: `/*content*/ ` (with trailing space)
    /// Line comments: `// content\n` (with newline)
    pub fn write_leading_js_comment(&mut self, comment: &tsv_lang::Comment) {
        if comment.is_block {
            self.write("/*");
            self.write(&comment.content);
            self.write("*/ ");
        } else {
            // Content already includes the space after // (e.g., " comment" from "// comment")
            self.write("//");
            self.write(&comment.content);
            self.write("\n");
        }
    }

    /// Write a JS comment as a trailing comment (after content)
    ///
    /// Block comments: ` /*content*/` (with leading space)
    /// Line comments: ` // content` (with leading space, no newline)
    pub fn write_trailing_js_comment(&mut self, comment: &tsv_lang::Comment) {
        if comment.is_block {
            self.write(" /*");
            self.write(&comment.content);
            self.write("*/");
        } else {
            // Content already includes the space after // (e.g., " comment" from "// comment")
            self.write(" //");
            self.write(&comment.content);
        }
    }

    /// Format a TypeScript expression with leading comments from the given span range.
    ///
    /// This looks up comments from Root.comments that fall within the span range
    /// and prints them before the expression.
    ///
    /// For simple expression contexts (tags, simple blocks), suffix_width defaults to 1
    /// for the closing `}`. For blocks with pattern/body suffixes, use
    /// `print_ts_expression_with_suffix_width` instead.
    pub fn print_ts_expression_with_comments(
        &mut self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
    ) {
        // Default suffix_width of 1 for the closing `}`
        self.print_ts_expression_with_suffix_width(expr, span_start, span_end, 1);
    }

    /// Format a TypeScript expression with explicit suffix width for width-aware wrapping.
    ///
    /// Use this for block expressions where the suffix (pattern, body, closing tag)
    /// should be accounted for in line width calculations.
    pub fn print_ts_expression_with_suffix_width(
        &mut self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
        suffix_width: usize,
    ) {
        // Print any leading comments between the opening brace and the expression
        let expr_start = expr.span().start;
        for comment in tsv_lang::comments_in_range(self.comments, span_start + 1, expr_start) {
            self.write_leading_js_comment(comment);
        }

        // Calculate first_line_offset for width-aware wrapping
        // This tells the TypeScript formatter where the expression starts on the line
        let first_line_offset = self.buffer.current_column(self.config.tab_width);
        // Pass current indent level so wrapped lines get proper indentation
        let base_indent_offset = self.indent_level;
        let config = tsv_lang::PrintConfig {
            first_line_offset,
            suffix_width,
            base_indent_offset,
            is_embedded_expression: true,
            ..Default::default()
        };

        // Format the expression with context-aware width calculations
        let formatted = tsv_ts::format_expression_with_config(
            expr,
            self.source(),
            std::rc::Rc::clone(&self.interner),
            self.comments,
            config,
            &self.line_breaks,
        );
        self.write(&formatted);

        // Print any trailing comments between the expression and closing brace
        let expr_end = expr.span().end;
        for comment in tsv_lang::comments_in_range(self.comments, expr_end, span_end - 1) {
            self.write_trailing_js_comment(comment);
        }
    }

    /// Get the content span for a node, skipping layout whitespace for text nodes
    ///
    /// Used for inline run grouping to determine if nodes are on the same source line.
    /// For text nodes, we skip both leading and trailing whitespace (which is often
    /// indentation and layout separation) to get the actual content position.
    pub fn get_content_span(&self, node: &FragmentNode) -> tsv_lang::Span {
        match node {
            FragmentNode::Text(text) => {
                // Skip leading and trailing whitespace to get the actual content position
                let leading_ws_len = text.raw.len() - text.raw.trim_start().len();
                let trailing_ws_len = text.raw.len() - text.raw.trim_end().len();
                tsv_lang::Span::new(
                    text.span.start + leading_ws_len as u32,
                    text.span.end - trailing_ws_len as u32,
                )
            }
            _ => node.span(),
        }
    }
}

/// Check if a fragment node is a control flow block (if/each/await/key/snippet).
///
/// Control flow blocks can hug adjacent inline content when directly adjacent,
/// unlike HTML block elements (`<div>`, `<p>`) which get their own lines.
pub fn is_control_flow_block(node: &FragmentNode) -> bool {
    matches!(
        node,
        FragmentNode::IfBlock(_)
            | FragmentNode::EachBlock(_)
            | FragmentNode::AwaitBlock(_)
            | FragmentNode::KeyBlock(_)
            | FragmentNode::SnippetBlock(_)
    )
}

/// Check if a fragment node is a control flow block that forces block elements to expand.
///
/// Only if/each/key blocks force expansion. Await blocks do NOT - they stay inline
/// in block elements (e.g., `<div>{#await promise}loading{/await}</div>` stays inline).
pub fn is_expanding_control_flow_block(node: &FragmentNode) -> bool {
    matches!(
        node,
        FragmentNode::IfBlock(_) | FragmentNode::EachBlock(_) | FragmentNode::KeyBlock(_)
    )
}

/// Check if nodes contain any expanding blocks, either directly or nested in await blocks.
///
/// This is a convenience function combining `is_expanding_control_flow_block` and
/// `has_expanding_block_in_await` checks that are commonly used together.
pub fn has_any_expanding_blocks(nodes: &[FragmentNode]) -> bool {
    nodes.iter().any(is_expanding_control_flow_block) || has_expanding_block_in_await(nodes)
}

/// Check if any await block contains expanding blocks (if/each/key) in its content.
///
/// Prettier treats expanding blocks inside await blocks as if they were directly
/// in the parent element, forcing multiline. For example:
/// `<a>{#await p}{#if c}text{/if}{/await}</a>` breaks because the if block
/// is effectively inside the inline element.
///
/// This function recursively checks nested await blocks, so deeply nested
/// structures like `{#await p1}{#await p2}{#if c}...{/if}{/await}{/await}`
/// are also detected.
fn has_expanding_block_in_await(nodes: &[FragmentNode]) -> bool {
    nodes.iter().any(|n| {
        if let FragmentNode::AwaitBlock(block) = n {
            // Check all branches of the await block for expanding blocks
            // or recursively for nested awaits containing expanding blocks
            let check_fragment = |f: &crate::ast::internal::Fragment| {
                f.nodes.iter().any(is_expanding_control_flow_block)
                    || has_expanding_block_in_await(&f.nodes)
            };
            let has_in_pending = block.pending.as_ref().is_some_and(check_fragment);
            let has_in_then = block.then.as_ref().is_some_and(check_fragment);
            let has_in_catch = block.catch.as_ref().is_some_and(check_fragment);
            has_in_pending || has_in_then || has_in_catch
        } else {
            false
        }
    })
}

/// Check if any child element contains block flow (if/each/etc).
///
/// Used to detect when a parent element will go multiline due to
/// nested content forcing line breaks.
pub fn has_nested_block_flow(nodes: &[FragmentNode]) -> bool {
    nodes.iter().any(|n| {
        if let FragmentNode::Element(child) = n {
            child.fragment.nodes.iter().any(is_control_flow_block)
        } else {
            false
        }
    })
}
