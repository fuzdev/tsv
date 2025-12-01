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
use crate::ast::internal::{self, Expression, Literal, LiteralValue};
use tsv_lang::SymbolResolver;
use tsv_lang::comments_in_range;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::{
    StringFormatOptions, format_string_literal, has_blank_line_between, has_newline_between,
    is_same_line,
};

/// Check if a string is a valid JavaScript identifier
///
/// Valid identifiers:
/// - Start with a letter, underscore, or dollar sign
/// - Contain only letters, digits, underscores, or dollar signs
/// - Can be reserved words (prettier outputs them without quotes)
pub(super) fn is_valid_js_identifier(s: &str) -> bool {
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
pub(super) fn escape_single_quote_string(s: &str) -> String {
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

    /// Print an empty object that contains only comments
    ///
    /// Prettier always expands empty objects with comments to multiline:
    /// `{/* comment */}` → `{\n\t/* comment */\n}`
    fn print_empty_object_with_comments(&mut self, obj: &internal::ObjectExpression) {
        self.write("{\n");
        self.indent_level += 1 + self.declaration_indent_depth;

        // Print all comments inside the object
        // Unlike print_leading_comments, we print ALL comments here (including same-line ones)
        // because there are no "statements" to attach trailing comments to
        // Uses binary search: O(log n + k)
        let inner_start = obj.span.start + 1; // After '{'
        let inner_end = obj.span.end - 1; // Before '}'

        for comment in comments_in_range(self.comments, inner_start, inner_end) {
            self.write_indent();
            self.print_comment(comment);
            self.write("\n");
        }

        self.indent_level -= 1;
        self.write_indent();
        self.indent_level -= self.declaration_indent_depth;
        self.write("}");
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
                parts.push(doc::if_break(doc::text(","), doc::text("")));
            }
        }

        // Build the full object doc
        // Force multiline if source had newlines OR content has multiline strings
        let line_type = if has_source_newline || has_multiline {
            doc::hardline()
        } else {
            doc::softline()
        };

        // In multi-declarator context, apply extra indentation:
        // - Properties get +1 extra indent (total 2 from declaration)
        // - Closing line gets +1 extra indent (for closing brace alignment)
        let inner = doc::concat(vec![line_type.clone(), doc::concat(parts)]);
        let (indented_content, closing_line) = if self.declaration_indent_depth > 0 {
            (doc::indent(doc::indent(inner)), doc::indent(line_type))
        } else {
            (doc::indent(inner), line_type)
        };

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
                let func_doc = self.build_function_doc(func);
                doc::concat(vec![key_doc, func_doc])
            } else {
                key_doc
            }
        } else if prop.method {
            // Method shorthand: `foo() {}` - print key followed by params and body
            if let Expression::FunctionExpression(func) = &prop.value {
                let func_doc = self.build_function_doc(func);
                doc::concat(vec![key_doc, func_doc])
            } else {
                // Fallback for malformed AST
                let value_doc = self.build_expression_doc(&prop.value);
                doc::concat(vec![key_doc, doc::text(": "), value_doc])
            }
        } else if prop.shorthand {
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
    pub(super) fn build_property_key_doc(&self, key: &Expression) -> Doc {
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
                        // Method shorthand: `foo() {}`
                        let value_str = self.expression_to_string(&p.value);
                        parts.push(format!("{key_str}{value_str}"));
                    } else if p.shorthand {
                        parts.push(key_str);
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

    /// Convert an expression to a string (for inline object building)
    pub(super) fn expression_to_string(&self, expr: &Expression) -> String {
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
                LiteralValue::Undefined => "undefined".to_string(),
            },
            Expression::Identifier(id) => self.resolve_symbol(id.name),
            Expression::ObjectExpression(obj) => {
                // Recursively build nested object inline
                let mut parts = Vec::new();
                parts.push("{".to_string());

                for (i, prop) in obj.properties.iter().enumerate() {
                    match prop {
                        internal::ObjectProperty::Property(p) => {
                            // For computed keys, use expression_to_string (preserves string quotes)
                            // For regular keys, use property_key_to_string (strips quotes)
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
                                // Method shorthand: `foo() {}`
                                let value_str = self.expression_to_string(&p.value);
                                parts.push(format!("{key_str}{value_str}"));
                            } else if p.shorthand {
                                parts.push(key_str);
                            } else {
                                let value_str = self.expression_to_string(&p.value);
                                parts.push(format!("{key_str}: {value_str}"));
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
            Expression::UpdateExpression(update) => {
                if update.prefix {
                    format!(
                        "{}{}",
                        update.operator.as_str(),
                        self.expression_to_string(&update.argument)
                    )
                } else {
                    format!(
                        "{}{}",
                        self.expression_to_string(&update.argument),
                        update.operator.as_str()
                    )
                }
            }
            Expression::BinaryExpression(binary) => {
                let left = self.expression_to_string(&binary.left);
                let right = self.expression_to_string(&binary.right);

                // Wrap operands in parens if needed
                let left_str = if let Expression::BinaryExpression(child) = binary.left.as_ref() {
                    if super::operators::needs_parens_for_clarity(child, binary.operator, false) {
                        format!("({left})")
                    } else {
                        left
                    }
                } else {
                    left
                };

                let right_str = if let Expression::BinaryExpression(child) = binary.right.as_ref() {
                    if super::operators::needs_parens_for_clarity(child, binary.operator, true) {
                        format!("({right})")
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
                    format!("{obj}{opt}[{prop}]")
                } else {
                    let dot = if member.optional { "?." } else { "." };
                    format!("{obj}{dot}{prop}")
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
            Expression::TemplateLiteral(template) => {
                let mut result = String::from("`");
                for (i, quasi) in template.quasis.iter().enumerate() {
                    result.push_str(&quasi.raw);
                    if i < template.expressions.len() {
                        result.push_str("${");
                        result.push_str(&self.expression_to_string(&template.expressions[i]));
                        result.push('}');
                    }
                }
                result.push('`');
                result
            }
            Expression::TaggedTemplateExpression(tagged) => {
                let tag = self.expression_to_string(&tagged.tag);
                let quasi =
                    self.expression_to_string(&Expression::TemplateLiteral(tagged.quasi.clone()));
                format!("{tag}{quasi}")
            }
            Expression::NewExpression(new_expr) => {
                let callee = self.expression_to_string(&new_expr.callee);
                let args: Vec<String> = new_expr
                    .arguments
                    .iter()
                    .map(|arg| self.expression_to_string(arg))
                    .collect();
                format!("new {}({})", callee, args.join(", "))
            }
            Expression::FunctionExpression(func) => {
                // Function expressions: () { return ...; }
                // params can be patterns, so extract from source instead of resolving names
                let params: Vec<String> = func
                    .params
                    .iter()
                    .map(|p| self.expression_to_string(p))
                    .collect();
                // For inline string, we just extract body from source
                let body_start = func.body.span.start as usize;
                let body_end = func.body.span.end as usize;
                let body_str = &self.source[body_start..body_end];
                format!("({}) {}", params.join(", "), body_str)
            }
            Expression::AwaitExpression(await_expr) => {
                format!("await {}", self.expression_to_string(&await_expr.argument))
            }
            Expression::SequenceExpression(seq) => {
                let inner = seq
                    .expressions
                    .iter()
                    .map(|e| self.expression_to_string(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({inner})")
            }
            Expression::RegexLiteral(regex) => {
                format!("/{}/{}", regex.pattern, regex.flags)
            }
            Expression::Super(_) => "super".to_string(),
            Expression::AssignmentExpression(assign) => {
                format!(
                    "{} {} {}",
                    self.expression_to_string(&assign.left),
                    assign.operator.as_str(),
                    self.expression_to_string(&assign.right)
                )
            }
            Expression::ObjectPattern(obj) => {
                let props: Vec<String> = obj
                    .properties
                    .iter()
                    .map(|p| self.object_pattern_property_to_string(p))
                    .collect();
                format!("{{{}}}", props.join(", "))
            }
            Expression::ArrayPattern(arr) => {
                let elems: Vec<String> = arr
                    .elements
                    .iter()
                    .map(|e| match e {
                        Some(expr) => self.expression_to_string(expr),
                        None => String::new(),
                    })
                    .collect();
                format!("[{}]", elems.join(", "))
            }
            Expression::AssignmentPattern(pattern) => {
                format!(
                    "{} = {}",
                    self.expression_to_string(&pattern.left),
                    self.expression_to_string(&pattern.right)
                )
            }
            Expression::RestElement(rest) => {
                format!("...{}", self.expression_to_string(&rest.argument))
            }
        }
    }

    /// Convert an object pattern property to a string
    fn object_pattern_property_to_string(&self, prop: &internal::ObjectPatternProperty) -> String {
        match prop {
            internal::ObjectPatternProperty::Property(p) => {
                if p.shorthand {
                    // For shorthand with default value, the value is an AssignmentPattern
                    // For shorthand without default, key and value are the same identifier
                    match &p.value {
                        Expression::AssignmentPattern(pattern) => {
                            // {a = 1} - shorthand with default
                            format!(
                                "{} = {}",
                                self.expression_to_string(&p.key),
                                self.expression_to_string(&pattern.right)
                            )
                        }
                        _ => self.expression_to_string(&p.key),
                    }
                } else {
                    // {a: x} or {a: x = 1} or {[key]: x}
                    // For regular keys, use property_key_to_string to normalize string keys to identifiers
                    let key_str = if p.computed {
                        format!("[{}]", self.expression_to_string(&p.key))
                    } else {
                        self.property_key_to_string(&p.key)
                    };
                    format!("{}: {}", key_str, self.expression_to_string(&p.value))
                }
            }
            internal::ObjectPatternProperty::RestElement(r) => {
                format!("...{}", self.expression_to_string(&r.argument))
            }
        }
    }

    /// Convert a property key expression to a string
    ///
    /// String keys that are valid identifiers are output without quotes.
    pub(super) fn property_key_to_string(&self, key: &Expression) -> String {
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
            // In multi-declarator context, add extra indent for properties
            self.indent_level += 1 + self.declaration_indent_depth;

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
                            internal::PropertyKind::Init => {}
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
                                self.print_function_expression(func);
                            }
                        } else if p.method {
                            // Method shorthand: `foo() {}` - print params and body directly
                            if let Expression::FunctionExpression(func) = &p.value {
                                self.print_function_expression(func);
                            } else {
                                // Fallback: shouldn't happen for well-formed AST
                                self.write(": ");
                                self.print_expression(&p.value);
                            }
                        } else if !p.shorthand {
                            // Regular property: `key: value`
                            self.write(": ");
                            self.print_expression(&p.value);
                        }
                        // Shorthand property: `{ prop }` - key is already printed
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
            self.print_leading_comments(prev_end, obj.span.end);

            // Restore indent: first go to closing brace level (with declaration_indent_depth)
            self.indent_level -= 1;
            self.write_indent();
            // Then restore fully
            self.indent_level -= self.declaration_indent_depth;
        }

        self.write("}");
    }

    /// Convert an arrow function to a string (for inline building)
    pub(super) fn arrow_function_to_string(
        &self,
        arrow: &internal::ArrowFunctionExpression,
    ) -> String {
        let mut result = String::new();

        // Parameters (can be patterns, so use expression_to_string)
        result.push('(');
        for (i, param) in arrow.params.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&self.expression_to_string(param));
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
