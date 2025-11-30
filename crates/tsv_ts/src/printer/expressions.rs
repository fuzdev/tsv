// Expression printing for TypeScript
//
// Dispatches expression printing to specialized modules:
// - objects.rs: Object expressions and property handling
// - arrays.rs: Array expressions
// - operators.rs: Unary and binary expressions
// - calls.rs: Call, member, and conditional expressions
//
// This module handles:
// - Expression dispatch (print_expression, build_expression_doc)
// - Literals and identifiers
// - Arrow functions and spread elements

use super::Printer;
use crate::ast::internal::{self, Expression, LiteralValue};
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

impl<'a> Printer<'a> {
    /// Print an expression
    pub(crate) fn print_expression(&mut self, expression: &Expression) {
        // TODO: Add comment support for additional expression types as they're implemented
        // Future expressions that will need comment handling:
        //   - ArrayExpression: Comments before/after elements, trailing comma
        //   - CallExpression: Comments around arguments, after callee
        //   - MemberExpression: Comments around dot/bracket notation
        //   - BinaryExpression: Comments around operators (+, -, *, etc.)
        //   - ConditionalExpression: Comments around ?, : in ternary
        //   - ArrowFunctionExpression: Comments in parameter lists, around =>
        //   - TemplateLiteral: Comments in template expressions ${}
        // Reference: Current object expression implementation as pattern
        match expression {
            Expression::Literal(lit) => self.print_literal(lit),
            Expression::Identifier(id) => self.print_identifier(id),
            Expression::ObjectExpression(obj) => self.print_object_expression(obj),
            Expression::ArrayExpression(arr) => self.print_array_expression(arr),
            Expression::UnaryExpression(unary) => self.print_unary_expression(unary),
            Expression::UpdateExpression(update) => self.print_update_expression(update),
            Expression::BinaryExpression(binary) => self.print_binary_expression(binary),
            Expression::CallExpression(call) => self.print_call_expression(call),
            Expression::NewExpression(new_expr) => self.print_new_expression(new_expr),
            Expression::MemberExpression(member) => self.print_member_expression(member),
            Expression::ConditionalExpression(cond) => self.print_conditional_expression(cond),
            Expression::ArrowFunctionExpression(arrow) => self.print_arrow_function(arrow),
            Expression::FunctionExpression(func) => self.print_function_expression(func),
            Expression::SpreadElement(spread) => self.print_spread_element(spread),
            Expression::TemplateLiteral(template) => self.print_template_literal(template),
            Expression::TaggedTemplateExpression(tagged) => {
                self.print_tagged_template_expression(tagged);
            }
            Expression::AwaitExpression(await_expr) => self.print_await_expression(await_expr),
            Expression::SequenceExpression(seq) => self.print_sequence_expression(seq),
            Expression::RegexLiteral(regex) => self.print_regex_literal(regex),
            Expression::Super(_) => self.write("super"),
        }
    }

    /// Print a literal value
    fn print_literal(&mut self, literal: &internal::Literal) {
        match &literal.value {
            LiteralValue::Number(n) => {
                // Format the number value
                // Note: This normalizes the format (hex/binary become decimal)
                // Future: preserve original format if needed
                self.write(&n.to_string());
            }
            LiteralValue::String { content: _, quote } => {
                // Extract raw literal from source (preserves escape sequences)
                let start = literal.span.start as usize;
                let end = literal.span.end as usize;
                let raw_literal = &self.source[start..end];

                // Extract content without surrounding quotes
                let raw_content = &raw_literal[1..raw_literal.len() - 1];

                // Format using shared utility (handles quote selection and escaping)
                let formatted =
                    format_string_literal(raw_content, *quote, StringFormatOptions::default());

                self.write(&formatted);
            }
            LiteralValue::Boolean(b) => {
                self.write(if *b { "true" } else { "false" });
            }
            LiteralValue::Null => {
                self.write("null");
            }
            LiteralValue::Undefined => {
                self.write("undefined");
            }
        }
    }

    /// Print an identifier
    pub(super) fn print_identifier(&mut self, identifier: &internal::Identifier) {
        // Resolve symbol from interner using centralized helper
        let name = self.resolve_symbol(identifier.name);
        self.write(&name);

        // Handle type annotations
        if let Some(type_annotation) = &identifier.type_annotation {
            self.print_type_annotation(type_annotation);
        }
    }

