// Attribute formatting for Svelte elements
//
// Handles formatting of HTML attributes on elements, including:
// - Boolean attributes (e.g., `disabled`)
// - String attributes (e.g., `class="foo"`)
// - Attach tags (e.g., `{@attach expr}`)
// - Directives (on:, bind:, class:, style:, use:, transition:, animate:, let:)
// - Dynamic attributes ({...spread})
//
// Uses Doc IR for all formatting - build_*_doc methods are the canonical implementations.

use std::rc::Rc;

use crate::ast::internal;
use crate::printer::Printer;
use tsv_lang::doc::arena::DocId;
use tsv_lang::{SymbolResolver, SymbolToU32};

/// Normalize whitespace in a class attribute text value.
///
/// Matches prettier-plugin-svelte behavior for `class` attributes on HTML elements:
/// - Collapses multiple spaces/tabs to a single space (within each line)
/// - Trims trailing whitespace per line and at end of value
/// - Preserves leading whitespace (spaces before first non-ws char on each line)
/// - Preserves newlines as-is
///
/// `is_last_part`: when false, preserves one trailing space (for separation from
/// subsequent expression tags in mixed-content attributes like `class="a {expr}"`).
fn normalize_class_text(raw: &str, is_last_part: bool) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut had_non_ws = false;
    for (line_idx, line) in raw.split('\n').enumerate() {
        if line_idx > 0 {
            result.push('\n');
        }
        let mut in_leading = true;
        let mut pending_space = false;
        for ch in line.chars() {
            if ch == ' ' || ch == '\t' {
                if in_leading {
                    result.push(ch);
                } else {
                    pending_space = true;
                }
            } else {
                in_leading = false;
                had_non_ws = true;
                if pending_space {
                    result.push(' ');
                    pending_space = false;
                }
                result.push(ch);
            }
        }
        // Trailing whitespace per line is dropped (pending_space not flushed)
    }

    // For non-last parts with content, keep one trailing space for separation
    // from subsequent expression tags (e.g., class="text {expr}")
    // All-whitespace text (e.g., " ") passes through unchanged — the regex-based
    // approach in prettier-plugin-svelte only matches after non-ws characters.
    if !is_last_part && had_non_ws && raw.ends_with([' ', '\t']) {
        result.push(' ');
    }

    result
}

impl<'a> Printer<'a> {
    //
    // JS Comment Doc builders
    //

    /// Build a Doc for a leading JS comment (before content)
    ///
    /// Block comments: `/*content*/ ` (with trailing space)
    /// Line comments: `// content\n` (with hardline)
    pub(super) fn build_leading_js_comment_doc(&self, comment: &tsv_lang::Comment) -> DocId {
        let d = self.d();
        if comment.is_block {
            d.concat(&[
                d.text("/*"),
                d.text_owned(comment.content.clone()),
                d.text("*/ "),
            ])
        } else {
            // Content already includes the space after // (e.g., " comment" from "// comment")
            d.concat(&[
                d.text("//"),
                d.text_owned(comment.content.clone()),
                d.hardline(),
            ])
        }
    }

    /// Build a Doc for a trailing JS comment (after content)
    ///
    /// Block comments: ` /*content*/` (with leading space)
    /// Line comments: ` // content` (with leading space, no hardline)
    pub(super) fn build_trailing_js_comment_doc(&self, comment: &tsv_lang::Comment) -> DocId {
        let d = self.d();
        if comment.is_block {
            d.concat(&[
                d.text(" /*"),
                d.text_owned(comment.content.clone()),
                d.text("*/"),
            ])
        } else {
            // Content already includes the space after // (e.g., " comment" from "// comment")
            d.concat(&[d.text(" //"), d.text_owned(comment.content.clone())])
        }
    }

    //
    // Attribute node printing (unified via Doc)
    //

