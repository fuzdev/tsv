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
        // Format the expression directly
        self.print_ts_expression(&tag.expression);
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
