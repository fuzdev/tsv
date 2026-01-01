// Object expression printing for TypeScript
//
// Handles printing of object expressions with:
// - Width-based wrapping via doc-builder
// - Comment preservation (block and line comments)
// - Property shorthand detection
// - String key normalization (unquote valid identifiers)
// - Blank line preservation between properties

use super::Printer;
use super::expression_stringifier::{escape_single_quote_string, is_valid_js_identifier};
use crate::ast::internal::{self, Expression, Literal, LiteralValue};
use tsv_lang::SymbolResolver;
use tsv_lang::comments_in_range;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::{has_blank_line_between, has_newline_between, is_same_line};

impl<'a> Printer<'a> {
    /// Build a Doc for an object expression
    ///
    /// Handles comments between properties, blank line preservation, and trailing comments.
    pub(super) fn build_object_doc(&self, obj: &internal::ObjectExpression) -> Doc {
        // Check for comments inside the object
        let has_comments = self.has_comments_between(obj.span.start, obj.span.end);

        // Check if object contains line comments (force multiline)
        let has_line_comments = self.has_line_comments_between(obj.span.start, obj.span.end);

        if obj.properties.is_empty() {
            // Handle empty object with comments
            if has_comments {
                let obj_start = obj.span.start + 1; // After '{'
                let obj_end = obj.span.end.saturating_sub(1); // Before '}'
                let mut comment_parts = Vec::new();
                for comment in comments_in_range(self.comments, obj_start, obj_end) {
                    comment_parts.push(self.build_comment_doc(comment));
                    if !comment.is_block {
                        comment_parts.push(doc::hardline());
                    }
                }
                return doc::concat(vec![
                    doc::text("{"),
                    doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::concat(comment_parts),
                    ])),
                    doc::hardline(),
                    doc::text("}"),
                ]);
            }
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

        // Decide the formatting strategy
        let force_multiline = has_line_comments || has_source_newline || has_multiline;

        if has_comments || force_multiline {
            // Comment-aware path
            // Use hardlines when force_multiline, use line() when only has_comments
            // This allows inline objects with block comments to stay inline if they fit
            let mut parts = Vec::new();
            let mut prev_end = obj.span.start + 1; // After opening brace

            for (i, prop) in obj.properties.iter().enumerate() {
                let prop_start = prop.span().start;
                let is_first = i == 0;

                // Get comments between previous position and this property
                // Filter out trailing same-line comments from the previous property
                let all_comments: Vec<_> =
                    comments_in_range(self.comments, prev_end, prop_start).collect();
                let comments: Vec<_> = if !is_first {
                    all_comments
                        .iter()
                        .filter(|c| !is_same_line(self.source, prev_end, c.span.start))
                        .copied()
                        .collect()
                } else {
                    all_comments
                };

                // For non-first properties, add separator
                if !is_first {
                    if force_multiline {
                        // Must break: check for blank line preservation
                        let check_pos = if comments.is_empty() {
                            prop_start
                        } else {
                            comments[0].span.start
                        };
                        if has_blank_line_between(self.source, prev_end, check_pos) {
                            parts.push(doc::literalline());
                        }
                        parts.push(doc::hardline());
                    } else {
                        // May stay inline: use line() for group-based breaking
                        parts.push(doc::line());
                    }
                }

                // Process comments before this property
                let mut last_pos = prev_end;
                for (j, comment) in comments.iter().enumerate() {
                    // For subsequent comments, check for blank lines between them
                    if force_multiline
                        && j > 0
                        && has_blank_line_between(self.source, last_pos, comment.span.start)
                    {
                        parts.push(doc::literalline());
                        parts.push(doc::hardline());
                    }

                    parts.push(self.build_comment_doc(comment));
                    if !comment.is_block {
                        // Line comments need a hardline after
                        parts.push(doc::hardline());
                    } else if force_multiline
                        && !is_same_line(self.source, comment.span.end, prop_start)
                    {
                        // Block comment on its own line - hardline after (only when forcing multiline)
                        parts.push(doc::hardline());
                    } else {
                        // Block comment on same line as property - space after
                        parts.push(doc::text(" "));
                    }
                    last_pos = comment.span.end;
                }

                // Check for blank line after last comment (before property)
                if force_multiline
                    && !comments.is_empty()
                    && has_blank_line_between(self.source, last_pos, prop_start)
                {
                    parts.push(doc::literalline());
                    parts.push(doc::hardline());
                }

                // Build property doc
                let prop_doc = self.build_object_property_doc(prop);
                parts.push(prop_doc);

                // Handle trailing inline comments on same line after property
                // Block comments go before comma, line comments go after comma
                let prop_end = prop.value_end();
                let upper_bound = obj
                    .properties
                    .get(i + 1)
                    .map_or(obj.span.end, |next| next.span().start);

                // Collect same-line trailing comments
                let trailing: Vec<_> = comments_in_range(self.comments, prop_end, upper_bound)
                    .filter(|c| is_same_line(self.source, prop_end, c.span.start))
                    .collect();

                // Block comments go before comma
                for comment in trailing.iter().filter(|c| c.is_block) {
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }

                // Add comma
                if i < obj.properties.len() - 1 {
                    parts.push(doc::text(","));
                } else {
                    // Last property: trailing comma only when broken
                    parts.push(doc::trailing_comma());
                }

                // Line comments go after comma
                for comment in trailing.iter().filter(|c| !c.is_block) {
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }

                prev_end = prop.value_end();
            }

            // Handle trailing comments before closing brace
            let closing_brace_pos = obj.span.end - 1;
            for comment in comments_in_range(self.comments, prev_end, closing_brace_pos) {
                // Skip same-line comments (already handled above)
                if is_same_line(self.source, prev_end, comment.span.start) {
                    continue;
                }
                if force_multiline {
                    parts.push(doc::hardline());
                } else {
                    parts.push(doc::line());
                }
                parts.push(self.build_comment_doc(comment));
            }

            if force_multiline {
                // Forced multiline - use hardlines for predictable formatting
                let inner = doc::concat(vec![doc::hardline(), doc::concat(parts)]);
                let (indented_content, closing_line) =
                    self.wrap_with_decl_indent(inner, doc::hardline());

                doc::concat(vec![
                    doc::text("{"),
                    indented_content,
                    closing_line,
                    doc::text("}"),
                ])
            } else {
                // May stay inline - use group with softlines for width-based breaking
                let inner = doc::concat(vec![doc::softline(), doc::concat(parts)]);
                let (indented_content, closing_line) =
                    self.wrap_with_decl_indent(inner, doc::softline());

                doc::group(doc::concat(vec![
                    doc::text("{"),
                    indented_content,
                    closing_line,
                    doc::text("}"),
                ]))
            }
        } else {
            // No comments, no forced multiline: use width-based wrapping with soft lines
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
                    // Blank line preservation
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
                    let next_prop = &obj.properties[i + 1];
                    let curr_end = prop.value_end();
                    let next_has_blank =
                        has_blank_line_between(self.source, curr_end, next_prop.span().start);

                    if !next_has_blank {
                        parts.push(doc::line());
                    }
                } else {
                    // Last property: trailing comma only when broken
                    parts.push(doc::trailing_comma());
                }
            }

            // Width-based wrapping
            let inner = doc::concat(vec![doc::softline(), doc::concat(parts)]);
            let (indented_content, closing_line) =
                self.wrap_with_decl_indent(inner, doc::softline());

            doc::group(doc::concat(vec![
                doc::text("{"),
                indented_content,
                closing_line,
                doc::text("}"),
            ]))
        }
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
            // Regular property: check for comments between key and value
            // Find colon position and check for comments
            let colon_pos = self.find_colon_after(prop.key.span().end);
            let value_start = prop.value.span().start;
            let colon_comments: Vec<_> =
                comments_in_range(self.comments, colon_pos + 1, value_start).collect();

            if colon_comments.is_empty() {
                // No comments: use unified assignment layout
                let key_width = self.estimate_key_width(&prop.key, prop.computed);
                let context = super::assignment::LayoutContext::for_property(
                    key_width,
                    self.config.tab_width,
                );
                self.build_assignment_layout(key_doc, ":", &prop.value, context)
            } else {
                // Comments between colon and value: build manually to preserve them
                let mut parts = vec![key_doc, doc::text(": ")];
                for comment in &colon_comments {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(doc::text(" "));
                }
                parts.push(self.build_expression_doc(&prop.value));
                doc::concat(parts)
            }
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
}
