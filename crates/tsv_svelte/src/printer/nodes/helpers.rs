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
        // Format the expression - comments are looked up from Root.comments by span position
        self.print_ts_expression_with_comments(&tag.expression, tag.span.start, tag.span.end);
        self.write("}");
    }

    /// Format a TypeScript pattern (destructuring context)
    ///
    /// Patterns use spaces inside braces: `{ a, b }` instead of `{a, b}`.
    /// Used for `{#each ... as pattern}` contexts.
    pub fn print_ts_pattern(&mut self, expr: &tsv_ts::Expression) {
        match expr {
            // ObjectExpression may appear in legacy AST or from external sources
            tsv_ts::Expression::ObjectExpression(obj) => {
                // Destructuring patterns use spaces inside braces (matches prettier)
                self.write("{ ");
                for (i, prop) in obj.properties.iter().enumerate() {
                    match prop {
                        tsv_ts::ObjectProperty::Property(p) => {
                            self.print_ts_expression(&p.key);
                            if !p.shorthand {
                                self.write(": ");
                                self.print_ts_pattern(&p.value);
                            }
                        }
                        tsv_ts::ObjectProperty::SpreadElement(s) => {
                            self.write("...");
                            self.print_ts_pattern(&s.argument);
                        }
                    }
                    if i < obj.properties.len() - 1 {
                        self.write(", ");
                    }
                }
                self.write(" }");
            }
            tsv_ts::Expression::ObjectPattern(obj) => {
                // ObjectPattern - correct AST type for destructuring patterns
                self.write("{ ");
                for (i, prop) in obj.properties.iter().enumerate() {
                    match prop {
                        tsv_ts::ObjectPatternProperty::Property(p) => {
                            self.print_ts_expression(&p.key);
                            if !p.shorthand {
                                self.write(": ");
                                self.print_ts_pattern(&p.value);
                            }
                        }
                        tsv_ts::ObjectPatternProperty::RestElement(r) => {
                            self.write("...");
                            self.print_ts_pattern(&r.argument);
                        }
                    }
                    if i < obj.properties.len() - 1 {
                        self.write(", ");
                    }
                }
                self.write(" }");
            }
            tsv_ts::Expression::ArrayExpression(arr) => {
                // Array patterns also use spaces inside brackets
                self.write("[");
                for (i, elem) in arr.elements.iter().enumerate() {
                    if let Some(e) = elem {
                        self.print_ts_pattern(e);
                    }
                    if i < arr.elements.len() - 1 {
                        self.write(", ");
                    }
                }
                self.write("]");
            }
            tsv_ts::Expression::ArrayPattern(arr) => {
                // ArrayPattern - correct AST type for array destructuring
                self.write("[");
                for (i, elem) in arr.elements.iter().enumerate() {
                    if let Some(e) = elem {
                        self.print_ts_pattern(e);
                    }
                    if i < arr.elements.len() - 1 {
                        self.write(", ");
                    }
                }
                self.write("]");
            }
            // For other expression types, delegate to regular expression printing
            _ => self.print_ts_expression(expr),
        }
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
        let formatted =
            tsv_ts::format_expression(expr, self.source(), std::rc::Rc::clone(&self.interner));
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
            self.write("// ");
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
            self.write(" // ");
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
            ..Default::default()
        };

        // Format the expression with context-aware width calculations
        let formatted = tsv_ts::format_expression_with_config(
            expr,
            self.source(),
            std::rc::Rc::clone(&self.interner),
            self.comments,
            config,
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