    /// Build a Doc for an expression (for use in object/array contexts and statements)
    pub(super) fn build_expression_doc(&self, expr: &Expression) -> Doc {
        match expr {
            Expression::Literal(lit) => self.build_literal_doc(lit),
            Expression::Identifier(id) => {
                let name = self.resolve_symbol(id.name);
                doc::text(name)
            }
            Expression::ObjectExpression(obj) => self.build_object_doc(obj),
            Expression::ArrayExpression(arr) => self.build_array_doc(arr),
            Expression::UnaryExpression(unary) => self.build_unary_doc(unary),
            Expression::UpdateExpression(update) => self.build_update_doc(update),
            Expression::BinaryExpression(binary) => self.build_binary_doc(binary),
            Expression::CallExpression(call) => self.build_call_doc(call),
            Expression::NewExpression(new_expr) => self.build_new_doc(new_expr),
            Expression::MemberExpression(member) => self.build_member_doc(member),
            Expression::ConditionalExpression(cond) => {
                self.build_conditional_doc_with_wrapping(cond)
            }
            Expression::ArrowFunctionExpression(arrow) => self.build_arrow_doc(arrow),
            Expression::FunctionExpression(func) => self.build_function_doc(func),
            Expression::SpreadElement(spread) => self.build_spread_doc(spread),
            Expression::TemplateLiteral(template) => self.build_template_literal_doc(template),
            Expression::TaggedTemplateExpression(tagged) => self.build_tagged_template_doc(tagged),
            Expression::AwaitExpression(await_expr) => self.build_await_doc(await_expr),
            Expression::SequenceExpression(seq) => self.build_sequence_doc(seq),
            Expression::RegexLiteral(regex) => self.build_regex_doc(regex),
            Expression::Super(_) => doc::text("super"),
        }
    }

    /// Build a Doc for a literal
    fn build_literal_doc(&self, literal: &internal::Literal) -> Doc {
        match &literal.value {
            LiteralValue::Number(n) => doc::text(n.to_string()),
            LiteralValue::String { content: _, quote } => {
                let start = literal.span.start as usize;
                let end = literal.span.end as usize;
                let raw_literal = &self.source[start..end];
                let raw_content = &raw_literal[1..raw_literal.len() - 1];
                let formatted =
                    format_string_literal(raw_content, *quote, StringFormatOptions::default());
                doc::text(formatted)
            }
            LiteralValue::Boolean(b) => doc::text(if *b { "true" } else { "false" }),
            LiteralValue::Null => doc::text("null"),
            LiteralValue::Undefined => doc::text("undefined"),
        }
    }

    /// Print a regex literal: /pattern/flags
    fn print_regex_literal(&mut self, regex: &internal::RegexLiteral) {
        self.write("/");
        self.write(&regex.pattern);
        self.write("/");
        self.write(&regex.flags);
    }

    /// Build a Doc for a regex literal
    fn build_regex_doc(&self, regex: &internal::RegexLiteral) -> Doc {
        doc::text(format!("/{}/{}", regex.pattern, regex.flags))
    }

    // =========================================================================
    // Arrow Function & Spread
    // =========================================================================

    /// Print an arrow function expression
    fn print_arrow_function(&mut self, arrow: &internal::ArrowFunctionExpression) {
        // Print parameters
        self.write("(");
        for (i, param) in arrow.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            let name = self.resolve_symbol(param.name);
            self.write(&name);
        }
        self.write(") => ");

