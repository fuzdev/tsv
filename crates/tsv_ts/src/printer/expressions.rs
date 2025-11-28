// Expression printing for TypeScript
//
// Handles printing of different expression types:
// - Literals (strings, numbers, booleans, etc.)
// - Identifiers (variable names, function names, etc.)
// - Future: Binary operations, function calls, member access, etc.

use super::Printer;
use crate::ast::internal::{self, BinaryOperator, Expression, Literal, LiteralValue};
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::{
    StringFormatOptions, format_string_literal, has_blank_line_between, has_newline_between,
    is_same_line,
};

/// Check if a chain expression contains any call expressions
fn chain_has_calls(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::CallExpression(_) => true,
        internal::Expression::MemberExpression(member) => chain_has_calls(&member.object),
        _ => false,
    }
}

/// Check if child binary expression needs parens for clarity
///
/// Based on prettier's parenthesization logic from:
/// ~/dev/prettier/src/language-js/needs-parens.js (lines 403-449)
///
/// Returns true when:
/// 1. Mixing different logical operators (&&, ||, ??) regardless of precedence
/// 2. Same precedence but can't flatten (e.g., chained equality operators)
fn needs_parens_for_clarity(child: &internal::BinaryExpression, parent_op: BinaryOperator) -> bool {
    let child_op = child.operator;

    // Special case: Logical operators (&&, ||, ??) mixing requires parens
    // prettier adds parens when mixing different logical operators for clarity
    if is_logical_operator(parent_op) && is_logical_operator(child_op) && parent_op != child_op {
        return true;
    }

    let parent_prec = parent_op.precedence();
    let child_prec = child_op.precedence();

    // Need parens when:
    // 1. Child has weaker precedence (lower number) - e.g., (a + b) * c needs parens around +
    // 2. Same precedence but can't flatten - e.g., (a && b) || c needs parens for clarity
    if child_prec < parent_prec {
        return true;
    }

    if child_prec == parent_prec && !parent_op.can_flatten_with(child_op) {
        return true;
    }

    false
}

/// Check if operator is a logical operator (&&, ||, ??)
fn is_logical_operator(op: BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::AmpersandAmpersand
            | BinaryOperator::PipePipe
            | BinaryOperator::QuestionQuestion
    )
}

/// Check if a string is a valid JavaScript identifier
///
/// Valid identifiers:
/// - Start with a letter, underscore, or dollar sign
/// - Contain only letters, digits, underscores, or dollar signs
/// - Can be reserved words (prettier outputs them without quotes)
fn is_valid_js_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();

    // First character must be letter, underscore, or dollar sign
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }

    // Rest can include digits
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' && c != '$' {
            return false;
        }
    }

    true
}