    /// Build a Doc for an attribute node (used for line wrapping calculations)
    ///
    /// `is_html`: true for HTML elements, enables class attribute whitespace normalization.
    pub(super) fn build_attribute_node_doc(
        &self,
        node: &internal::AttributeNode,
        is_html: bool,
    ) -> DocId {
        match node {
            internal::AttributeNode::Attribute(attr) => self.build_attribute_doc(attr, is_html),
            internal::AttributeNode::SpreadAttribute(spread) => {
                self.build_spread_attribute_doc(spread)
            }
            internal::AttributeNode::AttachTag(tag) => self.build_attach_tag_doc(tag),
            internal::AttributeNode::OnDirective(d) => self.build_on_directive_doc(d),
            internal::AttributeNode::BindDirective(d) => self.build_bind_directive_doc(d),
            internal::AttributeNode::ClassDirective(d) => self.build_class_directive_doc(d),
            internal::AttributeNode::StyleDirective(d) => self.build_style_directive_doc(d),
            internal::AttributeNode::UseDirective(d) => self.build_use_directive_doc(d),
            internal::AttributeNode::TransitionDirective(d) => {
                self.build_transition_directive_doc(d)
            }
            internal::AttributeNode::AnimateDirective(d) => self.build_animate_directive_doc(d),
            internal::AttributeNode::LetDirective(d) => self.build_let_directive_doc(d),
        }
    }

    //
    // Attribute Doc builders
    //

    /// Build a Doc for a single attribute (name="value" or name or {shorthand})
    ///
    /// `is_html`: true for HTML elements, enables class attribute whitespace normalization.
    pub(super) fn build_attribute_doc(&self, attr: &internal::Attribute, is_html: bool) -> DocId {
        let d = self.d();
        let name_sym = attr.name.to_u32();

        if let Some(value_parts) = &attr.value {
            // Check for shorthand: {name}
            if self.is_shorthand_attribute(attr.name, value_parts) {
                let sym = d.symbol(name_sym);
                return d.braces(sym);
            }

            // Normalize whitespace in class attributes on HTML elements
            let normalize_class = is_html && self.with_resolved_symbol(attr.name, |s| s == "class");

            let is_pure_expression = value_parts.len() == 1
                && matches!(value_parts[0], internal::AttributeValue::ExpressionTag(_));

            let mut parts = vec![d.symbol(name_sym)];

            if is_pure_expression {
                parts.push(d.text("="));
            } else {
                parts.push(d.text("=\""));
            }

            let last_idx = value_parts.len().saturating_sub(1);
            for (i, part) in value_parts.iter().enumerate() {
                if normalize_class {
                    parts.push(self.build_class_attribute_value_doc(part, i == last_idx));
                } else {
                    parts.push(self.build_attribute_value_doc(part));
                }
            }

            if !is_pure_expression {
                parts.push(d.text("\""));
            }

            d.concat(&parts)
        } else {
            // Boolean attribute
            d.symbol(name_sym)
        }
    }

