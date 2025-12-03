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

/// Normalize a number literal to match Prettier's output format.
///
/// Transformations:
/// - Hex to lowercase: `0xFF` → `0xff`
/// - Scientific notation to lowercase without `+`: `2E+10` → `2e10`
/// - Leading decimal gets zero: `.5` → `0.5`
/// - Trailing decimal removed: `5.` → `5`
/// - BigInt hex to lowercase: `0xFFn` → `0xffn`
/// - Numeric separators preserved
pub fn normalize_number_literal(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let chars: Vec<char> = raw.chars().collect();
    let len = chars.len();

    // Handle leading decimal: .5 → 0.5
    if chars.first() == Some(&'.') {
        result.push('0');
        result.push_str(raw);
        return result;
    }

    // Check for BigInt suffix
    let is_bigint = chars.last() == Some(&'n');
    let num_end = if is_bigint { len - 1 } else { len };

    // Check for trailing decimal: 5. → 5
    if num_end > 0 && chars[num_end - 1] == '.' {
        // Copy everything except the trailing decimal
        for &c in &chars[..num_end - 1] {
            result.push(c.to_ascii_lowercase());
        }
        if is_bigint {
            result.push('n');
        }
        return result;
    }

    // Process the number, lowercasing hex digits and 'e'/'E', removing '+' after 'e'
    let mut i = 0;
    while i < num_end {
        let c = chars[i];
        if c == 'E' {
            result.push('e');
            // Skip '+' after e/E if present
            if i + 1 < num_end && chars[i + 1] == '+' {
                i += 1;
            }
        } else {
            result.push(c.to_ascii_lowercase());
        }
        i += 1;
    }

    if is_bigint {
        result.push('n');
    }

    result
}