/// Escape a string for single-quoted output
///
/// Escapes single quotes and backslashes in the string content.
fn escape_single_quote_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\'' => result.push_str("\\'"),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ => result.push(c),
        }
    }
    result
}

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
            Expression::BinaryExpression(binary) => self.print_binary_expression(binary),
            Expression::CallExpression(call) => self.print_call_expression(call),
            Expression::MemberExpression(member) => self.print_member_expression(member),
            Expression::ConditionalExpression(cond) => self.print_conditional_expression(cond),
            Expression::ArrowFunctionExpression(arrow) => self.print_arrow_function(arrow),
            Expression::SpreadElement(spread) => self.print_spread_element(spread),
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

    /// Print an array expression: `[elem, ...]`
    ///
    /// Key behaviors (from prettier):
    /// - Numbers-only: fill (multiple per line) when wrapped
    /// - Strings/identifiers/booleans/mixed: one per line when wrapped
    /// - Nested arrays/objects: one per line when wrapped
    /// - Source newlines NOT preserved (unlike objects)
    fn print_array_expression(&mut self, arr: &internal::ArrayExpression) {
        if arr.elements.is_empty() {
            self.write("[]");
            return;
        }

        // Build doc for width-based wrapping
        let doc = self.build_array_doc_with_wrapping(arr);
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset + 1; // +1 for trailing punctuation
        let output = doc::print_doc_at_column(&doc, &self.config, current_col);
        self.write(&output);
    }

    /// Build a Doc for an array with proper wrapping behavior
    fn build_array_doc_with_wrapping(&self, arr: &internal::ArrayExpression) -> Doc {
        if arr.elements.is_empty() {
            return doc::text("[]");
        }

        // Check if this is a "numbers-only" array (use fill) vs other (one-per-line)
        let is_numbers_only = self.is_numbers_only_array(arr);

        if is_numbers_only {
            // Use fill for greedy packing of numbers
            self.build_array_fill_doc(arr)
        } else {
            // Use group with one-per-line for other content
            self.build_array_group_doc(arr)
        }
    }

    /// Check if array contains only numeric literals (for fill behavior)
    fn is_numbers_only_array(&self, arr: &internal::ArrayExpression) -> bool {
        arr.elements.iter().all(|elem| {
            match elem {
                Some(Expression::Literal(lit)) => {
                    matches!(lit.value, LiteralValue::Number(_))
                }
                Some(Expression::UnaryExpression(unary)) => {
                    // -1, +1 are also numeric
                    matches!(
                        unary.operator,
                        internal::UnaryOperator::Minus | internal::UnaryOperator::Plus
                    ) && matches!(
                        unary.argument.as_ref(),
                        Expression::Literal(lit) if matches!(lit.value, LiteralValue::Number(_))
                    )
                }
                _ => false,
            }
        })
    }

    /// Build fill doc for numbers-only arrays (greedy packing)
    fn build_array_fill_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }

            if i < arr.elements.len() - 1 {
                // Separator: comma + line (becomes space in flat mode, newline in break)
                parts.push(doc::concat(vec![doc::text(","), doc::line()]));
            }
        }

        // Wrap in group with brackets
        // Trailing comma goes after last element, before dedent/closing bracket
        doc::group(doc::concat(vec![
            doc::text("["),
            doc::indent(doc::concat(vec![
                doc::softline(),
                doc::fill(parts),
                doc::if_break(doc::text(","), doc::text("")),
            ])),
            doc::softline(),
            doc::text("]"),
        ]))
    }

    /// Build group doc for non-numeric arrays (one per line when broken)
    fn build_array_group_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }

            if i < arr.elements.len() - 1 {
                parts.push(doc::text(","));
                parts.push(doc::line());
            }
        }

        // Wrap in group with brackets
        // Trailing comma goes after last element, before dedent/closing bracket
        doc::group(doc::concat(vec![
            doc::text("["),
            doc::indent(doc::concat(vec![
                doc::softline(),
                doc::concat(parts),
                doc::if_break(doc::text(","), doc::text("")),
            ])),
            doc::softline(),
            doc::text("]"),
        ]))
    }

    /// Print a unary expression: `-x`, `+x`, `!(a && b)`
    fn print_unary_expression(&mut self, unary: &internal::UnaryExpression) {
        self.write(unary.operator.as_str());

        // Add parens around binary/logical expressions since unary has higher precedence
        // e.g., !(a && b) must keep parens, otherwise becomes !a && b (different meaning)
        match unary.argument.as_ref() {
            Expression::BinaryExpression(_) => {
                self.write("(");
                self.print_expression(&unary.argument);
                self.write(")");
            }
            _ => self.print_expression(&unary.argument),
        }
    }

    /// Print a binary expression: `a + b`, `x && y`
    fn print_binary_expression(&mut self, binary: &internal::BinaryExpression) {
        self.print_binary_operand(&binary.left, binary.operator);
        self.write(" ");
        self.write(binary.operator.as_str());
        self.write(" ");
        self.print_binary_operand(&binary.right, binary.operator);
    }

    /// Print operand with parens if needed for clarity
    fn print_binary_operand(&mut self, operand: &Expression, parent_op: BinaryOperator) {
        match operand {
            Expression::BinaryExpression(child) => {
                if needs_parens_for_clarity(child, parent_op) {
                    self.write("(");
                    self.print_expression(operand);
                    self.write(")");
                } else {
                    self.print_expression(operand);
                }
            }
            _ => self.print_expression(operand),
        }
    }

    /// Build a Doc for an array expression (for nested contexts)
    fn build_array_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        // Basic implementation: inline only
        let mut parts = vec![doc::text("[")];
        for (i, elem) in arr.elements.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }
        }
        parts.push(doc::text("]"));
        doc::concat(parts)
    }

    /// Print an object expression: `{ prop: value, ... }`
    ///
    /// Uses doc-builder approach for width-based wrapping and source newline preservation.
    /// Key behaviors:
    /// - Width > 100 chars: wraps to multiline with trailing comma
    /// - Source has newline after `{`: preserves multiline (cascades to outer objects)
    /// - Blank lines between properties: preserved in multiline mode
    /// - Line comments: force multiline (can't be inline)
    /// - Block comments in inline source: stay inline if fits
    fn print_object_expression(&mut self, obj: &internal::ObjectExpression) {
        // Check if object contains line comments (force multiline)
        let has_line_comments = self.has_line_comments_between(obj.span.start, obj.span.end);

        // Check if source has newlines (preserve multiline)
        let has_source_newline = if !obj.properties.is_empty() {
            let first_prop_start = obj.properties[0].span().start;
            has_newline_between(self.source, obj.span.start + 1, first_prop_start)
        } else {
            false
        };

        // Check if object has any comments
        let has_comments = self.has_comments_between(obj.span.start, obj.span.end);

        if has_line_comments || (has_source_newline && has_comments) {
            // Use multiline comment-aware path:
            // - Line comments can't be inline
            // - Source had newlines with comments → preserve multiline
            self.print_object_expression_with_comments(obj);
        } else if has_comments {
            // Block comments only, inline source → build inline string with comments
            self.print_object_expression_inline_with_comments(obj);
        } else {
            // Use doc-builder for width-based wrapping
            // Pass current column for accurate width calculation
            // Account for base_indent_offset (e.g., Svelte wrapper indentation)
            // NOTE: We add 1 to account for trailing punctuation (like `;`) that will be
            // added after the object by the statement printer. This is a conservative
            // approximation. The proper fix would be to build entire statements as docs.
            // TODO: Build full statements as docs for precise width calculation
            let doc = self.build_object_doc(obj);
            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset + 1; // +1 for trailing punctuation
            let output = doc::print_doc_at_column(&doc, &self.config, current_col);
            self.write(&output);
        }
    }

    /// Build a Doc for an object expression
    fn build_object_doc(&self, obj: &internal::ObjectExpression) -> Doc {
        if obj.properties.is_empty() {
            return doc::text("{}");
        }

        // Check if source has newline after opening brace
        let first_prop_start = obj.properties[0].span().start;
        let has_source_newline =
            has_newline_between(self.source, obj.span.start + 1, first_prop_start);

        // Build property docs
        let mut parts = Vec::new();

        for (i, prop) in obj.properties.iter().enumerate() {
            // Check for blank line before this property (preserved in multiline)
            let has_blank_before = if i > 0 {
                let prev_prop = &obj.properties[i - 1];
                let prev_end = prev_prop.value_end();
                has_blank_line_between(self.source, prev_end, prop.span().start)
            } else {
                false
            };

            if has_blank_before {
                // Blank line preservation: literal newline (no indentation)
                // followed by hardline (for proper indentation of next property)
                parts.push(doc::literalline());
                parts.push(doc::hardline());
            }

            // Build property doc
            let prop_doc = self.build_object_property_doc(prop);
            parts.push(prop_doc);

            // Add comma and line break
            if i < obj.properties.len() - 1 {
                parts.push(doc::text(","));
                // Only add line break if next property doesn't have blank line before it
                // (blank line handling above adds its own hardline)
                let next_prop = &obj.properties[i + 1];
                let curr_end = prop.value_end();
                let next_has_blank =
                    has_blank_line_between(self.source, curr_end, next_prop.span().start);

                if !next_has_blank {
                    if has_source_newline {
                        parts.push(doc::hardline());
                    } else {
                        parts.push(doc::line());
                    }
                }
            } else {
                // Last property: trailing comma only when broken
                parts.push(doc::if_break(doc::text(","), doc::text("")));
            }
        }

        // Build the full object doc
        let line_type = if has_source_newline {
            doc::hardline()
        } else {
            doc::softline()
        };

        doc::group(doc::concat(vec![
            doc::text("{"),
            doc::indent(doc::concat(vec![line_type.clone(), doc::concat(parts)])),
            line_type,
            doc::text("}"),
        ]))
    }

    /// Build a Doc for an object property (either Property or SpreadElement)
    fn build_object_property_doc(&self, prop: &internal::ObjectProperty) -> Doc {
        match prop {
            internal::ObjectProperty::Property(p) => self.build_property_doc(p),
            internal::ObjectProperty::SpreadElement(s) => self.build_spread_doc(s),
        }
    }

    /// Build a Doc for a single property
    fn build_property_doc(&self, prop: &internal::Property) -> Doc {
        let key_doc = self.build_property_key_doc(&prop.key);

        if prop.shorthand {
            key_doc
        } else {
            let value_doc = self.build_expression_doc(&prop.value);
            doc::concat(vec![key_doc, doc::text(": "), value_doc])
        }
    }

    /// Build a Doc for a property key
    ///
    /// String literal keys that are valid identifiers are output without quotes.
    /// Example: `{"key": 1}` → `{key: 1}`, but `{"kebab-case": 1}` keeps quotes.
    fn build_property_key_doc(&self, key: &Expression) -> Doc {
        match key {
            Expression::Literal(Literal {
                value: LiteralValue::String { content, quote: _ },
                ..
            }) => {
                // Check if the string content is a valid JS identifier
                if is_valid_js_identifier(content) {
                    // Output without quotes
                    doc::text(content.clone())
                } else {
                    // Keep as quoted string (normalized to single quotes)
                    doc::text(format!("'{}'", escape_single_quote_string(content)))
                }
            }
            _ => self.build_expression_doc(key),
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
            Expression::BinaryExpression(binary) => self.build_binary_doc(binary),
            Expression::CallExpression(call) => self.build_call_doc(call),
            Expression::MemberExpression(member) => self.build_member_doc(member),
            Expression::ConditionalExpression(cond) => {
                self.build_conditional_doc_with_wrapping(cond)
            }
            Expression::ArrowFunctionExpression(arrow) => self.build_arrow_doc(arrow),
            Expression::SpreadElement(spread) => self.build_spread_doc(spread),
        }
    }

    /// Build a Doc for a unary expression
    fn build_unary_doc(&self, unary: &internal::UnaryExpression) -> Doc {
        let argument_doc = match unary.argument.as_ref() {
            // Add parens around binary/logical expressions (unary has higher precedence)
            Expression::BinaryExpression(_) => doc::concat(vec![
                doc::text("("),
                self.build_expression_doc(&unary.argument),
                doc::text(")"),
            ]),
            _ => self.build_expression_doc(&unary.argument),
        };

        doc::concat(vec![doc::text(unary.operator.as_str()), argument_doc])
    }

    /// Build a Doc for a binary expression
    ///
    /// Implements prettier's "add parens for clarity" behavior where mixing certain
    /// operators requires parentheses for readability:
    /// - `a && b || c` → `(a && b) || c` (mixing && and ||)
    /// - `a || b && c` → `a || (b && c)`
    /// - `a == b == c` → `(a == b) == c` (chained equality)
    ///
    /// See: prettier/src/language-js/print/binaryish.js
    fn build_binary_doc(&self, binary: &internal::BinaryExpression) -> Doc {
        let left_doc = self.build_binary_operand_doc(&binary.left, binary.operator);
        let right_doc = self.build_binary_operand_doc(&binary.right, binary.operator);

        doc::concat(vec![
            left_doc,
            doc::text(" "),
            doc::text(binary.operator.as_str()),
            doc::text(" "),
            right_doc,
        ])
    }

    /// Build operand with parens if needed for clarity
    fn build_binary_operand_doc(&self, operand: &Expression, parent_op: BinaryOperator) -> Doc {
        match operand {
            Expression::BinaryExpression(child) => {
                let child_doc = self.build_expression_doc(operand);

                if needs_parens_for_clarity(child, parent_op) {
                    doc::concat(vec![doc::text("("), child_doc, doc::text(")")])
                } else {
                    child_doc
                }
            }
            _ => self.build_expression_doc(operand),
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
        }
    }

    /// Print object expression inline with block comments
    ///
    /// For objects that have only block comments (no line comments) and inline source,
    /// we try to keep them inline. Format: `{/* comment */ a: 1, b: 2}`
    fn print_object_expression_inline_with_comments(&mut self, obj: &internal::ObjectExpression) {
        // Build the inline representation
        let mut parts = Vec::new();
        parts.push("{".to_string());

        let mut prev_prop_end = obj.span.start + 1; // After opening brace
        let is_first_prop_check = |i: usize| i == 0;

        for (i, prop) in obj.properties.iter().enumerate() {
            // Print leading comments (block only since we're inline)
            // Skip comments that were trailing comments for the previous property
            // (those are on the same line as prev_prop_end)
            for comment in self.comments.iter() {
                if comment.span.start >= prev_prop_end
                    && comment.span.end <= prop.span().start
                    && comment.is_block
                {
                    // Skip if this is a trailing comment from previous property
                    // (on same line as prev_prop_end, unless it's the first property)
                    if !is_first_prop_check(i)
                        && is_same_line(self.source, prev_prop_end, comment.span.start)
                    {
                        continue;
                    }
                    parts.push(format!("/*{}*/ ", comment.content));
                }
            }

            // Print property
            match prop {
                internal::ObjectProperty::Property(p) => {
                    let key_str = self.property_key_to_string(&p.key);
                    if p.shorthand {
                        parts.push(key_str);
                    } else {
                        // Check for comments between key and value (after colon)
                        let colon_pos = self.find_colon_after(p.key.span().end);
                        let mut value_prefix = String::new();
                        for comment in self.comments.iter() {
                            if comment.span.start > colon_pos
                                && comment.span.end <= p.value.span().start
                                && comment.is_block
                            {
                                value_prefix.push_str(&format!("/*{}*/ ", comment.content));
                            }
                        }

                        let value_str = self.expression_to_string(&p.value);
                        parts.push(format!("{}: {}{}", key_str, value_prefix, value_str));
                    }
                }
                internal::ObjectProperty::SpreadElement(s) => {
                    parts.push(format!("...{}", self.expression_to_string(&s.argument)));
                }
            }

            // Print trailing comments after value (block only)
            let prop_end = prop.value_end();
            for comment in self.comments.iter() {
                if comment.span.start >= prop_end
                    && comment.span.end <= obj.span.end
                    && is_same_line(self.source, prop_end, comment.span.start)
                    && comment.is_block
                {
                    parts.push(format!(" /*{}*/", comment.content));
                }
            }

            // Add comma separator
            if i < obj.properties.len() - 1 {
                parts.push(", ".to_string());
            }

            prev_prop_end = prop_end;
        }

        parts.push("}".to_string());

        // Join and output
        let inline_str = parts.concat();
        self.write(&inline_str);
    }

    /// Convert an expression to a string (for inline object building)
    fn expression_to_string(&self, expr: &Expression) -> String {
        match expr {
            Expression::Literal(lit) => match &lit.value {
                LiteralValue::Number(n) => n.to_string(),
                LiteralValue::String { content: _, quote } => {
                    let start = lit.span.start as usize;
                    let end = lit.span.end as usize;
                    let raw_literal = &self.source[start..end];
                    let raw_content = &raw_literal[1..raw_literal.len() - 1];
                    format_string_literal(raw_content, *quote, StringFormatOptions::default())
                }
                LiteralValue::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
                LiteralValue::Null => "null".to_string(),
            },
            Expression::Identifier(id) => self.resolve_symbol(id.name),
            Expression::ObjectExpression(obj) => {
                // Recursively build nested object inline
                let mut parts = Vec::new();
                parts.push("{".to_string());

                for (i, prop) in obj.properties.iter().enumerate() {
                    match prop {
                        internal::ObjectProperty::Property(p) => {
                            let key_str = self.property_key_to_string(&p.key);
                            if p.shorthand {
                                parts.push(key_str);
                            } else {
                                let value_str = self.expression_to_string(&p.value);
                                parts.push(format!("{}: {}", key_str, value_str));
                            }
                        }
                        internal::ObjectProperty::SpreadElement(s) => {
                            parts.push(format!("...{}", self.expression_to_string(&s.argument)));
                        }
                    }

                    if i < obj.properties.len() - 1 {
                        parts.push(", ".to_string());
                    }
                }

                parts.push("}".to_string());
                parts.concat()
            }
            Expression::ArrayExpression(arr) => {
                // Recursively build nested array inline
                let mut parts = Vec::new();
                parts.push("[".to_string());

                for (i, elem) in arr.elements.iter().enumerate() {
                    if let Some(e) = elem {
                        parts.push(self.expression_to_string(e));
                    }
                    if i < arr.elements.len() - 1 {
                        parts.push(", ".to_string());
                    }
                }

                parts.push("]".to_string());
                parts.concat()
            }
            Expression::UnaryExpression(unary) => {
                format!(
                    "{}{}",
                    unary.operator.as_str(),
                    self.expression_to_string(&unary.argument)
                )
            }
            Expression::BinaryExpression(binary) => {
                let left = self.expression_to_string(&binary.left);
                let right = self.expression_to_string(&binary.right);

                // Wrap operands in parens if needed
                let left_str = if let Expression::BinaryExpression(child) = binary.left.as_ref() {
                    if needs_parens_for_clarity(child, binary.operator) {
                        format!("({})", left)
                    } else {
                        left
                    }
                } else {
                    left
                };

                let right_str = if let Expression::BinaryExpression(child) = binary.right.as_ref() {
                    if needs_parens_for_clarity(child, binary.operator) {
                        format!("({})", right)
                    } else {
                        right
                    }
                } else {
                    right
                };

                format!("{} {} {}", left_str, binary.operator.as_str(), right_str)
            }
            Expression::ArrowFunctionExpression(arrow) => self.arrow_function_to_string(arrow),
            Expression::SpreadElement(spread) => {
                format!("...{}", self.expression_to_string(&spread.argument))
            }
            Expression::CallExpression(call) => {
                let callee = self.expression_to_string(&call.callee);
                let args: Vec<String> = call
                    .arguments
                    .iter()
                    .map(|arg| self.expression_to_string(arg))
                    .collect();
                let opt = if call.optional { "?." } else { "" };
                format!("{}{}({})", callee, opt, args.join(", "))
            }
            Expression::MemberExpression(member) => {
                let obj = self.expression_to_string(&member.object);
                let prop = self.expression_to_string(&member.property);
                if member.computed {
                    let opt = if member.optional { "?." } else { "" };
                    format!("{}{}[{}]", obj, opt, prop)
                } else {
                    let dot = if member.optional { "?." } else { "." };
                    format!("{}{}{}", obj, dot, prop)
                }
            }
            Expression::ConditionalExpression(cond) => {
                format!(
                    "{} ? {} : {}",
                    self.expression_to_string(&cond.test),
                    self.expression_to_string(&cond.consequent),
                    self.expression_to_string(&cond.alternate)
                )
            }
        }
    }

    /// Convert a property key expression to a string
    ///
    /// String keys that are valid identifiers are output without quotes.
    fn property_key_to_string(&self, key: &Expression) -> String {
        match key {
            Expression::Literal(Literal {
                value: LiteralValue::String { content, .. },
                ..
            }) => {
                if is_valid_js_identifier(content) {
                    content.clone()
                } else {
                    format!("'{}'", escape_single_quote_string(content))
                }
            }
            _ => self.expression_to_string(key),
        }
    }

    /// Print a property key (for imperative path)
    ///
    /// String keys that are valid identifiers are output without quotes.
    fn print_property_key(&mut self, key: &Expression) {
        match key {
            Expression::Literal(Literal {
                value: LiteralValue::String { content, .. },
                ..
            }) => {
                if is_valid_js_identifier(content) {
                    self.write(content);
                } else {
                    self.write(&format!("'{}'", escape_single_quote_string(content)));
                }
            }
            _ => self.print_expression(key),
        }
    }

    /// Find the position of `:` after a position (for finding colon in property)
    fn find_colon_after(&self, start: u32) -> u32 {
        let start = start as usize;
        let slice = &self.source[start..];
        if let Some(offset) = slice.find(':') {
            (start + offset) as u32
        } else {
            start as u32
        }
    }

    /// Print object expression with comments (preserves existing behavior)
    fn print_object_expression_with_comments(&mut self, obj: &internal::ObjectExpression) {
        self.write("{");

        if !obj.properties.is_empty() {
            self.write("\n");
            self.indent_level += 1;

            let mut prev_end = obj.span.start + 1; // After opening brace

            for (i, prop) in obj.properties.iter().enumerate() {
                // Print leading comments before property
                // (don't use print_leading_comments - it skips same-line comments)
                // Returns true if a same-line comment was printed (indent already handled)
                let is_first_prop = i == 0;
                let had_same_line_comment =
                    self.print_object_leading_comments(prev_end, prop.span().start, is_first_prop);

                if !had_same_line_comment {
                    self.write_indent();
                }

                // Print property
                match prop {
                    internal::ObjectProperty::Property(p) => {
                        self.print_property_key(&p.key);

                        // Print colon and value (unless shorthand)
                        if !p.shorthand {
                            self.write(": ");
                            self.print_expression(&p.value);
                        }
                    }
                    internal::ObjectProperty::SpreadElement(s) => {
                        self.write("...");
                        self.print_expression(&s.argument);
                    }
                }

                // Print trailing inline comment after property value (same line only)
                let prop_end = prop.value_end();

                // Print trailing inline comments on same line as property
                // Block comments go before comma, line comments go after comma (matches prettier)
                let mut has_line_comment = false;
                for comment in self.comments.iter() {
                    if comment.span.start >= prop_end
                        && comment.span.start < obj.span.end
                        && is_same_line(self.source, prop_end, comment.span.start)
                    {
                        if comment.is_block {
                            // Block comment: print before comma
                            self.write(" ");
                            self.print_comment(comment);
                        } else {
                            // Line comment: defer until after comma
                            has_line_comment = true;
                        }
                    }
                }

                self.write(",");

                // Print line comments after comma
                if has_line_comment {
                    for comment in self.comments.iter() {
                        if comment.span.start >= prop_end
                            && comment.span.start < obj.span.end
                            && is_same_line(self.source, prop_end, comment.span.start)
                            && !comment.is_block
                        {
                            self.write(" ");
                            self.print_comment(comment);
                        }
                    }
                }

                self.write("\n");
                prev_end = prop.span().end;
            }

            // Print any final comments before closing brace
            self.print_leading_comments(prev_end, obj.span.end);

            self.indent_level -= 1;
            self.write_indent();
        }

        self.write("}");
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

    /// Print a call expression: `foo()`, `obj.method(arg1, arg2)`
    ///
    /// For method chains like `arr.filter().map()`, wraps with leading `.`:
    /// ```javascript
    /// arr
    ///     .filter(...)
    ///     .map(...)
    /// ```
    ///
    /// For standalone calls, wraps args when they exceed print_width:
    /// ```javascript
    /// fn(
    ///     arg1,
    ///     arg2,
    /// )
    /// ```
    fn print_call_expression(&mut self, call: &internal::CallExpression) {
        // Check if this is part of a chain (callee is member or call expression)
        let is_chain = matches!(
            &*call.callee,
            internal::Expression::MemberExpression(_) | internal::Expression::CallExpression(_)
        );

        let doc = if is_chain {
            // Use chain wrapping - collect all segments
            self.build_chain_doc_with_wrapping(&internal::Expression::CallExpression(call.clone()))
        } else {
            // Simple call - just wrap args
            self.build_call_doc_with_wrapping(call)
        };

        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset + 1;
        let output = doc::print_doc_at_column(&doc, &self.config, current_col);
        self.write(&output);
    }

    /// Build a Doc for a call expression with argument wrapping (not chain-aware)
    fn build_call_doc_with_wrapping(&self, call: &internal::CallExpression) -> Doc {
        let callee = self.build_expression_doc(&call.callee);

        // Handle optional chaining
        let callee = if call.optional {
            doc::concat(vec![callee, doc::text("?.")])
        } else {
            callee
        };

        // Empty args: just `fn()`
        if call.arguments.is_empty() {
            return doc::concat(vec![callee, doc::text("()")]);
        }

        // Build args with line separators (one per line when broken)
        let mut arg_parts = Vec::new();
        for (i, arg) in call.arguments.iter().enumerate() {
            if i > 0 {
                arg_parts.push(doc::text(","));
                arg_parts.push(doc::line());
            }
            arg_parts.push(self.build_expression_doc(arg));
        }

        // Wrap in group with parens
        doc::group(doc::concat(vec![
            callee,
            doc::text("("),
            doc::indent(doc::concat(vec![
                doc::softline(),
                doc::concat(arg_parts),
                doc::if_break(doc::text(","), doc::text("")),
            ])),
            doc::softline(),
            doc::text(")"),
        ]))
    }

    /// Build a Doc for a chain (method chain or member chain) with wrapping
    fn build_chain_doc_with_wrapping(&self, expr: &internal::Expression) -> Doc {
        let segments = self.collect_chain_segments(expr);

        if segments.len() <= 1 {
            return segments.into_iter().next().unwrap_or(doc::text(""));
        }

        // Build chain with optional breaks between segments
        let mut parts = Vec::new();
        for (i, segment) in segments.into_iter().enumerate() {
            if i == 0 {
                parts.push(segment);
            } else {
                // Each subsequent segment can break with indent
                parts.push(doc::indent(doc::concat(vec![doc::softline(), segment])));
            }
        }

        doc::group(doc::concat(parts))
    }

    /// Print a member expression: `obj.prop`, `arr[0]`
    ///
    /// For property-only chains, keeps inline (relies on assignment-level wrapping).
    /// For method chains (containing calls), wraps with leading `.`.
    fn print_member_expression(&mut self, member: &internal::MemberExpression) {
        // Check if this chain contains any calls (method chain vs property chain)
        let has_calls = chain_has_calls(&internal::Expression::MemberExpression(member.clone()));

        if has_calls {
            // Method chain - use chain wrapping
            let doc = self.build_chain_doc_with_wrapping(&internal::Expression::MemberExpression(
                member.clone(),
            ));
            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset + 1;
            let output = doc::print_doc_at_column(&doc, &self.config, current_col);
            self.write(&output);
        } else {
            // Property chain - keep inline, print directly
            self.print_expression(&member.object);

            if member.computed {
                if member.optional {
                    self.write("?.[");
                } else {
                    self.write("[");
                }
                self.print_expression(&member.property);
                self.write("]");
            } else {
                if member.optional {
                    self.write("?.");
                } else {
                    self.write(".");
                }
                self.print_expression(&member.property);
            }
        }
    }

    /// Collect chain segments from a member/call expression chain
    ///
    /// Flattens `a.b.c().d` into segments: [`a`, `.b`, `.c()`, `.d`]
    fn collect_chain_segments(&self, expr: &internal::Expression) -> Vec<Doc> {
        let mut segments = Vec::new();
        self.collect_chain_segments_recursive(expr, &mut segments);
        segments
    }

    fn collect_chain_segments_recursive(
        &self,
        expr: &internal::Expression,
        segments: &mut Vec<Doc>,
    ) {
        match expr {
            internal::Expression::MemberExpression(member) => {
                // Recurse into object first
                self.collect_chain_segments_recursive(&member.object, segments);

                // Build this segment: `.prop` or `[expr]`
                let segment = if member.computed {
                    let prop = self.build_expression_doc(&member.property);
                    if member.optional {
                        doc::concat(vec![doc::text("?.["), prop, doc::text("]")])
                    } else {
                        doc::concat(vec![doc::text("["), prop, doc::text("]")])
                    }
                } else {
                    let prop = self.build_expression_doc(&member.property);
                    if member.optional {
                        doc::concat(vec![doc::text("?."), prop])
                    } else {
                        doc::concat(vec![doc::text("."), prop])
                    }
                };
                segments.push(segment);
            }
            internal::Expression::CallExpression(call) => {
                // Recurse into callee first
                self.collect_chain_segments_recursive(&call.callee, segments);

                // Build this segment: the call arguments `(arg1, arg2)`
                // Use wrapping for the args
                let args_doc = if call.arguments.is_empty() {
                    doc::text("()")
                } else {
                    let mut arg_parts = Vec::new();
                    for (i, arg) in call.arguments.iter().enumerate() {
                        if i > 0 {
                            arg_parts.push(doc::text(","));
                            arg_parts.push(doc::line());
                        }
                        arg_parts.push(self.build_expression_doc(arg));
                    }
                    doc::group(doc::concat(vec![
                        doc::text("("),
                        doc::indent(doc::concat(vec![
                            doc::softline(),
                            doc::concat(arg_parts),
                            doc::if_break(doc::text(","), doc::text("")),
                        ])),
                        doc::softline(),
                        doc::text(")"),
                    ]))
                };

                // Append args to the last segment (if any) or create new segment
                if let Some(last) = segments.pop() {
                    segments.push(doc::concat(vec![last, args_doc]));
                } else {
                    segments.push(args_doc);
                }
            }
            // Base case: identifiers, literals, etc.
            _ => {
                segments.push(self.build_expression_doc(expr));
            }
        }
    }

    /// Print a conditional (ternary) expression: `a ? b : c`
    ///
    /// When the line exceeds print_width, wraps to:
    /// ```javascript
    /// longCondition
    ///     ? consequent
    ///     : alternate
    /// ```
    ///
    /// Also wraps nested conditionals in the consequent position with parentheses
    /// for readability: `a ? (b ? c : d) : e`.
    fn print_conditional_expression(&mut self, cond: &internal::ConditionalExpression) {
        let doc = self.build_conditional_doc_with_wrapping(cond);
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset + 1;
        let output = doc::print_doc_at_column(&doc, &self.config, current_col);
        self.write(&output);
    }

    /// Build a Doc for a conditional expression with wrapping support
    fn build_conditional_doc_with_wrapping(&self, cond: &internal::ConditionalExpression) -> Doc {
        let test = self.build_expression_doc(&cond.test);
        let consequent = self.build_expression_doc(&cond.consequent);
        let alternate = self.build_expression_doc(&cond.alternate);

        // Wrap nested conditional in consequent with parentheses
        let consequent = if matches!(
            &*cond.consequent,
            internal::Expression::ConditionalExpression(_)
        ) {
            doc::concat(vec![doc::text("("), consequent, doc::text(")")])
        } else {
            consequent
        };

        // When flat: `test ? consequent : alternate`
        // When broken: `test\n\t? consequent\n\t: alternate`
        doc::group(doc::concat(vec![
            test,
            doc::indent(doc::concat(vec![
                doc::line(),
                doc::text("? "),
                consequent,
                doc::line(),
                doc::text(": "),
                alternate,
            ])),
        ]))
    }

    /// Build a Doc for an arrow function
    fn build_arrow_doc(&self, arrow: &internal::ArrowFunctionExpression) -> Doc {
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
    fn build_spread_doc(&self, spread: &internal::SpreadElement) -> Doc {
        doc::concat(vec![
            doc::text("..."),
            self.build_expression_doc(&spread.argument),
        ])
    }

    /// Build a Doc for a call expression
    fn build_call_doc(&self, call: &internal::CallExpression) -> Doc {
        let mut parts = Vec::new();

        parts.push(self.build_expression_doc(&call.callee));

        if call.optional {
            parts.push(doc::text("?."));
        }

        parts.push(doc::text("("));

        for (i, arg) in call.arguments.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_expression_doc(arg));
        }

        parts.push(doc::text(")"));

        doc::concat(parts)
    }

    /// Build a Doc for a member expression with optional breaking at dots
    ///
    /// For long property chains, uses greedy line packing - fits as many segments
    /// as possible on each line before breaking:
    /// ```javascript
    /// const long =
    ///     obj.prop1.prop2.prop3.prop4.prop5.prop6.prop7.prop8.prop9.prop10.prop11.prop12.prop13.prop14
    ///         .prop15.prop16;
    /// ```
    fn build_member_doc(&self, member: &internal::MemberExpression) -> Doc {
        // Collect all segments of the chain (root + each member access)
        let mut segments: Vec<Doc> = Vec::new();
        self.collect_member_segments(
            &internal::Expression::MemberExpression(member.clone()),
            &mut segments,
        );

        if segments.len() <= 1 {
            return segments.into_iter().next().unwrap_or(doc::text(""));
        }

        // Build fill parts: [segment, softline, segment, softline, ...]
        // Fill uses greedy packing - fits as many on each line as possible
        let mut fill_parts = Vec::new();
        for (i, segment) in segments.into_iter().enumerate() {
            if i > 0 {
                // Separator before each segment (except first)
                // softline: nothing in flat mode, newline+indent in break mode
                fill_parts.push(doc::softline());
            }
            fill_parts.push(segment);
        }

        // Wrap in group with indent so breaks get proper indentation
        doc::group(doc::indent(doc::fill(fill_parts)))
    }

    /// Collect segments from a member expression chain
    ///
    /// Flattens `a.b.c[d]` into segments: [`a`, `.b`, `.c`, `[d]`]
    fn collect_member_segments(&self, expr: &internal::Expression, segments: &mut Vec<Doc>) {
        match expr {
            internal::Expression::MemberExpression(member) => {
                // Recurse into object first
                self.collect_member_segments(&member.object, segments);

                // Build this segment
                let segment = if member.computed {
                    let prop = self.build_expression_doc(&member.property);
                    if member.optional {
                        doc::concat(vec![doc::text("?.["), prop, doc::text("]")])
                    } else {
                        doc::concat(vec![doc::text("["), prop, doc::text("]")])
                    }
                } else {
                    let prop = self.build_expression_doc(&member.property);
                    if member.optional {
                        doc::concat(vec![doc::text("?."), prop])
                    } else {
                        doc::concat(vec![doc::text("."), prop])
                    }
                };
                segments.push(segment);
            }
            // Base case: root of the chain
            _ => {
                segments.push(self.build_expression_doc(expr));
            }
        }
    }

    /// Convert an arrow function to a string (for inline building)
    fn arrow_function_to_string(&self, arrow: &internal::ArrowFunctionExpression) -> String {
        let mut result = String::new();

        // Parameters
        result.push('(');
        for (i, param) in arrow.params.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&self.resolve_symbol(param.name));
        }
        result.push_str(") => ");

        // Body
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                result.push_str(&self.expression_to_string(expr));
            }
            internal::ArrowFunctionBody::BlockStatement { span } => {
                let raw = span.extract(self.source);
                result.push_str(raw);
            }
        }

        result
    }
}
