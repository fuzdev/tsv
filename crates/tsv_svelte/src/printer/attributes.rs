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
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    // =========================================================================
    // Doc writing helper
    // =========================================================================

    /// Write a Doc to the output buffer at the current position
    ///
    /// Handles column tracking and indent resolution for proper line wrapping.
    fn write_doc(&mut self, doc: &Doc) {
        let current_col = self.buffer.current_column(self.config.tab_width);
        let output = {
            let interner = self.interner.borrow();
            doc::print_doc_with_indent_resolved(
                doc,
                &self.config,
                current_col,
                self.indent_level,
                &*interner,
            )
        };
        self.write(&output);
    }

    // =========================================================================
    // JS Comment Doc builders
    // =========================================================================

    /// Build a Doc for a leading JS comment (before content)
    ///
    /// Block comments: `/*content*/ ` (with trailing space)
    /// Line comments: `// content\n` (with hardline)
    fn build_leading_js_comment_doc(comment: &tsv_lang::Comment) -> Doc {
        if comment.is_block {
            doc::concat(vec![
                doc::text("/*"),
                doc::text_owned(comment.content.clone()),
                doc::text("*/ "),
            ])
        } else {
            doc::concat(vec![
                doc::text("// "),
                doc::text_owned(comment.content.clone()),
                doc::hardline(),
            ])
        }
    }

    /// Build a Doc for a trailing JS comment (after content)
    ///
    /// Block comments: ` /*content*/` (with leading space)
    /// Line comments: ` // content` (with leading space, no hardline)
    fn build_trailing_js_comment_doc(comment: &tsv_lang::Comment) -> Doc {
        if comment.is_block {
            doc::concat(vec![
                doc::text(" /*"),
                doc::text_owned(comment.content.clone()),
                doc::text("*/"),
            ])
        } else {
            doc::concat(vec![
                doc::text(" // "),
                doc::text_owned(comment.content.clone()),
            ])
        }
    }

    // =========================================================================
    // Attribute node printing (unified via Doc)
    // =========================================================================

    /// Format an attribute node (attribute, attach tag, directive, etc.)
    ///
    /// All formatting goes through the Doc IR for consistency.
    pub(super) fn print_attribute_node(&mut self, node: &internal::AttributeNode) {
        let doc = self.build_attribute_node_doc(node);
        self.write_doc(&doc);
    }

    /// Build a Doc for an attribute node (used for line wrapping calculations)
    pub(super) fn build_attribute_node_doc(&self, node: &internal::AttributeNode) -> Doc {
        match node {
            internal::AttributeNode::Attribute(attr) => self.build_attribute_doc(attr),
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

    // =========================================================================
    // Attribute Doc builders
    // =========================================================================

    /// Build a Doc for a single attribute (name="value" or name or {shorthand})
    pub(super) fn build_attribute_doc(&self, attr: &internal::Attribute) -> Doc {
        let name = self.resolve_symbol(attr.name);

        if let Some(value_parts) = &attr.value {
            // Check for shorthand: {name}
            if self.is_shorthand_attribute(attr.name, value_parts) {
                return doc::braces(doc::text_owned(name));
            }

            let is_pure_expression = value_parts.len() == 1
                && matches!(value_parts[0], internal::AttributeValue::ExpressionTag(_));

            let mut parts = vec![doc::text_owned(name)];

            if is_pure_expression {
                parts.push(doc::text("="));
            } else {
                parts.push(doc::text("=\""));
            }

            for part in value_parts {
                parts.push(self.build_attribute_value_doc(part));
            }

            if !is_pure_expression {
                parts.push(doc::text("\""));
            }

            doc::concat(parts)
        } else {
            // Boolean attribute
            doc::text_owned(name)
        }
    }

    /// Build a Doc for an attribute value part
    fn build_attribute_value_doc(&self, value: &internal::AttributeValue) -> Doc {
        match value {
            internal::AttributeValue::Text(text) => doc::text_owned(text.raw.clone()),
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                self.build_expression_tag_doc(expr_tag)
            }
        }
    }

    /// Build a Doc for a spread attribute: `{...expr}`
    fn build_spread_attribute_doc(&self, spread: &internal::SpreadAttribute) -> Doc {
        let mut parts = vec![doc::text("{...")];

        // Leading comments (between `{...` and expression)
        let expr_start = spread.expression.span().start;
        for comment in tsv_lang::comments_in_range(self.comments, spread.span.start + 4, expr_start)
        {
            parts.push(Self::build_leading_js_comment_doc(comment));
        }

        // Expression doc with any nested comments
        let expr_doc = tsv_ts::build_expression_doc_isolated_with_comments(
            &spread.expression,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            self.comments,
        );
        parts.push(expr_doc);

        // Trailing comments (between expression and `}`)
        let expr_end = spread.expression.span().end;
        for comment in tsv_lang::comments_in_range(self.comments, expr_end, spread.span.end - 1) {
            parts.push(Self::build_trailing_js_comment_doc(comment));
        }

        parts.push(doc::text("}"));
        doc::concat(parts)
    }

    /// Build a Doc for an attach tag: `{@attach expr}`
    fn build_attach_tag_doc(&self, tag: &internal::AttachTag) -> Doc {
        let mut parts = vec![doc::text("{@attach ")];

        // Leading comments (between `{@attach ` and expression)
        let expr_start = tag.expression.span().start;
        for comment in tsv_lang::comments_in_range(self.comments, tag.span.start + 9, expr_start) {
            parts.push(Self::build_leading_js_comment_doc(comment));
        }

        // Expression doc with any nested comments
        let expr_doc = tsv_ts::build_expression_doc_isolated_with_comments(
            &tag.expression,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            self.comments,
        );
        parts.push(expr_doc);

        // Trailing comments (between expression and `}`)
        let expr_end = tag.expression.span().end;
        for comment in tsv_lang::comments_in_range(self.comments, expr_end, tag.span.end - 1) {
            parts.push(Self::build_trailing_js_comment_doc(comment));
        }

        parts.push(doc::text("}"));
        doc::concat(parts)
    }

    // =========================================================================
    // Directive Doc builders
    // =========================================================================

    /// Build a Doc for on:event directive
    fn build_on_directive_doc(&self, d: &internal::OnDirective) -> Doc {
        let mut parts = vec![doc::text("on:"), doc::text_owned(d.name.clone())];
        parts.extend(self.build_modifiers_doc(&d.modifiers));
        if let Some(expr) = &d.expression {
            parts.extend(self.build_expression_doc_parts_with_span(expr, d.expression_tag_span));
        }
        doc::concat(parts)
    }

    /// Build a Doc for bind:prop directive
    fn build_bind_directive_doc(&self, d: &internal::BindDirective) -> Doc {
        let mut parts = vec![doc::text("bind:"), doc::text_owned(d.name.clone())];
        // Only include expression if not shorthand
        if !self.is_identifier_with_name(&d.expression, &d.name) {
            parts.extend(
                self.build_expression_doc_parts_with_span(&d.expression, d.expression_tag_span),
            );
        }
        doc::concat(parts)
    }

    /// Build a Doc for class:name directive
    fn build_class_directive_doc(&self, d: &internal::ClassDirective) -> Doc {
        let mut parts = vec![doc::text("class:"), doc::text_owned(d.name.clone())];
        // Only include expression if not shorthand
        if !self.is_identifier_with_name(&d.expression, &d.name) {
            parts.extend(
                self.build_expression_doc_parts_with_span(&d.expression, d.expression_tag_span),
            );
        }
        doc::concat(parts)
    }

    /// Build a Doc for style:prop directive
    fn build_style_directive_doc(&self, d: &internal::StyleDirective) -> Doc {
        let mut parts = vec![doc::text("style:"), doc::text_owned(d.name.clone())];
        parts.extend(self.build_modifiers_doc(&d.modifiers));
        match &d.value {
            internal::StyleDirectiveValue::True => {}
            internal::StyleDirectiveValue::ExpressionTag(tag) => {
                parts.push(doc::text("="));
                parts.push(self.build_expression_tag_doc(tag));
            }
            internal::StyleDirectiveValue::Parts(value_parts) => {
                parts.push(doc::text("=\""));
                for part in value_parts {
                    parts.push(self.build_attribute_value_doc(part));
                }
                parts.push(doc::text("\""));
            }
        }
        doc::concat(parts)
    }

    /// Build a Doc for use:action directive
    fn build_use_directive_doc(&self, d: &internal::UseDirective) -> Doc {
        let mut parts = vec![doc::text("use:"), doc::text_owned(d.name.clone())];
        if let Some(expr) = &d.expression {
            parts.extend(self.build_expression_doc_parts_with_span(expr, d.expression_tag_span));
        }
        doc::concat(parts)
    }

    /// Build a Doc for transition/in/out directive
    fn build_transition_directive_doc(&self, d: &internal::TransitionDirective) -> Doc {
        let mut parts = vec![
            doc::text(d.direction.prefix_with_colon()),
            doc::text_owned(d.name.clone()),
        ];
        parts.extend(self.build_modifiers_doc(&d.modifiers));
        if let Some(expr) = &d.expression {
            parts.extend(self.build_expression_doc_parts(expr));
        }
        doc::concat(parts)
    }

    /// Build a Doc for animate:name directive
    fn build_animate_directive_doc(&self, d: &internal::AnimateDirective) -> Doc {
        let mut parts = vec![doc::text("animate:"), doc::text_owned(d.name.clone())];
        if let Some(expr) = &d.expression {
            parts.extend(self.build_expression_doc_parts(expr));
        }
        doc::concat(parts)
    }

    /// Build a Doc for let:name directive
    fn build_let_directive_doc(&self, d: &internal::LetDirective) -> Doc {
        let mut parts = vec![doc::text("let:"), doc::text_owned(d.name.clone())];
        if let Some(expr) = &d.expression {
            parts.extend(self.build_expression_doc_parts(expr));
        }
        doc::concat(parts)
    }

    // =========================================================================
    // Shared helpers
    // =========================================================================

    /// Build Doc parts for modifiers: `|mod1|mod2`
    fn build_modifiers_doc(&self, modifiers: &[String]) -> Vec<Doc> {
        modifiers
            .iter()
            .flat_map(|m| vec![doc::text("|"), doc::text_owned(m.clone())])
            .collect()
    }

    /// Build Doc parts for an expression: `={expr}`
    fn build_expression_doc_parts(&self, expr: &tsv_ts::ast::internal::Expression) -> Vec<Doc> {
        self.build_expression_doc_parts_with_span(expr, None)
    }

    /// Build Doc parts for an expression with optional span for comment lookup: `={expr}`
    fn build_expression_doc_parts_with_span(
        &self,
        expr: &tsv_ts::ast::internal::Expression,
        tag_span: Option<tsv_lang::Span>,
    ) -> Vec<Doc> {
        let mut parts = vec![doc::text("={")];

        // Add leading comments if we have the tag span
        if let Some(span) = tag_span {
            let expr_start = expr.span().start;
            for comment in tsv_lang::comments_in_range(self.comments, span.start + 1, expr_start) {
                parts.push(Self::build_leading_js_comment_doc(comment));
            }
        }

        // Use isolated context since the braces provide grouping (no extra parens for sequences)
        // Pass comments so nested comments (in call args, binary expressions) are preserved
        let expr_doc = tsv_ts::build_expression_doc_isolated_with_comments(
            expr,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            self.comments,
        );
        parts.push(expr_doc);

        // Add trailing comments if we have the tag span
        if let Some(span) = tag_span {
            let expr_end = expr.span().end;
            for comment in tsv_lang::comments_in_range(self.comments, expr_end, span.end - 1) {
                parts.push(Self::build_trailing_js_comment_doc(comment));
            }
        }

        parts.push(doc::text("}"));
        parts
    }

    /// Build a Doc for an expression tag: `{expr}`
    pub(super) fn build_expression_tag_doc(&self, tag: &internal::ExpressionTag) -> Doc {
        let mut parts = vec![doc::text("{")];

        // Add leading comments between opening brace and expression
        let expr_start = tag.expression.span().start;
        for comment in tsv_lang::comments_in_range(self.comments, tag.span.start + 1, expr_start) {
            if comment.is_block {
                parts.push(doc::text_owned(format!("/*{}*/ ", comment.content)));
            }
        }

        // Use isolated context since the braces provide grouping (no extra parens for sequences)
        // Pass comments so nested comments (in call args, binary expressions) are preserved
        let expr_doc = tsv_ts::build_expression_doc_isolated_with_comments(
            &tag.expression,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            self.comments,
        );
        parts.push(expr_doc);

        // Add trailing comments between expression and closing brace
        let expr_end = tag.expression.span().end;
        for comment in tsv_lang::comments_in_range(self.comments, expr_end, tag.span.end - 1) {
            if comment.is_block {
                parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
            }
        }

        parts.push(doc::text("}"));
        doc::concat(parts)
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
