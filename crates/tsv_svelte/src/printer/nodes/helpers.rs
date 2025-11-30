// Helper utilities for node formatting
//
// Provides utilities for expression tag formatting and source position tracking
// used in inline run grouping and multiline formatting decisions.

use crate::ast::internal::FragmentNode;
use crate::printer::Printer;
use tsv_lang::SymbolResolver;
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

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

    /// Format a TypeScript expression (helper for expression tags)
    ///
    /// Delegates to TypeScript printer logic for string literals (quote conversion).
    /// Static HTML attributes always use double quotes per HTML spec.
    fn print_ts_expression(&mut self, expr: &tsv_ts::Expression) {
        match expr {
            tsv_ts::Expression::Literal(lit) => {
                self.print_ts_literal(lit);
            }
            tsv_ts::Expression::Identifier(id) => {
                let name = self.resolve_symbol(id.name);
                self.write(&name);
            }
            tsv_ts::Expression::ObjectExpression(obj) => {
                // TODO: This is a simplified implementation that should be replaced
                // with proper TypeScript printer delegation for complex formatting.
                // For now, handle simple inline objects.
                self.write("{");
                if !obj.properties.is_empty() {
                    self.write(" ");
                    for (i, prop) in obj.properties.iter().enumerate() {
                        match prop {
                            tsv_ts::ObjectProperty::Property(p) => {
                                self.print_ts_expression(&p.key);
                                if !p.shorthand {
                                    self.write(": ");
                                    self.print_ts_expression(&p.value);
                                }
                            }
                            tsv_ts::ObjectProperty::SpreadElement(s) => {
                                self.write("...");
                                self.print_ts_expression(&s.argument);
                            }
                        }
                        if i < obj.properties.len() - 1 {
                            self.write(", ");
                        }
                    }
                    self.write(" ");
                }
                self.write("}");
            }
            tsv_ts::Expression::ArrayExpression(arr) => {
                // Basic array formatting
                self.write("[");
                for (i, elem) in arr.elements.iter().enumerate() {
                    if let Some(e) = elem {
                        self.print_ts_expression(e);
                    }
                    if i < arr.elements.len() - 1 {
                        self.write(", ");
                    }
                }
                self.write("]");
            }
            tsv_ts::Expression::UnaryExpression(unary) => {
                self.write(unary.operator.as_str());
                // Keyword operators need a space before the operand
                if unary.operator.is_keyword_operator() {
                    self.write(" ");
                }
                self.print_ts_expression(&unary.argument);
            }
            tsv_ts::Expression::UpdateExpression(update) => {
                if update.prefix {
                    self.write(update.operator.as_str());
                    self.print_ts_expression(&update.argument);
                } else {
                    self.print_ts_expression(&update.argument);
                    self.write(update.operator.as_str());
                }
            }
            tsv_ts::Expression::BinaryExpression(binary) => {
                self.print_ts_expression(&binary.left);
                self.write(" ");
                self.write(binary.operator.as_str());
                self.write(" ");
                self.print_ts_expression(&binary.right);
            }
            tsv_ts::Expression::ArrowFunctionExpression(arrow) => {
                // Print arrow function: (params) => body
                self.write("(");
                for (i, param) in arrow.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    let name = self.resolve_symbol(param.name);
                    self.write(&name);
                }
                self.write(") => ");
                match &arrow.body {
                    tsv_ts::ArrowFunctionBody::Expression(expr) => {
                        self.print_ts_expression(expr);
                    }
                    tsv_ts::ArrowFunctionBody::BlockStatement { span } => {
                        // Extract raw block from source
                        let raw = span.extract(self.source);
                        self.write(raw);
                    }
                }
            }
            tsv_ts::Expression::SpreadElement(spread) => {
                self.write("...");
                self.print_ts_expression(&spread.argument);
            }
            tsv_ts::Expression::CallExpression(call) => {
                self.print_ts_expression(&call.callee);
                if call.optional {
                    self.write("?.");
                }
                self.write("(");
                for (i, arg) in call.arguments.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_ts_expression(arg);
                }
                self.write(")");
            }
            tsv_ts::Expression::MemberExpression(member) => {
                self.print_ts_expression(&member.object);
                if member.computed {
                    if member.optional {
                        self.write("?.");
                    }
                    self.write("[");
                    self.print_ts_expression(&member.property);
                    self.write("]");
                } else {
                    if member.optional {
                        self.write("?.");
                    } else {
                        self.write(".");
                    }
                    self.print_ts_expression(&member.property);
                }
            }
            tsv_ts::Expression::ConditionalExpression(cond) => {
                self.print_ts_expression(&cond.test);
                self.write(" ? ");
                self.print_ts_expression(&cond.consequent);
                self.write(" : ");
                self.print_ts_expression(&cond.alternate);
            }
            tsv_ts::Expression::TemplateLiteral(template) => {
                self.write("`");
                for (i, quasi) in template.quasis.iter().enumerate() {
                    self.write(&quasi.raw);
                    if i < template.expressions.len() {
                        self.write("${");
                        self.print_ts_expression(&template.expressions[i]);
                        self.write("}");
                    }
                }
                self.write("`");
            }
            tsv_ts::Expression::TaggedTemplateExpression(tagged) => {
                self.print_ts_expression(&tagged.tag);
                self.print_ts_expression(&tsv_ts::Expression::TemplateLiteral(
                    tagged.quasi.clone(),
                ));
            }
            tsv_ts::Expression::NewExpression(new_expr) => {
                self.write("new ");
                self.print_ts_expression(&new_expr.callee);
                self.write("(");
                for (i, arg) in new_expr.arguments.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_ts_expression(arg);
                }
                self.write(")");
            }
            tsv_ts::Expression::FunctionExpression(func) => {
                // Print function expression: (params) { body }
                self.write("(");
                for (i, param) in func.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    let name = self.resolve_symbol(param.name);
                    self.write(&name);
                }
                self.write(") ");
                // Extract body from source (need to own the string to avoid borrow issues)
                let body_start = func.body.span.start as usize;
                let body_end = func.body.span.end as usize;
                let body = self.source()[body_start..body_end].to_string();
                self.write(&body);
            }
            tsv_ts::Expression::AwaitExpression(await_expr) => {
                self.write("await ");
                self.print_ts_expression(&await_expr.argument);
            }
            tsv_ts::Expression::SequenceExpression(seq) => {
                self.write("(");
                for (i, expr) in seq.expressions.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_ts_expression(expr);
                }
                self.write(")");
            }
            tsv_ts::Expression::RegexLiteral(regex) => {
                self.write("/");
                self.write(&regex.pattern);
                self.write("/");
                self.write(&regex.flags);
            }
            tsv_ts::Expression::Super(_) => {
                self.write("super");
            }
        }
    }

    /// Format a TypeScript literal (numbers and strings)
    ///
    /// For strings, applies smart quote selection (singleQuote: true) matching
    /// the TypeScript printer behavior. This ensures consistent quote handling
    /// across expression attributes and template expressions.
    fn print_ts_literal(&mut self, lit: &tsv_ts::Literal) {
        match &lit.value {
            tsv_ts::LiteralValue::Number(n) => {
                // Format the number value
                self.write(&n.to_string());
            }
            tsv_ts::LiteralValue::String { content: _, quote } => {
                // Extract raw literal from source (preserves escape sequences)
                // TypeScript AST was parsed with base_offset, so spans are absolute
                // positions in the full Svelte source.
                let start = lit.span.start as usize;
                let end = lit.span.end as usize;
                let raw_literal = &self.source()[start..end];

                // Extract content without surrounding quotes
                let raw_content = &raw_literal[1..raw_literal.len() - 1];

                // Format using shared utility (handles quote selection and escaping)
                let formatted =
                    format_string_literal(raw_content, *quote, StringFormatOptions::default());

                self.write(&formatted);
            }
            tsv_ts::LiteralValue::Boolean(b) => {
                self.write(if *b { "true" } else { "false" });
            }
            tsv_ts::LiteralValue::Null => {
                self.write("null");
            }
            tsv_ts::LiteralValue::Undefined => {
                self.write("undefined");
            }
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