        // Print body
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                self.print_expression(expr);
            }
            internal::ArrowFunctionBody::BlockStatement { span } => {
                // Extract raw block from source
                let raw = span.extract(self.source);
                self.write(raw);
            }
        }
    }

    /// Print a spread element
    fn print_spread_element(&mut self, spread: &internal::SpreadElement) {
        self.write("...");
        self.print_expression(&spread.argument);
    }

    /// Build a Doc for an arrow function
    pub(super) fn build_arrow_doc(&self, arrow: &internal::ArrowFunctionExpression) -> Doc {
        let mut parts = Vec::new();

        // Parameters
        parts.push(doc::text("("));
        for (i, param) in arrow.params.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            let name = self.resolve_symbol(param.name);
            parts.push(doc::text(name));
        }
        parts.push(doc::text(") => "));

        // Body
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                parts.push(self.build_expression_doc(expr));
            }
            internal::ArrowFunctionBody::BlockStatement { span } => {
                // Extract raw block from source
                let raw = span.extract(self.source);
                parts.push(doc::text(raw));
            }
        }

        doc::concat(parts)
    }

    /// Build a Doc for a spread element
    pub(super) fn build_spread_doc(&self, spread: &internal::SpreadElement) -> Doc {
        doc::concat(vec![
            doc::text("..."),
            self.build_expression_doc(&spread.argument),
        ])
    }

    // =========================================================================
    // Template Literals
    // =========================================================================

    /// Print a template literal: `hello ${name}`
    pub(super) fn print_template_literal(&mut self, template: &internal::TemplateLiteral) {
        self.write("`");

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Print the raw template content (preserving escapes)
            self.write(&quasi.raw);

            // Print interpolation if there's a corresponding expression
            if i < template.expressions.len() {
                self.write("${");
                self.print_expression(&template.expressions[i]);
                self.write("}");
            }
        }

        self.write("`");
    }

    /// Print a tagged template expression: tag`content ${expr}`
    fn print_tagged_template_expression(&mut self, tagged: &internal::TaggedTemplateExpression) {
        self.print_expression(&tagged.tag);
        self.print_template_literal(&tagged.quasi);
    }

    /// Build a Doc for a template literal
    fn build_template_literal_doc(&self, template: &internal::TemplateLiteral) -> Doc {
        let mut parts = Vec::new();
        parts.push(doc::text("`"));

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Template content (raw, preserving escapes)
            parts.push(doc::text(quasi.raw.clone()));

            // Interpolation
            if i < template.expressions.len() {
                parts.push(doc::text("${"));
                parts.push(self.build_expression_doc(&template.expressions[i]));
                parts.push(doc::text("}"));
            }
        }

        parts.push(doc::text("`"));
        doc::concat(parts)
    }

    /// Build a Doc for a tagged template expression
    fn build_tagged_template_doc(&self, tagged: &internal::TaggedTemplateExpression) -> Doc {
        doc::concat(vec![
            self.build_expression_doc(&tagged.tag),
            self.build_template_literal_doc(&tagged.quasi),
        ])
    }

    // =========================================================================
    // Function Expressions
    // =========================================================================

    /// Print a function expression (for method shorthand): `foo() { return 1; }`
    pub(super) fn print_function_expression(&mut self, func: &internal::FunctionExpression) {
        // Print parameters
        self.write("(");
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            let name = self.resolve_symbol(param.name);
            self.write(&name);
        }
        self.write(") ");

        // Print body
        self.print_block_statement(&func.body);
    }

    /// Build a Doc for a function expression
    pub(super) fn build_function_doc(&self, func: &internal::FunctionExpression) -> Doc {
        let mut parts = Vec::new();

        // Parameters
        parts.push(doc::text("("));
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            let name = self.resolve_symbol(param.name);
            parts.push(doc::text(name));
        }
        parts.push(doc::text(") "));

        // Body
        parts.push(self.build_block_statement_doc(&func.body));

        doc::concat(parts)
    }

    /// Print a block statement: `{ stmt1; stmt2; }`
    pub(super) fn print_block_statement(&mut self, block: &internal::BlockStatement) {
        if block.body.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{\n");
        self.indent_level += 1;

        for stmt in &block.body {
            self.write_indent();
            self.print_statement(stmt);
            self.write("\n");
        }

        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
    }

    /// Build a Doc for a block statement
    pub(super) fn build_block_statement_doc(&self, block: &internal::BlockStatement) -> Doc {
        if block.body.is_empty() {
            return doc::text("{}");
        }

        // Build statements with line breaks between them
        let mut body_parts = Vec::new();
        for (i, stmt) in block.body.iter().enumerate() {
            if i > 0 {
                body_parts.push(doc::hardline());
            }
            body_parts.push(self.build_statement_doc(stmt));
        }

        // Structure: `{` + indent(hardline + statements) + hardline + `}`
        doc::concat(vec![
            doc::text("{"),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(body_parts)])),
            doc::hardline(),
            doc::text("}"),
        ])
    }
}
