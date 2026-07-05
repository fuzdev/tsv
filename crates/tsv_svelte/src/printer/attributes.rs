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

use crate::ast::internal;
use crate::printer::Printer;
use smallvec::smallvec;
use tsv_lang::comments_in_range;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{Span, SymbolResolver, SymbolToU32};
use tsv_ts::ast::internal::Expression;

// Opening prefixes for brace-wrapped attribute expressions. `build_braced_expression_doc`
// emits the prefix and derives the expression offset from its `.len()`, so these are the
// single source for both the emitted text and the comment-scan anchor.
const SPREAD_OPEN: &str = "{...";
const ATTACH_TAG_OPEN: &str = "{@attach ";

/// Whether `raw` is trivially already-normalized, so [`normalize_class_text`]
/// would return it unchanged: single-line, no tabs, no collapsible space runs,
/// no trailing space. Conservative — a `false` only means the `String` path
/// runs (and decides for itself), so this can never change output; it only
/// lets the common `class="a b c"` case skip the transient allocation.
fn class_text_is_normalized(raw: &str) -> bool {
    !raw.ends_with(' ')
        && !raw.as_bytes().windows(2).any(|w| w == b"  ")
        && !raw.contains(['\n', '\t'])
}

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
                d.source_span(comment.content_span, self.source),
                d.text("*/ "),
            ])
        } else {
            // Content already includes the space after // (e.g., " comment" from "// comment")
            d.concat(&[
                d.text("//"),
                d.source_span(comment.content_span, self.source),
                d.hardline(),
            ])
        }
    }

    /// Build a Doc for a trailing JS comment (after content), before a closing
    /// `}` / `)` / ` as ` token emitted by the caller.
    ///
    /// Block comments: ` /*content*/` (inline, leading space) — the closing token
    /// follows on the same line.
    /// Line comments: ` // content` + `hardline` — a `//` comment runs to end of
    /// line, so the closing token MUST drop to the next line; otherwise it would be
    /// swallowed into the comment and lost on reparse. Unlike a trailing line comment
    /// on a TypeScript statement (deferred past the `;` via `line_suffix`), here the
    /// brace stays in expression context — text past `}` is Svelte template text, so
    /// `line_suffix` would render the comment on the page. Keeping `}` on its own line
    /// is the only placement that preserves the comment and stays idempotent. See
    /// `docs/conformance_prettier.md` §Comment Position Philosophy and the
    /// `expr_trailing_line` divergence fixture.
    pub(super) fn build_trailing_js_comment_doc(&self, comment: &tsv_lang::Comment) -> DocId {
        let d = self.d();
        if comment.is_block {
            d.concat(&[
                d.text(" /*"),
                d.source_span(comment.content_span, self.source),
                d.text("*/"),
            ])
        } else {
            // Content already includes the space after // (e.g., " comment" from "// comment")
            d.concat(&[
                d.text(" //"),
                d.source_span(comment.content_span, self.source),
                d.hardline(),
            ])
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
        node: &internal::AttributeNode<'_>,
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
    pub(super) fn build_attribute_doc(
        &self,
        attr: &internal::Attribute<'_>,
        is_html: bool,
    ) -> DocId {
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

            // Fast path: a single value part (the common `name="x"` / `name={x}`).
            // Build with a stack array instead of the per-attribute `parts` buffer.
            if value_parts.len() == 1 {
                let sym = d.symbol(name_sym);
                let value_doc = if normalize_class {
                    self.build_class_attribute_value_doc(&value_parts[0], true)
                } else {
                    self.build_attribute_value_doc(&value_parts[0])
                };
                return if matches!(value_parts[0], internal::AttributeValue::ExpressionTag(_)) {
                    d.concat(&[sym, d.text("="), value_doc])
                } else {
                    d.concat(&[sym, d.text("=\""), value_doc, d.text("\"")])
                };
            }

            // General path: a multi-part value is always a quoted string (a pure
            // `{expr}` value is single-part and handled by the fast path above).
            let mut parts: DocBuf = smallvec![d.symbol(name_sym), d.text("=\"")];
            let last_idx = value_parts.len().saturating_sub(1);
            for (i, part) in value_parts.iter().enumerate() {
                if normalize_class {
                    parts.push(self.build_class_attribute_value_doc(part, i == last_idx));
                } else {
                    parts.push(self.build_attribute_value_doc(part));
                }
            }
            parts.push(d.text("\""));

            d.concat(&parts)
        } else {
            // Boolean attribute
            d.symbol(name_sym)
        }
    }

    /// Build a Doc for an attribute value part
    fn build_attribute_value_doc(&self, value: &internal::AttributeValue<'_>) -> DocId {
        match value {
            internal::AttributeValue::Text(text) => {
                self.build_attribute_text_doc(text.raw(self.source), Some(text.raw_span))
            }
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
        value: &internal::AttributeValue<'_>,
        is_last_part: bool,
    ) -> DocId {
        match value {
            internal::AttributeValue::Text(text) => {
                let raw = text.raw(self.source);
                if class_text_is_normalized(raw) {
                    self.build_attribute_text_doc(raw, Some(text.raw_span))
                } else {
                    let normalized = normalize_class_text(raw, is_last_part);
                    self.build_attribute_text_doc(&normalized, None)
                }
            }
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                self.build_attribute_expression_doc(expr_tag)
            }
        }
    }

    /// Build a Doc for an expression tag inside an attribute value.
    fn build_attribute_expression_doc(&self, expr_tag: &internal::ExpressionTag<'_>) -> DocId {
        self.build_expression_tag_doc(expr_tag)
    }

    /// Build a Doc for attribute text content, handling newlines as literallines.
    fn build_attribute_text_doc(&self, raw: &str, raw_span: Option<Span>) -> DocId {
        let d = self.d();
        if raw.contains('\n') {
            // Split at newlines, join with literalline to preserve literal newlines
            // and trigger will_break on the attribute group
            let line_docs: DocBuf = raw.split('\n').map(|part| d.text_pooled(part)).collect();
            let sep = d.literalline();
            d.join_doc(line_docs, sep)
        } else if let Some(span) = raw_span {
            // Verbatim source slice (`raw == source[span]`): emit without a pool copy.
            d.source_span(span, self.source)
        } else {
            // Owned/normalized text (no source span): pool it.
            d.text_pooled(raw)
        }
    }

    /// Build a Doc for a spread attribute: `{...expr}`
    fn build_spread_attribute_doc(&self, spread: &internal::SpreadAttribute<'_>) -> DocId {
        self.build_braced_expression_doc(
            SPREAD_OPEN,
            &spread.expression,
            spread.span.start,
            spread.span.end,
        )
    }

    /// Build a Doc for an attach tag: `{@attach expr}`
    fn build_attach_tag_doc(&self, tag: &internal::AttachTag<'_>) -> DocId {
        self.build_braced_expression_doc(
            ATTACH_TAG_OPEN,
            &tag.expression,
            tag.span.start,
            tag.span.end,
        )
    }

    /// Build a Doc for a braced expression with comments: `prefix expr }`
    ///
    /// Handles leading/trailing comments between the prefix/suffix and expression.
    fn build_braced_expression_doc(
        &self,
        prefix: &'static str,
        expr: &Expression<'_>,
        span_start: u32,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![d.text(prefix)];

        // The expression begins exactly `prefix.len()` bytes past the span start,
        // so the comment-scan anchor derives from the emitted prefix — the two
        // can't drift apart.
        let comment_start = span_start + prefix.len() as u32;

        // Leading comments (between prefix and expression)
        let expr_start = expr.span().start;
        for comment in comments_in_range(self.comments, comment_start, expr_start) {
            parts.push(self.build_leading_js_comment_doc(comment));
        }

        // Expression doc with any nested comments
        parts.push(self.build_ts_expression_doc(expr));

        // Trailing comments (between expression and `}`)
        let expr_end = expr.span().end;
        for comment in comments_in_range(self.comments, expr_end, span_end - 1) {
            parts.push(self.build_trailing_js_comment_doc(comment));
        }

        parts.push(d.text("}"));
        d.concat(&parts)
    }

    //
    // Directive Doc builders
    //

    /// Build a Doc for a directive with no shorthand suppression: `prefix` + name +
    /// modifiers + optional `={expr}`. Backs the `on:` / `use:` / `animate:` / transition
    /// (`transition:`/`in:`/`out:`) builders, which differ only in the prefix; each passes
    /// its own `modifiers`. Directives with shorthand suppression (`bind`/`class`/`let`) or
    /// non-expression values (`style`) keep their own builders (they also emit modifiers).
    fn build_simple_directive_doc(
        &self,
        prefix: DocId,
        name_span: Span,
        modifiers: &[&str],
        expression: Option<&Expression<'_>>,
        expression_tag_span: Option<Span>,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![prefix, d.source_span(name_span, self.source)];
        parts.extend(self.build_modifiers_doc(modifiers));
        if let Some(expr) = expression {
            parts.extend(self.build_expression_doc_parts_with_span(expr, expression_tag_span));
        }
        d.concat(&parts)
    }

    /// Build a Doc for on:event directive
    fn build_on_directive_doc(&self, dir: &internal::OnDirective<'_>) -> DocId {
        self.build_simple_directive_doc(
            self.d().text("on:"),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
        )
    }

    /// Build a Doc for bind:prop directive
    fn build_bind_directive_doc(&self, dir: &internal::BindDirective<'_>) -> DocId {
        let d = self.d();
        let name = dir.name_span.extract(self.source);
        let mut parts: DocBuf =
            smallvec![d.text("bind:"), d.source_span(dir.name_span, self.source)];
        parts.extend(self.build_modifiers_doc(dir.modifiers));
        // Only include expression if not shorthand
        if !self.is_identifier_with_name(&dir.expression, name) {
            // bind: uses {getter, setter} syntax where SequenceExpression is bare (no parens)
            parts.extend(self.build_expression_doc_parts_with_span_for_bind(
                &dir.expression,
                dir.expression_tag_span,
            ));
        }
        d.concat(&parts)
    }

    /// Build a Doc for class:name directive
    fn build_class_directive_doc(&self, dir: &internal::ClassDirective<'_>) -> DocId {
        let d = self.d();
        let name = dir.name_span.extract(self.source);
        let mut parts: DocBuf =
            smallvec![d.text("class:"), d.source_span(dir.name_span, self.source)];
        parts.extend(self.build_modifiers_doc(dir.modifiers));
        // Only include expression if not shorthand
        if !self.is_identifier_with_name(&dir.expression, name) {
            parts.extend(
                self.build_expression_doc_parts_with_span(&dir.expression, dir.expression_tag_span),
            );
        }
        d.concat(&parts)
    }

    /// Build a Doc for style:prop directive
    fn build_style_directive_doc(&self, dir: &internal::StyleDirective<'_>) -> DocId {
        let d = self.d();
        let name = dir.name_span.extract(self.source);
        let mut parts: DocBuf =
            smallvec![d.text("style:"), d.source_span(dir.name_span, self.source)];
        parts.extend(self.build_modifiers_doc(dir.modifiers));
        match &dir.value {
            internal::StyleDirectiveValue::True => {}
            internal::StyleDirectiveValue::ExpressionTag(tag) => {
                // Only include expression if not shorthand (style:color={color} → style:color)
                if !self.is_identifier_with_name(&tag.expression, name) {
                    parts.push(d.text("="));
                    parts.push(self.build_expression_tag_doc(tag));
                }
            }
            internal::StyleDirectiveValue::Parts(value_parts) => {
                parts.push(d.text("=\""));
                for part in value_parts.iter() {
                    parts.push(self.build_attribute_value_doc(part));
                }
                parts.push(d.text("\""));
            }
        }
        d.concat(&parts)
    }

    /// Build a Doc for use:action directive
    fn build_use_directive_doc(&self, dir: &internal::UseDirective<'_>) -> DocId {
        self.build_simple_directive_doc(
            self.d().text("use:"),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
        )
    }

    /// Build a Doc for transition/in/out directive
    fn build_transition_directive_doc(&self, dir: &internal::TransitionDirective<'_>) -> DocId {
        self.build_simple_directive_doc(
            self.d().text(dir.direction.prefix_with_colon()),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
        )
    }

    /// Build a Doc for animate:name directive
    fn build_animate_directive_doc(&self, dir: &internal::AnimateDirective<'_>) -> DocId {
        self.build_simple_directive_doc(
            self.d().text("animate:"),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
        )
    }

    /// Build a Doc for let:name directive
    fn build_let_directive_doc(&self, dir: &internal::LetDirective<'_>) -> DocId {
        let d = self.d();
        let name = dir.name_span.extract(self.source);
        let mut parts: DocBuf =
            smallvec![d.text("let:"), d.source_span(dir.name_span, self.source)];
        parts.extend(self.build_modifiers_doc(dir.modifiers));
        // Only include expression if not shorthand (let:foo={foo} → let:foo)
        if let Some(expr) = &dir.expression
            && !self.is_identifier_with_name(expr, name)
        {
            parts.extend(self.build_expression_doc_parts_with_span(expr, dir.expression_tag_span));
        }
        d.concat(&parts)
    }

    //
    // Shared helpers
    //

    /// Build Doc parts for modifiers: `|mod1|mod2`
    fn build_modifiers_doc(&self, modifiers: &[&str]) -> DocBuf {
        modifiers
            .iter()
            .flat_map(|m| [self.d().text("|"), self.d().text_pooled(m)])
            .collect()
    }

    /// Build expression doc for attribute context (embedded expression).
    ///
    /// Sets `LayoutMode::Embedded` so binary expressions use ContinuationIndent style.
    /// Assignment expressions get wrapped in parens: `prop={(a = b)}`.
    fn build_expression_doc_for_attribute(&self, expr: &Expression<'_>) -> DocId {
        let d = self.d();
        let embedded = tsv_lang::EmbedContext {
            mode: tsv_lang::LayoutMode::Embedded,
            ..tsv_lang::EmbedContext::default()
        };

        // Assignment expressions need parens in attribute values: prop={(a = b)}
        if let Expression::AssignmentExpression(_) = expr {
            let inner =
                tsv_ts::build_expression_doc_with_comments(d, expr, &self.ts_inputs(), &embedded);
            return d.parens(inner);
        }

        tsv_ts::build_expression_doc_with_comments(d, expr, &self.ts_inputs(), &embedded)
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
        expr: &Expression<'_>,
        tag_span: Option<Span>,
    ) -> DocBuf {
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
            Expression::ArrowFunctionExpression(_)
                | Expression::FunctionExpression(_)
                | Expression::ObjectExpression(_)
                | Expression::ConditionalExpression(_)
                | Expression::CallExpression(_)
                | Expression::NewExpression(_)
                | Expression::ArrayExpression(_)
                | Expression::BinaryExpression(_)
        );

        // A trailing line comment already forces `}` onto its own line (its doc ends
        // in a hardline). Hug it directly — block structure would add its own softline
        // before `}`, leaving a stray blank line (`={\n\texpr // c\n\n}`).
        let has_trailing_line_comment = tag_span.is_some_and(|span| {
            tsv_lang::has_line_comments_in_range(self.comments, expr.span().end, span.end - 1)
        });

        let d = self.d();
        let inner = if is_hugged || has_trailing_line_comment {
            // Hugged: the expression's internal doc handles wrapping
            let content = d.concat(&expr_content);
            d.braces(content)
        } else {
            // Block structure for other expressions
            self.wrap_in_block_structure(expr_content)
        };

        smallvec![d.text("="), inner]
    }

    /// Build expression content with leading/trailing comments
    ///
    /// Returns the doc parts: leading comments + expression doc + trailing comments
    fn build_expression_content_with_comments(
        &self,
        expr: &Expression<'_>,
        tag_span: Option<Span>,
    ) -> DocBuf {
        // Collect leading comments
        let mut leading_comments: DocBuf = DocBuf::new();
        if let Some(span) = tag_span {
            let expr_start = expr.span().start;
            for comment in comments_in_range(self.comments, span.start + 1, expr_start) {
                leading_comments.push(self.build_leading_js_comment_doc(comment));
            }
        }

        let expr_doc = self.build_expression_doc_for_attribute(expr);

        // Collect trailing comments
        let mut trailing_comments: DocBuf = DocBuf::new();
        if let Some(span) = tag_span {
            let expr_end = expr.span().end;
            for comment in comments_in_range(self.comments, expr_end, span.end - 1) {
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
    fn wrap_in_block_structure(&self, expr_content: DocBuf) -> DocId {
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
        expr: &Expression<'_>,
        tag_span: Option<Span>,
    ) -> DocBuf {
        let d = self.d();
        // For SequenceExpression, use the bare (no parens) version for getter/setter syntax
        if let Expression::SequenceExpression(seq) = expr {
            // The per-operand path below is comment-blind, so a leading or interior
            // comment prettier preserves (`{// c\n get, set}`, `{get, /* c */ set}`)
            // was silently dropped — real content loss. Route those through the
            // comment-aware builder. Trailing comments after the last operand are NOT
            // included in the range: prettier drops them, so tsv matches by dropping.
            if let Some(span) = tag_span {
                let last_end = seq.expressions[seq.expressions.len() - 1].span().end;
                if tsv_lang::has_comments_in_range(self.comments, span.start + 1, last_end) {
                    return smallvec![
                        d.text("="),
                        self.build_bind_sequence_with_comments_doc(seq, span),
                    ];
                }
            }

            let len = seq.expressions.len();

            // Build items: each expression with trailing comma (except last)
            let items: DocBuf = seq
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

            // Bare block structure (shared with every other bind value): flat
            // `={getter, setter}`, broken `={\n\tgetter,\n\tsetter\n}`.
            return smallvec![
                d.text("="),
                self.wrap_in_block_structure(smallvec![items_doc])
            ];
        }

        // For bind: directives, BinaryExpression should use block structure (not hugging).
        // This matches Prettier's behavior where bind: uses `={\n\texpr\n}` format.
        if let Expression::BinaryExpression(_) = expr {
            return self.build_expression_doc_parts_with_span_block_structure(expr, tag_span);
        }

        // For other expressions, use the standard method
        self.build_expression_doc_parts_with_span(expr, tag_span)
    }

    /// Build the bare (no-parens) function-binding sequence value when it carries
    /// a leading or interior comment, preserving each comment at the author's
    /// position to match prettier. A line comment, or a multi-line block comment,
    /// forces the broken `{\n …\n}` layout; a lone mid block comment stays inline.
    ///
    /// ```svelte
    /// bind:value={
    ///     // c
    ///     () => a, (v) => (a = v)
    /// }
    /// bind:value={() => a, /* c */ (v) => (a = v)}
    /// ```
    ///
    /// A single-line *leading* block comment (`{/* c */ a, b}`) stays inline and
    /// bare: prettier parenthesizes it (`{/* c */ (a, b)}`) but that form is
    /// non-idempotent — it drops the comment on the next pass — so tsv keeps the
    /// comment bare and idempotent instead. Trailing comments after the last
    /// operand are dropped (prettier drops them too); the caller's range excludes
    /// them.
    fn build_bind_sequence_with_comments_doc(
        &self,
        seq: &tsv_ts::ast::internal::SequenceExpression<'_>,
        tag_span: Span,
    ) -> DocId {
        let d = self.d();
        let bytes = self.source.as_bytes();
        let mut content: DocBuf = DocBuf::new();

        // Leading comments between `{` and the first operand. A line or multi-line
        // block comment ends in a hardline, forcing the outer `{ }` to break — but
        // the operands sit in their own group below, so they only break when *they*
        // overflow or carry their own forced break (matching prettier, which keeps
        // `() => a, (v) => (a = v)` on one line under a leading comment).
        let first_start = seq.expressions[0].span().start;
        for comment in comments_in_range(self.comments, tag_span.start + 1, first_start) {
            if comment.is_block && comment.multiline {
                // Multi-line block: own line(s), forcing the broken layout. Emitted
                // without the inline trailing space so the line ends at `*/`.
                content.push(d.text("/*"));
                content.push(d.source_span(comment.content_span, self.source));
                content.push(d.text("*/"));
                content.push(d.hardline());
            } else {
                // Single-line block: `/*…*/ ` inline. Line comment: `//…` + hardline.
                content.push(self.build_leading_js_comment_doc(comment));
            }
        }

        let mut items: DocBuf = DocBuf::new();
        for (i, sub_expr) in seq.expressions.iter().enumerate() {
            if i > 0 {
                let prev_end = seq.expressions[i - 1].span().end;
                let cur_start = sub_expr.span().start;
                // The separator comma, located in source so a comment on either side
                // is attributed to the right operand (a comment's `,` can't fool it).
                let comma_pos =
                    find_char_skipping_comments(bytes, prev_end as usize, cur_start as usize, b',')
                        .map_or(prev_end, |c| c as u32);

                // Comments before the comma trail the previous operand.
                for comment in comments_in_range(self.comments, prev_end, comma_pos) {
                    items.push(self.build_trailing_js_comment_doc(comment));
                }

                items.push(d.text(","));

                // Comments after the comma: an all-block run leads the next operand
                // inline; a line comment trails the comma and forces the break.
                let after: Vec<_> =
                    comments_in_range(self.comments, comma_pos + 1, cur_start).collect();
                if after.is_empty() {
                    items.push(d.line());
                } else if after.iter().all(|c| c.is_block) {
                    items.push(d.line());
                    for comment in &after {
                        items.push(self.build_leading_js_comment_doc(comment));
                    }
                } else {
                    for comment in &after {
                        items.push(self.build_trailing_js_comment_doc(comment));
                    }
                }
            }

            items.push(self.build_ts_expression_doc(sub_expr));
        }

        // The operands sit in their own group so a forced break in the *surrounding*
        // `{ }` (a leading comment) doesn't break them — they break only when they
        // overflow or carry an interior forced break (a mid line comment, a block-body
        // arrow). Matches prettier.
        let items_doc = d.concat(&items);
        content.push(d.group(items_doc));

        // Same bare block structure as the comment-free path: flat `{a, b}`, broken
        // `{\n\ta,\n\tb\n}`. Comment hardlines force the break; a lone inline block
        // comment leaves the operand group free to stay flat.
        self.wrap_in_block_structure(content)
    }

    /// Build Doc parts using block structure: `={\n\texpr\n}`
    ///
    /// Used for bind: directive expressions where Prettier always uses this format.
    fn build_expression_doc_parts_with_span_block_structure(
        &self,
        expr: &Expression<'_>,
        tag_span: Option<Span>,
    ) -> DocBuf {
        let expr_content = self.build_expression_content_with_comments(expr, tag_span);
        smallvec![
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
    pub(super) fn build_expression_tag_doc(&self, tag: &internal::ExpressionTag<'_>) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![d.text("{")];

        // Add leading comments between { and expression (block inline, line + hardline)
        let expr_start = tag.expression.span().start;
        for comment in comments_in_range(self.comments, tag.span.start + 1, expr_start) {
            parts.push(self.build_leading_js_comment_doc(comment));
        }

        parts.push(self.build_expression_doc_for_attribute(&tag.expression));

        // Add trailing comments. A line comment forces `}` onto its own line (the
        // helper appends a hardline) so the `//` doesn't swallow the brace.
        let expr_end = tag.expression.span().end;
        for comment in comments_in_range(self.comments, expr_end, tag.span.end - 1) {
            parts.push(self.build_trailing_js_comment_doc(comment));
        }

        parts.push(d.text("}"));
        d.concat(&parts)
    }

    /// Check if an attribute is a shorthand: {name} where value is ExpressionTag(Identifier(name))
    fn is_shorthand_attribute(
        &self,
        attr_name: string_interner::DefaultSymbol,
        value_parts: &[internal::AttributeValue<'_>],
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
        let Expression::Identifier(ident) = &expr_tag.expression else {
            return false;
        };

        // The identifier name must match the attribute name. TS identifiers are
        // span-identity (no shared symbol space with the Svelte attribute-name
        // interner), so compare the resolved names.
        let interner = self.interner.borrow();
        let Some(attr_str) = interner.resolve(attr_name) else {
            return false;
        };
        ident.name(self.source, &interner) == attr_str
    }

    /// Check if expression is an identifier with the given name
    fn is_identifier_with_name(&self, expr: &Expression<'_>, name: &str) -> bool {
        if let Expression::Identifier(id) = expr {
            let interner = self.interner.borrow();
            id.name(self.source, &interner) == name
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
