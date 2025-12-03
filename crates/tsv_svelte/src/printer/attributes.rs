// Attribute formatting for Svelte elements
//
// Handles formatting of HTML attributes on elements, including:
// - Boolean attributes (e.g., `disabled`)
// - String attributes (e.g., `class="foo"`)
// - Attach tags (e.g., `{@attach expr}`)
// - Directives (on:, bind:, class:, style:, use:, transition:, animate:, let:)
// - Future: Dynamic attributes ({...spread})

use std::rc::Rc;

use crate::ast::internal;
use crate::printer::Printer;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Format an attribute node (attribute, attach tag, directive, etc.)
    pub(super) fn print_attribute_node(&mut self, node: &internal::AttributeNode) {
        match node {
            internal::AttributeNode::Attribute(attr) => self.print_attribute(attr),
            internal::AttributeNode::SpreadAttribute(spread) => self.print_spread_attribute(spread),
            internal::AttributeNode::AttachTag(tag) => self.print_attach_tag(tag),
            // Directives
            internal::AttributeNode::OnDirective(d) => self.print_on_directive(d),
            internal::AttributeNode::BindDirective(d) => self.print_bind_directive(d),
            internal::AttributeNode::ClassDirective(d) => self.print_class_directive(d),
            internal::AttributeNode::StyleDirective(d) => self.print_style_directive(d),
            internal::AttributeNode::UseDirective(d) => self.print_use_directive(d),
            internal::AttributeNode::TransitionDirective(d) => self.print_transition_directive(d),
            internal::AttributeNode::AnimateDirective(d) => self.print_animate_directive(d),
            internal::AttributeNode::LetDirective(d) => self.print_let_directive(d),
        }
    }

    /// Build a Doc for an attribute node (used for line wrapping calculations)
    pub(super) fn build_attribute_node_doc(&self, node: &internal::AttributeNode) -> Doc {
        match node {
            internal::AttributeNode::Attribute(attr) => self.build_attribute_doc(attr),
            internal::AttributeNode::SpreadAttribute(spread) => {
                self.build_spread_attribute_doc(spread)
            }
            internal::AttributeNode::AttachTag(tag) => self.build_attach_tag_doc(tag),
            // Directives - build doc by printing to string (TODO: proper Doc tree)
            internal::AttributeNode::OnDirective(d) => {
                self.build_directive_doc_via_string(|p| p.print_on_directive(d))
            }
            internal::AttributeNode::BindDirective(d) => {
                self.build_directive_doc_via_string(|p| p.print_bind_directive(d))
            }
            internal::AttributeNode::ClassDirective(d) => {
                self.build_directive_doc_via_string(|p| p.print_class_directive(d))
            }
            internal::AttributeNode::StyleDirective(d) => {
                self.build_directive_doc_via_string(|p| p.print_style_directive(d))
            }
            internal::AttributeNode::UseDirective(d) => {
                self.build_directive_doc_via_string(|p| p.print_use_directive(d))
            }
            internal::AttributeNode::TransitionDirective(d) => {
                self.build_directive_doc_via_string(|p| p.print_transition_directive(d))
            }
            internal::AttributeNode::AnimateDirective(d) => {
                self.build_directive_doc_via_string(|p| p.print_animate_directive(d))
            }
            internal::AttributeNode::LetDirective(d) => {
                self.build_directive_doc_via_string(|p| p.print_let_directive(d))
            }
        }
    }

    /// Helper to build a Doc for directives by printing to string
    fn build_directive_doc_via_string<F>(&self, print_fn: F) -> Doc
    where
        F: FnOnce(&mut Printer),
    {
        let mut temp_printer = Printer::with_config(
            self.source,
            Rc::clone(&self.interner),
            self.ts_comments,
            self.config,
        );
        print_fn(&mut temp_printer);
        doc::text(temp_printer.into_string())
    }

    /// Format an attach tag: {@attach expr}
    fn print_attach_tag(&mut self, tag: &internal::AttachTag) {
        self.write("{@attach ");
        // Use format_expression for proper line breaking on long expressions
        let formatted =
            tsv_ts::format_expression(&tag.expression, self.source, Rc::clone(&self.interner));
        self.write(&formatted);
        self.write("}");
    }

    /// Build a Doc for an attach tag
    fn build_attach_tag_doc(&self, tag: &internal::AttachTag) -> Doc {
        // For now, render the attach tag to a string
        let mut temp_printer = Printer::with_config(
            self.source,
            Rc::clone(&self.interner),
            self.ts_comments,
            self.config,
        );
        temp_printer.print_attach_tag(tag);
        doc::text(temp_printer.into_string())
    }

    /// Format a spread attribute: {...expr}
    ///
    /// Uses doc system for proper line wrapping of long member chains:
    /// ```svelte
    /// <Comp
    ///     {...a.b.c.d.e.f.g.h.i.j.k.l.m.n
    ///         .o.p.q.r.s.t}
    /// />
    /// ```
    fn print_spread_attribute(&mut self, spread: &internal::SpreadAttribute) {
        let doc = self.build_spread_attribute_doc(spread);
        let current_col = self.buffer.current_column(self.config.tab_width);
        // Use print_doc_with_indent to preserve indent level for wrapped content
        let output = doc::print_doc_with_indent(&doc, &self.config, current_col, self.indent_level);
        self.write(&output);
    }

    /// Build a Doc for a spread attribute with wrapping support
    fn build_spread_attribute_doc(&self, spread: &internal::SpreadAttribute) -> Doc {
        // Build the expression doc using the TS printer's doc builder
        let expr_doc = tsv_ts::build_expression_doc(
            &spread.expression,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
        );

        // Wrap in {...} with potential break after opening
        doc::concat(vec![doc::text("{..."), expr_doc, doc::text("}")])
    }

    /// Format an attribute (name="value" or name or {shorthand})
    ///
    /// Formats both boolean attributes (no value), valued attributes, and shorthand.
    ///
    /// # Examples
    /// ```text
    /// disabled          → disabled
    /// class="foo"       → class="foo"
    /// context="module"  → context="module"
    /// {name}            → {name}  (shorthand)
    /// ```
    fn print_attribute(&mut self, attr: &internal::Attribute) {
        // Resolve attribute name from interner
        let name = self.resolve_symbol(attr.name);

        // Check for shorthand: {name} where name == expression identifier
        if let Some(value_parts) = &attr.value {
            if self.is_shorthand_attribute(attr.name, value_parts) {
                // Shorthand: print as {name}
                self.write("{");
                self.write(&name);
                self.write("}");
                return;
            }

            // Regular attribute with value
            self.write(&name);

            // Check if value is a single expression (no quotes needed)
            let is_pure_expression = value_parts.len() == 1
                && matches!(value_parts[0], internal::AttributeValue::ExpressionTag(_));

            if is_pure_expression {
                self.write("=");
            } else {
                self.write("=\"");
            }

            for part in value_parts {
                self.print_attribute_value(part);
            }

            if !is_pure_expression {
                self.write("\"");
            }
        } else {
            // Boolean attribute (no value)
            self.write(&name);
        }
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

    /// Build a Doc for a single attribute (used for line wrapping calculations)
    ///
    /// This is used in the hybrid doc-builder approach for attribute wrapping.
    pub(super) fn build_attribute_doc(&self, attr: &internal::Attribute) -> Doc {
        let name = self.resolve_symbol(attr.name);

        if let Some(value_parts) = &attr.value {
            // Check for shorthand: {name}
            if self.is_shorthand_attribute(attr.name, value_parts) {
                return doc::concat(vec![doc::text("{"), doc::text(name), doc::text("}")]);
            }

            let is_pure_expression = value_parts.len() == 1
                && matches!(value_parts[0], internal::AttributeValue::ExpressionTag(_));

            let mut parts = vec![doc::text(name)];

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
            doc::text(name)
        }
    }

    /// Build a Doc for an attribute value part
    fn build_attribute_value_doc(&self, value: &internal::AttributeValue) -> Doc {
        match value {
            internal::AttributeValue::Text(text) => doc::text(&text.raw),
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                // TODO: For now, render the expression tag to a string
                // In future full refactor, this would build a Doc tree for the expression
                let mut temp_printer = Printer::with_config(
                    self.source,
                    Rc::clone(&self.interner),
                    self.ts_comments,
                    self.config,
                );
                temp_printer.print_expression_tag(expr_tag);
                doc::text(temp_printer.into_string())
            }
        }
    }

    /// Format an attribute value part
    ///
    /// Attribute values can contain static text or dynamic expressions.
    fn print_attribute_value(&mut self, value: &internal::AttributeValue) {
        match value {
            internal::AttributeValue::Text(text) => {
                self.write(&text.raw);
            }
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                self.print_expression_tag(expr_tag);
            }
        }
    }

    // =========================================================================
    // Directive printing
    // =========================================================================

    /// Format on:event directive: `on:click={handler}` or `on:click|modifiers={handler}`
    fn print_on_directive(&mut self, d: &internal::OnDirective) {
        self.write("on:");
        self.write(&d.name);
        self.print_modifiers(&d.modifiers);
        self.print_optional_expression(d.expression.as_ref());
    }

    /// Format bind:prop directive: `bind:value={name}` or `bind:value`
    fn print_bind_directive(&mut self, d: &internal::BindDirective) {
        self.write("bind:");
        self.write(&d.name);
        self.print_modifiers(&d.modifiers);
        // Only print expression if it's not the shorthand (identifier matching name)
        if !self.is_identifier_with_name(&d.expression, &d.name) {
            self.print_expression(&d.expression);
        }
    }

    /// Format class:name directive: `class:active={isActive}` or `class:active`
    fn print_class_directive(&mut self, d: &internal::ClassDirective) {
        self.write("class:");
        self.write(&d.name);
        self.print_modifiers(&d.modifiers);
        // Only print expression if it's not the shorthand (identifier matching name)
        if !self.is_identifier_with_name(&d.expression, &d.name) {
            self.print_expression(&d.expression);
        }
    }

    /// Format style:prop directive: `style:color={value}` or `style:color`
    fn print_style_directive(&mut self, d: &internal::StyleDirective) {
        self.write("style:");
        self.write(&d.name);
        self.print_modifiers(&d.modifiers);
        match &d.value {
            internal::StyleDirectiveValue::True => {}
            internal::StyleDirectiveValue::ExpressionTag(tag) => {
                self.write("=");
                self.print_expression_tag(tag);
            }
            internal::StyleDirectiveValue::Parts(parts) => {
                self.write("=\"");
                for part in parts {
                    self.print_attribute_value(part);
                }
                self.write("\"");
            }
        }
    }

    /// Format use:action directive: `use:action={params}` or `use:action`
    fn print_use_directive(&mut self, d: &internal::UseDirective) {
        self.write("use:");
        self.write(&d.name);
        self.print_modifiers(&d.modifiers);
        self.print_optional_expression(d.expression.as_ref());
    }

    /// Format transition/in/out directive: `transition:fade|local={params}`
    fn print_transition_directive(&mut self, d: &internal::TransitionDirective) {
        let prefix = match (d.intro, d.outro) {
            (true, true) => "transition:",
            (true, false) => "in:",
            (false, true) => "out:",
            (false, false) => "transition:", // Shouldn't happen
        };
        self.write(prefix);
        self.write(&d.name);
        self.print_modifiers(&d.modifiers);
        self.print_optional_expression(d.expression.as_ref());
    }

    /// Format animate:name directive: `animate:flip={params}`
    fn print_animate_directive(&mut self, d: &internal::AnimateDirective) {
        self.write("animate:");
        self.write(&d.name);
        self.print_modifiers(&d.modifiers);
        self.print_optional_expression(d.expression.as_ref());
    }

    /// Format let:name directive: `let:item={localItem}` or `let:item`
    fn print_let_directive(&mut self, d: &internal::LetDirective) {
        self.write("let:");
        self.write(&d.name);
        self.print_modifiers(&d.modifiers);
        self.print_optional_expression(d.expression.as_ref());
    }

    // =========================================================================
    // Directive helpers
    // =========================================================================

    /// Print modifiers: `|mod1|mod2`
    fn print_modifiers(&mut self, modifiers: &[String]) {
        for modifier in modifiers {
            self.write("|");
            self.write(modifier);
        }
    }

    /// Print an optional expression as `={expr}`
    fn print_optional_expression(&mut self, expr: Option<&tsv_ts::ast::internal::Expression>) {
        if let Some(e) = expr {
            self.print_expression(e);
        }
    }

    /// Print an expression as `={expr}`
    fn print_expression(&mut self, expr: &tsv_ts::ast::internal::Expression) {
        self.write("={");
        let formatted = tsv_ts::format_expression(expr, self.source, Rc::clone(&self.interner));
        self.write(&formatted);
        self.write("}");
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
