// Object expression printing for TypeScript
//
// Handles printing of object expressions with:
// - Width-based wrapping via doc-builder
// - Comment preservation (block and line comments)
// - Property shorthand detection
// - String key normalization (unquote valid identifiers)
// - Blank line preservation between properties

use std::fmt::Write;

use super::Printer;
use super::expression_stringifier::{escape_single_quote_string, is_valid_js_identifier};
use crate::ast::internal::{self, Expression, Literal, LiteralValue};
use tsv_lang::SymbolResolver;
use tsv_lang::comments_in_range;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::{has_blank_line_between, has_newline_between, is_same_line};

impl<'a> Printer<'a> {
    /// Print an object expression: `{ prop: value, ... }`
    ///
    /// Uses doc-builder approach for width-based wrapping and source newline preservation.
    /// Key behaviors:
    /// - Width > 100 chars: wraps to multiline with trailing comma
    /// - Source has newline after `{`: preserves multiline (cascades to outer objects)
    /// - Blank lines between properties: preserved in multiline mode
    /// - Line comments: force multiline (can't be inline)
    /// - Block comments in inline source: stay inline if fits
    /// - Empty objects with comments: expand to multiline
    pub(super) fn print_object_expression(&mut self, obj: &internal::ObjectExpression) {
        // Check if object contains line comments (force multiline)
        let has_line_comments = self.has_line_comments_between(obj.span.start, obj.span.end);

        // Check if object has any comments
        let has_comments = self.has_comments_between(obj.span.start, obj.span.end);

        // Handle empty objects with comments specially
        if obj.properties.is_empty() && has_comments {
            self.print_empty_object_with_comments(obj);
            return;
        }

        // Check if source has newlines (preserve multiline)
        let has_source_newline = if !obj.properties.is_empty() {
            let first_prop_start = obj.properties[0].span().start;
            has_newline_between(self.source, obj.span.start + 1, first_prop_start)
        } else {
            false
        };

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
            let doc = self.build_object_doc(obj);
            self.write_doc_with_margin(&doc);
        }
    }

    /// Print an empty object that contains only comments
    ///
    /// Prettier always expands empty objects with comments to multiline:
    /// `{/* comment */}` → `{\n\t/* comment */\n}`
    fn print_empty_object_with_comments(&mut self, obj: &internal::ObjectExpression) {
        self.print_empty_container_with_comments("{", "}", obj.span.start + 1, obj.span.end - 1);
    }

    /// Build a Doc for an object expression
    pub(super) fn build_object_doc(&self, obj: &internal::ObjectExpression) -> Doc {
        if obj.properties.is_empty() {
            return doc::text("{}");
        }

        // Check if source has newline after opening brace
        let first_prop_start = obj.properties[0].span().start;
        let has_source_newline =
            has_newline_between(self.source, obj.span.start + 1, first_prop_start);

        // Check if any property value has multiline content (e.g., line continuation strings)
        // Prettier expands objects containing multiline strings (recursively)
        let has_multiline = obj.properties.iter().any(|prop| match prop {
            internal::ObjectProperty::Property(p) => {
                super::has_multiline_content(&p.value, self.source)
            }
            internal::ObjectProperty::SpreadElement(s) => {
                super::has_multiline_content(&s.argument, self.source)
            }
        });

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
                    if has_source_newline || has_multiline {
                        parts.push(doc::hardline());
                    } else {
                        parts.push(doc::line());
                    }
                }
            } else {
                // Last property: trailing comma only when broken
                parts.push(doc::trailing_comma());
            }
        }

        // Build the full object doc
        // Force multiline if source had newlines OR content has multiline strings
        let line_type = if has_source_newline || has_multiline {
            doc::hardline()
        } else {
            doc::softline()
        };

        let inner = doc::concat(vec![line_type.clone(), doc::concat(parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, line_type);

        doc::group(doc::concat(vec![
            doc::text("{"),
            indented_content,
            closing_line,
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
        // For computed keys, use expression doc (preserves string quotes)
        // For regular keys, use property key doc (converts strings to bare identifiers when valid)
        let key_doc = if prop.computed {
            doc::concat(vec![
                doc::text("["),
                self.build_expression_doc(&prop.key),
                doc::text("]"),
            ])
        } else {
            self.build_property_key_doc(&prop.key)
        };

        // Add getter/setter prefix if applicable
        let key_doc = match prop.kind {
            internal::PropertyKind::Get => doc::concat(vec![doc::text("get "), key_doc]),
            internal::PropertyKind::Set => doc::concat(vec![doc::text("set "), key_doc]),
            internal::PropertyKind::Init => key_doc,
        };

        // Handle getter/setter vs method vs regular property
        if matches!(
            prop.kind,
            internal::PropertyKind::Get | internal::PropertyKind::Set
        ) {
            // Getter/setter: `get x() {}` or `set x(v) {}`
            if let Expression::FunctionExpression(func) = &prop.value {
                let func_doc = self.build_function_doc_body(func);
                doc::concat(vec![key_doc, func_doc])
            } else {
                key_doc
            }
        } else if prop.method {
            // Method shorthand: `foo() {}`, `async foo() {}`, `*gen() {}`, or `async *gen() {}`
            if let Expression::FunctionExpression(func) = &prop.value {
                let func_doc = self.build_function_doc_body(func);
                // Build prefix: async? + *?
                let mut parts = Vec::new();
                if func.r#async {
                    parts.push(doc::text("async "));
                }
                if func.generator {
                    parts.push(doc::text("*"));
                }
                parts.push(key_doc);
                parts.push(func_doc);
                doc::concat(parts)
            } else {
                // Fallback for malformed AST
                let value_doc = self.build_expression_doc(&prop.value);
                doc::concat(vec![key_doc, doc::text(": "), value_doc])
            }
        } else if prop.shorthand {
            // Handle shorthand with default value: {a = 1}
            // The value is an AssignmentExpression (or AssignmentPattern in proper patterns)
            if let Expression::AssignmentExpression(assign) = &prop.value {
                let default_doc = self.build_expression_doc(&assign.right);
                doc::concat(vec![key_doc, doc::text(" = "), default_doc])
            } else if let Expression::AssignmentPattern(pattern) = &prop.value {
                let default_doc = self.build_expression_doc(&pattern.right);
                doc::concat(vec![key_doc, doc::text(" = "), default_doc])
            } else {
                key_doc
            }
        } else {
            // Regular property: use unified assignment layout
            // Calculate key width for short key detection
            let key_width = self.estimate_key_width(&prop.key, prop.computed);
            let context =
                super::assignment::LayoutContext::for_property(key_width, self.config.tab_width);
            self.build_assignment_layout(key_doc, ":", &prop.value, context)
        }
    }

    /// Estimate the width of a property key for layout decisions
    fn estimate_key_width(&self, key: &Expression, computed: bool) -> usize {
        let base_width = match key {
            Expression::Identifier(id) => self.resolve_symbol(id.name).len(),
            Expression::Literal(lit) => match &lit.value {
                LiteralValue::String { content, .. } => {
                    // Check if it's a valid identifier (no quotes needed)
                    if is_valid_js_identifier(content) {
                        content.len()
                    } else {
                        content.len() + 2 // Add quotes
                    }
                }
                LiteralValue::Number(_) => {
                    // Use span to get actual source width
                    (lit.span.end - lit.span.start) as usize
                }
                _ => 10, // Conservative estimate
            },
            _ => 10, // Conservative estimate for complex keys
        };
        if computed {
            base_width + 2 // Add brackets
        } else {
            base_width
        }
    }

    /// Build a Doc for a property key
    ///
    /// String literal keys that are valid identifiers are output without quotes.
    /// Example: `{"key": 1}` → `{key: 1}`, but `{"kebab-case": 1}` keeps quotes.
    pub(super) fn build_property_key_doc(&self, key: &Expression) -> Doc {
        match key {
            Expression::Literal(Literal {
                value: LiteralValue::String { content, quote: _ },
                ..
            }) => {
                // Check if the string content is a valid JS identifier
                if is_valid_js_identifier(content) {
                    // Output without quotes
                    doc::text_owned(content.clone())
                } else {
                    // Keep as quoted string (normalized to single quotes)
                    doc::text_owned(format!("'{}'", escape_single_quote_string(content)))
                }
            }
            _ => self.build_expression_doc(key),
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
            // Uses binary search: O(log n + k)
            for comment in comments_in_range(self.comments, prev_prop_end, prop.span().start) {
                if comment.is_block {
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
                    // For computed keys, use expression_to_string (preserves string quotes)
                    // For regular keys, use property_key_to_string (strips quotes from valid identifiers)
                    let base_key = if p.computed {
                        format!("[{}]", self.expression_to_string(&p.key))
                    } else {
                        self.property_key_to_string(&p.key)
                    };
                    // Add getter/setter prefix if applicable
                    let key_str = match p.kind {
                        internal::PropertyKind::Get => format!("get {base_key}"),
                        internal::PropertyKind::Set => format!("set {base_key}"),
                        internal::PropertyKind::Init => base_key,
                    };
                    if matches!(
                        p.kind,
                        internal::PropertyKind::Get | internal::PropertyKind::Set
                    ) {
                        // Getter/setter: `get x() {}` or `set x(v) {}`
                        let value_str = self.expression_to_string(&p.value);
                        parts.push(format!("{key_str}{value_str}"));
                    } else if p.method {
                        // Method shorthand: `foo() {}`, `async foo() {}`, `*gen() {}`, or `async *gen() {}`
                        let value_str = self.expression_to_string(&p.value);
                        // Build prefix: async? + *?
                        if let Expression::FunctionExpression(func) = &p.value {
                            let async_prefix = if func.r#async { "async " } else { "" };
                            let gen_prefix = if func.generator { "*" } else { "" };
                            parts.push(format!("{async_prefix}{gen_prefix}{key_str}{value_str}"));
                        } else {
                            parts.push(format!("{key_str}{value_str}"));
                        }
                    } else if p.shorthand {
                        // Handle shorthand with default value: {a = 1}
                        if let Expression::AssignmentExpression(assign) = &p.value {
                            let default_str = self.expression_to_string(&assign.right);
                            parts.push(format!("{key_str} = {default_str}"));
                        } else if let Expression::AssignmentPattern(pattern) = &p.value {
                            let default_str = self.expression_to_string(&pattern.right);
                            parts.push(format!("{key_str} = {default_str}"));
                        } else {
                            parts.push(key_str);
                        }
                    } else {
                        // Check for comments between key and value (after colon)
                        // Uses binary search: O(log n + k)
                        let colon_pos = self.find_colon_after(p.key.span().end);
                        let mut value_prefix = String::new();
                        for comment in
                            comments_in_range(self.comments, colon_pos + 1, p.value.span().start)
                        {
                            if comment.is_block {
                                let _ = write!(value_prefix, "/*{}*/ ", comment.content);
                            }
                        }

                        let value_str = self.expression_to_string(&p.value);
                        parts.push(format!("{key_str}: {value_prefix}{value_str}"));
                    }
                }
                internal::ObjectProperty::SpreadElement(s) => {
                    parts.push(format!("...{}", self.expression_to_string(&s.argument)));
                }
            }

            // Print trailing comments after value (block only)
            // Uses binary search: O(log n + k)
            let prop_end = prop.value_end();
            for comment in comments_in_range(self.comments, prop_end, obj.span.end) {
                if is_same_line(self.source, prop_end, comment.span.start) && comment.is_block {
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

    /// Print a property key (for imperative path)
    ///
    /// String keys that are valid identifiers are output without quotes.
    pub(super) fn print_property_key(&mut self, key: &Expression) {
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
    pub(super) fn find_colon_after(&self, start: u32) -> u32 {
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
            self.indent_level += self.container_indent_increment();

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
                        // Print getter/setter keyword if applicable
                        match p.kind {
                            internal::PropertyKind::Get => self.write("get "),
                            internal::PropertyKind::Set => self.write("set "),
                            internal::PropertyKind::Init => {
                                // For methods, print async/generator prefixes
                                if p.method
                                    && let Expression::FunctionExpression(func) = &p.value
                                {
                                    if func.r#async {
                                        self.write("async ");
                                    }
                                    if func.generator {
                                        self.write("*");
                                    }
                                }
                            }
                        }

                        // For computed keys, use print_expression (preserves string quotes)
                        // For regular keys, use print_property_key (strips quotes from valid identifiers)
                        if p.computed {
                            self.write("[");
                            self.print_expression(&p.key);
                            self.write("]");
                        } else {
                            self.print_property_key(&p.key);
                        }

                        // Handle method vs getter/setter vs regular property
                        if matches!(
                            p.kind,
                            internal::PropertyKind::Get | internal::PropertyKind::Set
                        ) {
                            // Getter/setter: `get x() {}` or `set x(v) {}`
                            if let Expression::FunctionExpression(func) = &p.value {
                                self.print_function_expression_body(func);
                            }
                        } else if p.method {
                            // Method shorthand: `foo() {}` - print params and body directly
                            if let Expression::FunctionExpression(func) = &p.value {
                                self.print_function_expression_body(func);
                            } else {
                                // Fallback: shouldn't happen for well-formed AST
                                self.write(": ");
                                self.print_expression(&p.value);
                            }
                        } else if p.shorthand {
                            // Shorthand property: `{ prop }` - key is already printed
                            // Handle shorthand with default value: {a = 1}
                            if let Expression::AssignmentExpression(assign) = &p.value {
                                self.write(" = ");
                                self.print_expression(&assign.right);
                            } else if let Expression::AssignmentPattern(pattern) = &p.value {
                                self.write(" = ");
                                self.print_expression(&pattern.right);
                            }
                            // For regular shorthand, nothing extra to print
                        } else {
                            // Regular property: `key: value`
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
                // Uses binary search: O(log n + k)
                let mut has_line_comment = false;
                for comment in comments_in_range(self.comments, prop_end, obj.span.end) {
                    if is_same_line(self.source, prop_end, comment.span.start) {
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
                    for comment in comments_in_range(self.comments, prop_end, obj.span.end) {
                        if is_same_line(self.source, prop_end, comment.span.start)
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
            self.print_leading_comments(prev_end, obj.span.end, false);

            self.write_container_closing_indent();
        }

        self.write("}");
    }
}