    /// Build a Doc for an attribute value part
    fn build_attribute_value_doc(&self, value: &internal::AttributeValue) -> DocId {
        match value {
            internal::AttributeValue::Text(text) => self.build_attribute_text_doc(&text.raw),
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                self.build_attribute_expression_doc(expr_tag)
            }
        }
    }

    /// Build a Doc for a class attribute value part with whitespace normalization.
    ///
    /// Normalizes text content per prettier-plugin-svelte behavior:
    /// collapses multiple spaces, trims trailing whitespace per line.
    /// Expression tags are passed through unchanged.
    fn build_class_attribute_value_doc(
        &self,
        value: &internal::AttributeValue,
        is_last_part: bool,
    ) -> DocId {
        match value {
            internal::AttributeValue::Text(text) => {
                let normalized = normalize_class_text(&text.raw, is_last_part);
                self.build_attribute_text_doc(&normalized)
            }
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                self.build_attribute_expression_doc(expr_tag)
            }
        }
    }

    /// Build a Doc for an expression tag inside an attribute value.
    fn build_attribute_expression_doc(&self, expr_tag: &internal::ExpressionTag) -> DocId {
        self.build_expression_tag_doc(expr_tag)
    }

    /// Build a Doc for attribute text content, handling newlines as literallines.
    fn build_attribute_text_doc(&self, raw: &str) -> DocId {
        let d = self.d();
        if raw.contains('\n') {
            // Split at newlines, join with literalline to preserve literal newlines
            // and trigger will_break on the attribute group
            let line_docs: Vec<DocId> = raw
                .split('\n')
                .map(|part| d.text_owned(part.to_string()))
                .collect();
            let sep = d.literalline();
            d.join_doc(line_docs, sep)
        } else {
            d.text_owned(raw.to_string())
        }
    }

    /// Build a Doc for a spread attribute: `{...expr}`
    fn build_spread_attribute_doc(&self, spread: &internal::SpreadAttribute) -> DocId {
        self.build_braced_expression_doc(
            "{...",
            &spread.expression,
            spread.span.start + 4, // after `{...`
            spread.span.end,
        )
    }

    /// Build a Doc for an attach tag: `{@attach expr}`
    fn build_attach_tag_doc(&self, tag: &internal::AttachTag) -> DocId {
        self.build_braced_expression_doc(
            "{@attach ",
            &tag.expression,
            tag.span.start + 9, // after `{@attach `
            tag.span.end,
        )
    }

    /// Build a Doc for a braced expression with comments: `prefix expr }`
    ///
    /// Handles leading/trailing comments between the prefix/suffix and expression.
    fn build_braced_expression_doc(
        &self,
        prefix: &'static str,
        expr: &tsv_ts::ast::internal::Expression,
        comment_start: u32,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text(prefix)];

        // Leading comments (between prefix and expression)
        let expr_start = expr.span().start;
        for comment in tsv_lang::comments_in_range(self.comments, comment_start, expr_start) {
            parts.push(self.build_leading_js_comment_doc(comment));
        }

        // Expression doc with any nested comments
        parts.push(self.build_ts_expression_doc(expr));

        // Trailing comments (between expression and `}`)
        let expr_end = expr.span().end;
        for comment in tsv_lang::comments_in_range(self.comments, expr_end, span_end - 1) {
            parts.push(self.build_trailing_js_comment_doc(comment));
        }

        parts.push(d.text("}"));
        d.concat(&parts)
    }

    //
    // Directive Doc builders
    //

    /// Build a Doc for on:event directive
    fn build_on_directive_doc(&self, dir: &internal::OnDirective) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("on:"), d.text_owned(dir.name.clone())];
        parts.extend(self.build_modifiers_doc(&dir.modifiers));
        if let Some(expr) = &dir.expression {
            parts.extend(self.build_expression_doc_parts_with_span(expr, dir.expression_tag_span));
        }
        d.concat(&parts)
    }

    /// Build a Doc for bind:prop directive
    fn build_bind_directive_doc(&self, dir: &internal::BindDirective) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("bind:"), d.text_owned(dir.name.clone())];
        // Only include expression if not shorthand
        if !self.is_identifier_with_name(&dir.expression, &dir.name) {
            // bind: uses {getter, setter} syntax where SequenceExpression is bare (no parens)
            parts.extend(self.build_expression_doc_parts_with_span_for_bind(
                &dir.expression,
                dir.expression_tag_span,
            ));
        }
        d.concat(&parts)
    }

    /// Build a Doc for class:name directive
    fn build_class_directive_doc(&self, dir: &internal::ClassDirective) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("class:"), d.text_owned(dir.name.clone())];
        // Only include expression if not shorthand
        if !self.is_identifier_with_name(&dir.expression, &dir.name) {
            parts.extend(
                self.build_expression_doc_parts_with_span(&dir.expression, dir.expression_tag_span),
            );
        }
        d.concat(&parts)
    }

    /// Build a Doc for style:prop directive
    fn build_style_directive_doc(&self, dir: &internal::StyleDirective) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("style:"), d.text_owned(dir.name.clone())];
        parts.extend(self.build_modifiers_doc(&dir.modifiers));
        match &dir.value {
            internal::StyleDirectiveValue::True => {}
            internal::StyleDirectiveValue::ExpressionTag(tag) => {
                // Only include expression if not shorthand (style:color={color} → style:color)
                if !self.is_identifier_with_name(&tag.expression, &dir.name) {
                    parts.push(d.text("="));
                    parts.push(self.build_expression_tag_doc(tag));
                }
            }
            internal::StyleDirectiveValue::Parts(value_parts) => {
                parts.push(d.text("=\""));
                for part in value_parts {
                    parts.push(self.build_attribute_value_doc(part));
                }
                parts.push(d.text("\""));
            }
        }
        d.concat(&parts)
    }

    /// Build a Doc for use:action directive
    fn build_use_directive_doc(&self, dir: &internal::UseDirective) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("use:"), d.text_owned(dir.name.clone())];
        if let Some(expr) = &dir.expression {
            parts.extend(self.build_expression_doc_parts_with_span(expr, dir.expression_tag_span));
        }
        d.concat(&parts)
    }

    /// Build a Doc for transition/in/out directive
    fn build_transition_directive_doc(&self, dir: &internal::TransitionDirective) -> DocId {
        let d = self.d();
        let mut parts = vec![
            d.text(dir.direction.prefix_with_colon()),
            d.text_owned(dir.name.clone()),
        ];
        parts.extend(self.build_modifiers_doc(&dir.modifiers));
        if let Some(expr) = &dir.expression {
            parts.extend(self.build_expression_doc_parts_with_span(expr, dir.expression_tag_span));
        }
        d.concat(&parts)
    }

    /// Build a Doc for animate:name directive
    fn build_animate_directive_doc(&self, dir: &internal::AnimateDirective) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("animate:"), d.text_owned(dir.name.clone())];
        if let Some(expr) = &dir.expression {
            parts.extend(self.build_expression_doc_parts_with_span(expr, dir.expression_tag_span));
        }
        d.concat(&parts)
    }

    /// Build a Doc for let:name directive
    fn build_let_directive_doc(&self, dir: &internal::LetDirective) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("let:"), d.text_owned(dir.name.clone())];
        // Only include expression if not shorthand (let:foo={foo} → let:foo)
        if let Some(expr) = &dir.expression
            && !self.is_identifier_with_name(expr, &dir.name)
        {
            parts.extend(self.build_expression_doc_parts_with_span(expr, dir.expression_tag_span));
        }
        d.concat(&parts)
    }

    //
    // Shared helpers
    //

    /// Build Doc parts for modifiers: `|mod1|mod2`
    fn build_modifiers_doc(&self, modifiers: &[String]) -> Vec<DocId> {
        modifiers
            .iter()
            .flat_map(|m| vec![self.d().text("|"), self.d().text_owned(m.clone())])
            .collect()
    }

    /// Build expression doc for attribute context (embedded expression).
    ///
    /// Sets `LayoutMode::Embedded` so binary expressions use ContinuationIndent style.
    /// Assignment expressions get wrapped in parens: `prop={(a = b)}`.
    fn build_expression_doc_for_attribute(
        &self,
        expr: &tsv_ts::ast::internal::Expression,
    ) -> DocId {
        let d = self.d();
        let embedded = tsv_lang::EmbedContext {
            mode: tsv_lang::LayoutMode::Embedded,
            ..tsv_lang::EmbedContext::default()
        };

        // Assignment expressions need parens in attribute values: prop={(a = b)}
        if let tsv_ts::ast::internal::Expression::AssignmentExpression(_) = expr {
            let inner = tsv_ts::build_expression_doc_with_comments(
                d,
                expr,
                self.source,
                Rc::clone(&self.interner),
                &embedded,
                self.comments,
                &self.line_breaks,
                tsv_ts::TsContext::Svelte,
            );
            return d.parens(inner);
        }

        tsv_ts::build_expression_doc_with_comments(
            d,
            expr,
            self.source,
            Rc::clone(&self.interner),
            &embedded,
            self.comments,
            &self.line_breaks,
            tsv_ts::TsContext::Svelte,
        )
    }

    /// Build Doc parts for an expression with optional span for comment lookup: `={expr}`
    ///
    /// When the expression is too long, uses block structure:
    /// - Flat: `={expr}`
    /// - Broken: `={\n\t\texpr\n\t}`
    ///
    /// For binary expressions, uses continuation indent when broken:
    /// - Flat: `={a && b && c}`
    /// - Broken: `={\n\t\ta &&\n\t\t\tb &&\n\t\t\tc\n\t}`
    fn build_expression_doc_parts_with_span(
        &self,
        expr: &tsv_ts::ast::internal::Expression,
        tag_span: Option<tsv_lang::Span>,
    ) -> Vec<DocId> {
        let expr_content = self.build_expression_content_with_comments(expr, tag_span);

        // For expressions with internal group structure, keep them hugged with the braces.
        // Prettier lets their internal structure handle wrapping.
        //
        // Arrow functions:
        //   Flat: ={() => fn()}
        //   Broken: ={(() =>\n\t\tfn())}
        //
        // Object literals (e.g., transition:fade={{...}}):
        //   Flat: ={{duration: 300, delay: 100}}
        //   Broken: ={{\n\t\tduration: 300,\n\t\tdelay: 100,\n\t}}
        //   Note: ={{ stays together, object properties wrap internally
        //
        // Ternary expressions:
        //   Flat: ={cond ? a : b}
        //   Broken: ={cond\n\t\t? aLong\n\t\t: bLong}
        //
        // Call expressions:
        //   Flat: ={fn(a, b, c)}
        //   Broken: ={fn(\n\t\ta,\n\t\tb,\n\t\tc,\n\t)}
        //
        // For other expressions, use block structure when broken:
        //   Flat: ={expr}
        //   Broken: ={\n\t\texpr\n\t}
        let is_hugged = matches!(
            expr,
            tsv_ts::ast::internal::Expression::ArrowFunctionExpression(_)
                | tsv_ts::ast::internal::Expression::FunctionExpression(_)
                | tsv_ts::ast::internal::Expression::ObjectExpression(_)
                | tsv_ts::ast::internal::Expression::ConditionalExpression(_)
                | tsv_ts::ast::internal::Expression::CallExpression(_)
                | tsv_ts::ast::internal::Expression::NewExpression(_)
                | tsv_ts::ast::internal::Expression::ArrayExpression(_)
                | tsv_ts::ast::internal::Expression::BinaryExpression(_)
        );

        let d = self.d();
        let inner = if is_hugged {
            // Hugged: the expression's internal doc handles wrapping
            let content = d.concat(&expr_content);
            d.concat(&[d.text("{"), content, d.text("}")])
        } else {
            // Block structure for other expressions
            self.wrap_in_block_structure(expr_content)
        };

        vec![d.text("="), inner]
    }

    /// Build expression content with leading/trailing comments
    ///
    /// Returns a Vec<DocId> containing: leading comments + expression doc + trailing comments
    fn build_expression_content_with_comments(
        &self,
        expr: &tsv_ts::ast::internal::Expression,
        tag_span: Option<tsv_lang::Span>,
    ) -> Vec<DocId> {
        // Collect leading comments
        let mut leading_comments = Vec::new();
        if let Some(span) = tag_span {
            let expr_start = expr.span().start;
            for comment in tsv_lang::comments_in_range(self.comments, span.start + 1, expr_start) {
                leading_comments.push(self.build_leading_js_comment_doc(comment));
            }
        }

        let expr_doc = self.build_expression_doc_for_attribute(expr);

        // Collect trailing comments
        let mut trailing_comments = Vec::new();
        if let Some(span) = tag_span {
            let expr_end = expr.span().end;
            for comment in tsv_lang::comments_in_range(self.comments, expr_end, span.end - 1) {
                trailing_comments.push(self.build_trailing_js_comment_doc(comment));
            }
        }

        // Build the expression content (leading comments + expr + trailing comments)
        let mut expr_content = leading_comments;
        expr_content.push(expr_doc);
        expr_content.extend(trailing_comments);
        expr_content
    }

    /// Wrap expression content in block structure: `{\n\texpr\n}`
    fn wrap_in_block_structure(&self, expr_content: Vec<DocId>) -> DocId {
        let d = self.d();
        let content = d.concat(&expr_content);
        let softline = d.softline();
        let inner = d.concat(&[softline, content]);
        let indented = d.indent(inner);
        let softline2 = d.softline();
        let concat = d.concat(&[d.text("{"), indented, softline2, d.text("}")]);
        d.group(concat)
    }

    /// Build Doc parts for bind directive expressions: `={expr}`
    ///
    /// Handles the special `bind:prop={getter, setter}` syntax where SequenceExpression
    /// is printed without parentheses (the "function bindings" syntax in Svelte 5.9+).
    ///
    /// Unlike other directives, bind: always uses block structure for expressions
    /// that need to wrap (Prettier behavior).
    ///
    /// When the sequence contains multiline expressions (e.g., arrow with block body),
    /// formats as:
    /// ```svelte
    /// bind:value={
    ///     () => a,
    ///     (v) => {
    ///         a = v;
    ///     }
    /// }
    /// ```
    fn build_expression_doc_parts_with_span_for_bind(
        &self,
        expr: &tsv_ts::ast::internal::Expression,
        tag_span: Option<tsv_lang::Span>,
    ) -> Vec<DocId> {
        let d = self.d();
        // For SequenceExpression, use the bare (no parens) version for getter/setter syntax
        if let tsv_ts::ast::internal::Expression::SequenceExpression(seq) = expr {
            let len = seq.expressions.len();

            // Build items: each expression with trailing comma (except last)
            let items: Vec<DocId> = seq
                .expressions
                .iter()
                .enumerate()
                .map(|(i, sub_expr)| {
                    let expr_doc = self.build_ts_expression_doc(sub_expr);
                    if i < len - 1 {
                        let comma = d.text(",");
                        d.concat(&[expr_doc, comma])
                    } else {
                        expr_doc
                    }
                })
                .collect();

            // Join with line() - becomes " " when flat, "\n" when broken
            let line = d.line();
            let items_doc = d.join_doc(items, line);

            // Use group/indent structure that expands when content is multiline:
            // Flat: ={getter, setter}
            // Broken: ={\n\tgetter,\n\tsetter\n}
            let indent_softline = d.indent_softline(items_doc);
            let softline = d.softline();
            let concat = d.concat(&[d.text("{"), indent_softline, softline, d.text("}")]);
            let inner = d.group(concat);

            return vec![d.text("="), inner];
        }

        // For bind: directives, BinaryExpression should use block structure (not hugging).
        // This matches Prettier's behavior where bind: uses `={\n\texpr\n}` format.
        if let tsv_ts::ast::internal::Expression::BinaryExpression(_) = expr {
            return self.build_expression_doc_parts_with_span_block_structure(expr, tag_span);
        }

        // For other expressions, use the standard method
        self.build_expression_doc_parts_with_span(expr, tag_span)
    }

    /// Build Doc parts using block structure: `={\n\texpr\n}`
    ///
    /// Used for bind: directive expressions where Prettier always uses this format.
    fn build_expression_doc_parts_with_span_block_structure(
        &self,
        expr: &tsv_ts::ast::internal::Expression,
        tag_span: Option<tsv_lang::Span>,
    ) -> Vec<DocId> {
        let expr_content = self.build_expression_content_with_comments(expr, tag_span);
        vec![
            self.d().text("="),
            self.wrap_in_block_structure(expr_content),
        ]
    }

    /// Build a Doc for an expression tag: `{expr}`
    ///
    /// For binary expressions, uses continuation indent so wrapped lines are indented
    /// relative to the opening `{`:
    /// ```text
    /// {condA &&
    ///   condB &&
    ///   condC}
    /// ```
    pub(super) fn build_expression_tag_doc(&self, tag: &internal::ExpressionTag) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("{")];

        // Add leading comments between { and expression
        let expr_start = tag.expression.span().start;
        for comment in tsv_lang::comments_in_range(self.comments, tag.span.start + 1, expr_start) {
            if comment.is_block {
                parts.push(d.text_owned(format!("/*{}*/ ", comment.content)));
            }
        }

        parts.push(self.build_expression_doc_for_attribute(&tag.expression));

        // Add trailing comments (block comments only in expression tags)
        let expr_end = tag.expression.span().end;
        for comment in tsv_lang::comments_in_range(self.comments, expr_end, tag.span.end - 1) {
            if comment.is_block {
                parts.push(d.text_owned(format!(" /*{}*/", comment.content)));
            }
        }

        parts.push(d.text("}"));
        d.concat(&parts)
    }

    /// Check if an attribute is a shorthand: {name} where value is ExpressionTag(Identifier(name))
    fn is_shorthand_attribute(
        &self,
        attr_name: string_interner::DefaultSymbol,
        value_parts: &[internal::AttributeValue],
    ) -> bool {
        // Must be exactly one value part
        if value_parts.len() != 1 {
            return false;
        }

        // Must be an ExpressionTag
        let internal::AttributeValue::ExpressionTag(expr_tag) = &value_parts[0] else {
            return false;
        };

        // Must contain an Identifier expression
        let tsv_ts::ast::internal::Expression::Identifier(ident) = &expr_tag.expression else {
            return false;
        };

        // The identifier name must match the attribute name
        ident.name == attr_name
    }

    /// Check if expression is an identifier with the given name
    fn is_identifier_with_name(
        &self,
        expr: &tsv_ts::ast::internal::Expression,
        name: &str,
    ) -> bool {
        use tsv_ts::ast::internal::Expression;
        if let Expression::Identifier(id) = expr {
            self.resolve_symbol(id.name) == name
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_class_text;

    #[test]
    fn collapses_runs_and_trims_trailing_per_line() {
        assert_eq!(normalize_class_text("a   b", true), "a b");
        // Leading whitespace preserved, trailing dropped.
        assert_eq!(normalize_class_text("  a b  ", true), "  a b");
        // Newlines kept; per-line leading preserved, intra-line runs collapsed.
        assert_eq!(normalize_class_text("a  b\n  c  d", true), "a b\n  c d");
    }

    #[test]
    fn last_part_flag_controls_separator_space() {
        // Non-last part with content keeps one trailing space (separates from `{expr}`).
        assert_eq!(normalize_class_text("text ", false), "text ");
        // Last part drops the trailing space.
        assert_eq!(normalize_class_text("text ", true), "text");
    }

    #[test]
    fn all_whitespace_passes_through() {
        // No non-whitespace ⇒ the separator-space rule doesn't apply.
        assert_eq!(normalize_class_text(" ", false), " ");
        assert_eq!(normalize_class_text("", true), "");
    }
}