impl<'a> Printer<'a> {
    /// Print an expression
    pub fn print_expression(&mut self, expression: &Expression) {
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
            Expression::AssignmentExpression(assign) => self.print_assignment_expression(assign),
            Expression::ObjectPattern(obj) => self.print_object_pattern(obj),
            Expression::ArrayPattern(arr) => self.print_array_pattern(arr),
            Expression::AssignmentPattern(pattern) => self.print_assignment_pattern(pattern),
            Expression::RestElement(rest) => self.print_rest_element(rest),
        }
    }

    /// Print a literal value
    pub(super) fn print_literal(&mut self, literal: &internal::Literal) {
        match &literal.value {
            LiteralValue::Number(_) => {
                // Extract raw literal and normalize it
                let raw = literal.span.extract(self.source);
                let normalized = normalize_number_literal(raw);
                self.write(&normalized);
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

        // Handle optional marker (e.g., `a?` in `function fn(a?: number) {}`)
        if identifier.optional {
            self.write("?");
        }

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
                let name_doc = doc::text(name);

                // Fast path: no optional marker or type annotation
                if !id.optional && id.type_annotation.is_none() {
                    name_doc
                } else {
                    let mut parts = vec![name_doc];

                    // Handle optional marker (e.g., `a?` in `function fn(a?: number) {}`)
                    if id.optional {
                        parts.push(doc::text("?"));
                    }

                    // Handle type annotations
                    if let Some(type_annotation) = &id.type_annotation {
                        parts.push(self.build_type_annotation_doc(type_annotation));
                    }

                    doc::concat(parts)
                }
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
            Expression::AssignmentExpression(assign) => self.build_assignment_doc(assign),
            Expression::ObjectPattern(obj) => self.build_object_pattern_doc(obj),
            Expression::ArrayPattern(arr) => self.build_array_pattern_doc(arr),
            Expression::AssignmentPattern(pattern) => self.build_assignment_pattern_doc(pattern),
            Expression::RestElement(rest) => self.build_rest_element_doc(rest),
        }
    }

    /// Build a Doc for a literal
    pub(super) fn build_literal_doc(&self, literal: &internal::Literal) -> Doc {
        match &literal.value {
            LiteralValue::Number(_) => {
                // Extract raw literal and normalize it
                let raw = literal.span.extract(self.source);
                doc::text(normalize_number_literal(raw))
            }
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
        // Print async keyword if present
        if arrow.r#async {
            self.write("async ");
        }

        // Print parameters (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        self.write("(");
        for (i, param) in arrow.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_expression(param);
        }
        self.write(")");

        // Print return type annotation if present
        if let Some(return_type) = &arrow.return_type {
            self.print_type_annotation(return_type);
        }

        self.write(" => ");

        // Print body
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                self.print_expression(expr);
            }
            internal::ArrowFunctionBody::BlockStatement(block) => {
                self.print_block_statement(block);
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

        // Async keyword if present
        if arrow.r#async {
            parts.push(doc::text("async "));
        }

        // Parameters (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        parts.push(doc::text("("));
        for (i, param) in arrow.params.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_expression_doc(param));
        }
        parts.push(doc::text(")"));

        // Return type annotation
        if let Some(return_type) = &arrow.return_type {
            parts.push(self.build_type_annotation_doc(return_type));
        }

        parts.push(doc::text(" => "));

        // Body
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                parts.push(self.build_expression_doc(expr));
            }
            internal::ArrowFunctionBody::BlockStatement(block) => {
                parts.push(self.build_block_statement_doc(block));
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
        // Print parameters (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        self.write("(");
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_expression(param);
        }
        self.write(") ");

        // Print body
        self.print_block_statement(&func.body);
    }

    /// Build a Doc for a function expression
    pub(super) fn build_function_doc(&self, func: &internal::FunctionExpression) -> Doc {
        let mut parts = Vec::new();

        // Parameters (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        parts.push(doc::text("("));
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_expression_doc(param));
        }
        parts.push(doc::text(")"));

        // Return type annotation (e.g., `: number`)
        if let Some(return_type) = &func.return_type {
            parts.push(self.build_type_annotation_doc(return_type));
        }

        parts.push(doc::text(" "));

        // Body
        parts.push(self.build_block_statement_doc(&func.body));

        doc::concat(parts)
    }

    /// Print a block statement: `{ stmt1; stmt2; }`
    ///
    /// Handles comments between statements similar to print_program.
    pub(super) fn print_block_statement(&mut self, block: &internal::BlockStatement) {
        if block.body.is_empty() {
            // Check for comments inside empty block
            // Block span starts after '{' and ends before '}'
            let block_start = block.span.start + 1; // After '{'
            let block_end = block.span.end - 1; // Before '}'
            let has_inner_comments = self.has_comments_between(block_start, block_end);

            if has_inner_comments {
                self.write("{\n");
                self.indent_level += 1;
                self.print_leading_comments(block_start, block_end, false);
                self.indent_level -= 1;
                self.write_indent();
                self.write("}");
            } else {
                self.write("{}");
            }
            return;
        }

        self.write("{\n");
        self.indent_level += 1;

        // Track previous statement end for comment printing (similar to print_program)
        let mut prev_end = block.span.start + 1; // Start after '{'

        for (i, stmt) in block.body.iter().enumerate() {
            let is_first = i == 0;

            // Check for blank lines between statements (when no comments)
            if !is_first {
                let has_comments = self
                    .comments
                    .iter()
                    .any(|c| c.span.start >= prev_end && c.span.end <= stmt.span().start);

                if !has_comments
                    && tsv_lang::printing::has_blank_line_between(
                        self.source,
                        prev_end,
                        stmt.span().start,
                    )
                {
                    self.write("\n");
                }
            }

            // Print leading comments before this statement
            // For the first statement, include same-line comments after '{'
            self.print_block_leading_comments(prev_end, stmt.span().start, is_first);

            self.write_indent();
            self.print_statement(stmt);
            self.write("\n");

            prev_end = stmt.span().end;
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

    // =========================================================================
    // Assignment and Pattern Expressions
    // =========================================================================

    /// Print an assignment expression with width-based wrapping
    ///
    /// When the assignment exceeds print_width, wraps after `=`:
    /// ```javascript
    /// [long, array, pattern] =
    ///     value;
    /// ```
    ///
    /// However, when the LHS pattern is already expanded (multiline), don't wrap:
    /// ```javascript
    /// ({
    ///     a: {b},
    /// } = obj);  // stays together
    /// ```
    fn print_assignment_expression(&mut self, assign: &internal::AssignmentExpression) {
        // Object patterns: let the pattern handle its own width-based expansion
        // The `} = rhs` should stay together on the closing line
        if matches!(assign.left.as_ref(), Expression::ObjectPattern(_)) {
            self.print_expression(&assign.left);
            self.write(" ");
            self.write(assign.operator.as_str());
            self.write(" ");
            self.print_expression(&assign.right);
            return;
        }

        // Array patterns and regular assignments: use doc-builder for width-based wrapping
        // When the line exceeds print width, wrap after `=`
        let assignment_doc = self.build_assignment_doc(assign);
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset;
        let output = doc::print_doc_at_column(&assignment_doc, &self.config, current_col);
        self.write(&output);
    }

    /// Build a Doc for an assignment expression
    fn build_assignment_doc(&self, assign: &internal::AssignmentExpression) -> Doc {
        let left_doc = self.build_expression_doc(&assign.left);
        let right_doc = self.build_expression_doc(&assign.right);

        // Structure: group(left + " =" + indent(line + right))
        doc::group(doc::concat(vec![
            left_doc,
            doc::text(" "),
            doc::text(assign.operator.as_str()),
            doc::indent(doc::concat(vec![doc::line(), right_doc])),
        ]))
    }

    /// Print an object pattern: `{a, b}` or expanded `{\n\ta,\n\tb,\n}`
    ///
    /// Prettier expands object patterns when:
    /// 1. Any property has a nested pattern value (always expand)
    /// 2. The pattern exceeds print width (width-based expansion)
    fn print_object_pattern(&mut self, obj: &internal::ObjectPattern) {
        if obj.properties.is_empty() {
            self.write("{}");
            return;
        }

        let should_expand = super::object_pattern_should_expand(obj);

        if should_expand {
            // Nested patterns: always expand (imperative path for performance)
            let last_is_rest = matches!(
                obj.properties.last(),
                Some(internal::ObjectPatternProperty::RestElement(_))
            );

            self.write("{\n");
            self.indent_level += 1;
            for (i, prop) in obj.properties.iter().enumerate() {
                self.write_indent();
                self.print_object_pattern_property(prop);
                // Add trailing comma unless it's a rest element (syntax error)
                let is_last = i == obj.properties.len() - 1;
                if !is_last || !last_is_rest {
                    self.write(",");
                }
                if !is_last {
                    self.write("\n");
                }
            }
            self.write("\n");
            self.indent_level -= 1;
            self.write_indent();
            self.write("}");
        } else {
            // Use doc-builder for width-based expansion
            let doc = self.build_object_pattern_doc(obj);
            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset;
            let output = doc::print_doc_at_column(&doc, &self.config, current_col);
            self.write(&output);
        }
    }

    /// Print an object pattern property
    fn print_object_pattern_property(&mut self, prop: &internal::ObjectPatternProperty) {
        match prop {
            internal::ObjectPatternProperty::Property(p) => {
                if p.shorthand {
                    // For shorthand with default value, we need to print key = default
                    // Handle both AssignmentPattern and AssignmentExpression
                    // (parser may produce AssignmentExpression in some contexts)
                    if let Expression::AssignmentPattern(pattern) = &p.value {
                        self.print_expression(&p.key);
                        self.write(" = ");
                        self.print_expression(&pattern.right);
                    } else if let Expression::AssignmentExpression(assign) = &p.value {
                        self.print_expression(&p.key);
                        self.write(" = ");
                        self.print_expression(&assign.right);
                    } else {
                        self.print_expression(&p.key);
                    }
                } else {
                    // Handle computed keys: {[key]: value}
                    // For regular keys, use print_property_key to normalize string keys to identifiers
                    if p.computed {
                        self.write("[");
                        self.print_expression(&p.key);
                        self.write("]");
                    } else {
                        self.print_property_key(&p.key);
                    }
                    self.write(": ");
                    self.print_expression(&p.value);
                }
            }
            internal::ObjectPatternProperty::RestElement(r) => {
                self.print_rest_element(r);
            }
        }
    }

    /// Build a Doc for an object pattern
    ///
    /// Prettier expands object patterns when:
    /// 1. Any property has a nested pattern value (always expand)
    /// 2. The pattern exceeds print width (width-based expansion)
    fn build_object_pattern_doc(&self, obj: &internal::ObjectPattern) -> Doc {
        if obj.properties.is_empty() {
            return doc::text("{}");
        }

        let should_expand = super::object_pattern_should_expand(obj);

        if should_expand {
            // Nested patterns: always expand
            let last_is_rest = matches!(
                obj.properties.last(),
                Some(internal::ObjectPatternProperty::RestElement(_))
            );

            let mut prop_parts = Vec::new();
            for (i, prop) in obj.properties.iter().enumerate() {
                prop_parts.push(self.build_object_pattern_property_doc(prop));
                let is_last = i == obj.properties.len() - 1;
                // Add trailing comma unless it's a rest element (syntax error)
                if !is_last || !last_is_rest {
                    prop_parts.push(doc::text(","));
                }
                if !is_last {
                    prop_parts.push(doc::hardline());
                }
            }

            // Structure: { + indent(hardline + props) + hardline + }
            // The hardline INSIDE indent gets indented, the one before } does not
            doc::concat(vec![
                doc::text("{"),
                doc::indent(doc::concat(vec![doc::hardline(), doc::concat(prop_parts)])),
                doc::hardline(),
                doc::text("}"),
            ])
        } else {
            // Use group with line breaks for width-based expansion
            let mut parts = Vec::new();
            let last_is_rest = matches!(
                obj.properties.last(),
                Some(internal::ObjectPatternProperty::RestElement(_))
            );

            for (i, prop) in obj.properties.iter().enumerate() {
                parts.push(self.build_object_pattern_property_doc(prop));
                if i < obj.properties.len() - 1 {
                    parts.push(doc::text(","));
                    parts.push(doc::line());
                } else if !last_is_rest {
                    // Last property: trailing comma only when broken
                    // (rest elements can't have trailing commas)
                    parts.push(doc::if_break(doc::text(","), doc::text("")));
                }
            }

            doc::group(doc::concat(vec![
                doc::text("{"),
                doc::indent(doc::concat(vec![doc::softline(), doc::concat(parts)])),
                doc::softline(),
                doc::text("}"),
            ]))
        }
    }

    /// Build a Doc for an object pattern property
    ///
    /// String keys that are valid identifiers are normalized to unquoted form:
    /// `{"key": value}` → `{key: value}`
    fn build_object_pattern_property_doc(&self, prop: &internal::ObjectPatternProperty) -> Doc {
        match prop {
            internal::ObjectPatternProperty::Property(p) => {
                if p.shorthand {
                    // Handle both AssignmentPattern and AssignmentExpression
                    // (parser may produce AssignmentExpression in some contexts)
                    if let Expression::AssignmentPattern(pattern) = &p.value {
                        doc::concat(vec![
                            self.build_expression_doc(&p.key),
                            doc::text(" = "),
                            self.build_expression_doc(&pattern.right),
                        ])
                    } else if let Expression::AssignmentExpression(assign) = &p.value {
                        doc::concat(vec![
                            self.build_expression_doc(&p.key),
                            doc::text(" = "),
                            self.build_expression_doc(&assign.right),
                        ])
                    } else {
                        self.build_expression_doc(&p.key)
                    }
                } else {
                    // Handle computed keys: {[key]: value}
                    // For regular keys, use property_key_doc to normalize string keys to identifiers
                    let key_doc = if p.computed {
                        doc::concat(vec![
                            doc::text("["),
                            self.build_expression_doc(&p.key),
                            doc::text("]"),
                        ])
                    } else {
                        self.build_property_key_doc(&p.key)
                    };
                    doc::concat(vec![
                        key_doc,
                        doc::text(": "),
                        self.build_expression_doc(&p.value),
                    ])
                }
            }
            internal::ObjectPatternProperty::RestElement(r) => self.build_rest_element_doc(r),
        }
    }

    /// Print an array pattern: `[a, b]`
    fn print_array_pattern(&mut self, arr: &internal::ArrayPattern) {
        self.write("[");
        for (i, elem) in arr.elements.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            if let Some(e) = elem {
                self.print_expression(e);
            }
        }
        self.write("]");
    }

    /// Build a Doc for an array pattern
    fn build_array_pattern_doc(&self, arr: &internal::ArrayPattern) -> Doc {
        let mut parts = Vec::new();
        parts.push(doc::text("["));
        for (i, elem) in arr.elements.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            if let Some(e) = elem {
                parts.push(self.build_expression_doc(e));
            }
        }
        parts.push(doc::text("]"));
        doc::concat(parts)
    }

    /// Print an assignment pattern: `a = 1`
    fn print_assignment_pattern(&mut self, pattern: &internal::AssignmentPattern) {
        self.print_expression(&pattern.left);
        self.write(" = ");
        self.print_expression(&pattern.right);
    }

    /// Build a Doc for an assignment pattern
    fn build_assignment_pattern_doc(&self, pattern: &internal::AssignmentPattern) -> Doc {
        doc::concat(vec![
            self.build_expression_doc(&pattern.left),
            doc::text(" = "),
            self.build_expression_doc(&pattern.right),
        ])
    }

    /// Print a rest element: `...rest`
    fn print_rest_element(&mut self, rest: &internal::RestElement) {
        self.write("...");
        self.print_expression(&rest.argument);
    }

    /// Build a Doc for a rest element
    fn build_rest_element_doc(&self, rest: &internal::RestElement) -> Doc {
        doc::concat(vec![
            doc::text("..."),
            self.build_expression_doc(&rest.argument),
        ])
    }
}
