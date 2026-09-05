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
use crate::printer::{CommentRun, HeadExpr, HeadLayout, Printer};
use smallvec::smallvec;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{Comment, Span};
use tsv_ts::ast::internal::Expression;

/// How a directive prints its expression value — the only axis the expression-valued
/// directive builders differ on, so [`Printer::build_directive_doc`] serves them all.
#[derive(Clone, Copy)]
enum DirectiveValue {
    /// `on:` / `use:` / `animate:` / `transition:`(`in:`/`out:`) — the value is always
    /// emitted; `on:click={click}` is not a shorthand and keeps its value.
    Always,
    /// `class:` / `let:` — `name={name}` is the shorthand form and collapses to bare `name`.
    Shorthand,
    /// `bind:` — the shorthand rule plus the bare (no-parens) `{getter, setter}` sequence.
    ShorthandBind,
}

impl DirectiveValue {
    /// Whether this directive KIND has a bare-`name` shorthand at all — `class:` / `let:` /
    /// `bind:` do, `on:` / `use:` / `animate:` / `transition:` do not.
    ///
    /// **Necessary, never sufficient**: whether a given *value* may take that form is
    /// [`Printer::value_collapses_to_shorthand`], the one predicate every collapse site
    /// asks. Reading this shape test as the whole answer is how the sites would drift.
    const fn has_shorthand_form(self) -> bool {
        matches!(self, Self::Shorthand | Self::ShorthandBind)
    }
}

/// Which host an **unprefixed** `{…}` value is built for — the two per-host verdicts
/// [`Printer::build_expression_content_with_comments`] needs, as one axis instead of the
/// bool pair (`always_block` + `cast_cannot_hang`) whose fourth state no caller could
/// mean.
#[derive(Clone, Copy)]
enum UnprefixedHost {
    /// The expression tag — and everything routed through it: attribute values (event
    /// handlers included), the `style:` directive value, and the special elements'
    /// `this={…}`. Hugs its braces, so a leading JSDoc cast cannot hang its break and
    /// reflows (`EmbedContext::jsdoc_cast_cannot_hang`).
    Tag,
    /// An expression-valued directive choosing hug-vs-block by expression kind
    /// ([`Printer::build_expression_doc_parts_with_span`]): its block form gives a
    /// leading cast's own-line comment a real indented line, so the authoring survives —
    /// no reflow.
    Directive,
    /// `bind:`'s always-block path: block structure whatever the freeze verdict says,
    /// with `Directive`'s no-reflow answer.
    DirectiveBlock,
}

impl UnprefixedHost {
    /// The caller wraps the content in block structure whatever the freeze verdict says.
    const fn always_block(self) -> bool {
        matches!(self, Self::DirectiveBlock)
    }

    /// A leading JSDoc cast cannot hang in this host, so its break reflows.
    const fn cast_cannot_hang(self) -> bool {
        matches!(self, Self::Tag)
    }
}

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
/// `is_last_part`: when false, keeps one trailing space to separate the text from the
/// `{expr}` that follows (`class="a {expr}"`). The separator rule is keyed on the text's
/// **last line**, never on the whole part — prettier's `([^ \t\n])[ \t]+$` fires only
/// where the trailing run follows content on its own line. An indentation-only last
/// line (`class="a⏎    {expr}"`) is leading whitespace and stays as authored; pushing a
/// separator there would deepen the continuation line by one column on every pass.
fn normalize_class_text(raw: &str, is_last_part: bool) -> String {
    const CLASS_WS: [char; 2] = [' ', '\t'];
    let mut result = String::with_capacity(raw.len());
    for (line_idx, line) in raw.split('\n').enumerate() {
        if line_idx > 0 {
            result.push('\n');
        }
        // Leading whitespace is kept verbatim; after it, every `[ \t]+` run collapses to
        // one space and the trailing run is dropped.
        let content = line.trim_start_matches(CLASS_WS);
        result.push_str(&line[..line.len() - content.len()]);
        let mut words = content.split(CLASS_WS).filter(|word| !word.is_empty());
        if let Some(first) = words.next() {
            result.push_str(first);
            for word in words {
                result.push(' ');
                result.push_str(word);
            }
        }
    }

    // The separator space: a non-last part whose LAST line ends content with whitespace
    // keeps one space before the `{expr}` (`class="text {expr}"`). A last line with no
    // content — all-whitespace text (" ") or a continuation line's indentation — is
    // leading whitespace, already pushed verbatim above, and gets no separator.
    let last_line = raw.rsplit_once('\n').map_or(raw, |(_, last)| last);
    if !is_last_part
        && last_line.ends_with(CLASS_WS)
        && !last_line.trim_matches(CLASS_WS).is_empty()
    {
        result.push(' ');
    }

    result
}

impl<'a> Printer<'a> {
    //
    // JS Comment Doc builders
    //

    /// One JS comment's own text, **verbatim** from source — no separator, no break, no
    /// ledger tag. The **attr** builder's spelling (`Printer::build_attr_js_comment_doc`,
    /// which adds the ledger tag): a between-attributes comment is a template-position
    /// comment with a live prettier oracle, and prettier keeps it verbatim at every
    /// payload — multi-line interiors included (`svelte/attributes/comment` pins the
    /// match, "preserved verbatim, not dedented"). The leading and trailing builders'
    /// comments are JS-*expression* comments instead, which prettier either reindents
    /// (leading) or drops outright (trailing — no oracle, so tsv answers with the
    /// `<script>` twin's form): those route through `tsv_ts::build_comment_doc` and
    /// never reach this.
    ///
    /// ⚠️ **A line comment must reach the doc as ONE node**, which is why this returns a
    /// whole-`span` slice rather than assembling `text("//") + <content>`. The swallow
    /// check arms on a text node carrying the entire comment
    /// (`DocArena::line_comment_source_span`), so the split spelling — a hand-rolled
    /// `text("//") + content` — presents nothing for it to tag, and every `//`
    /// this crate prints goes unguarded. Spelling, not intent, decides whether an emitter
    /// is instrumented; keep new emitters on this function or on
    /// `tsv_ts::build_comment_doc`, whose line arm is the same instrumented node.
    ///
    /// A block comment's whole span is verbatim `/*…*/` for the same reason the content
    /// slice was: the delimiters are source bytes, not synthesized.
    pub(super) fn js_comment_text_doc(&self, comment: &Comment) -> DocId {
        let d = self.d();
        if comment.is_block {
            d.source_span(comment.span, self.source)
        } else {
            d.line_comment_source_span(comment.span, self.source)
        }
    }

    /// Build a Doc for a leading JS comment (before content)
    ///
    /// The comment's own text comes from `tsv_ts::build_comment_doc` — the *same*
    /// rendering the owned path uses (`prepend_owned_leading_comment`) — so a JS comment
    /// in an expression value formats exactly as it would in a `<script>` block. For a
    /// single-line block or a `//` that is the identical verbatim node this builder used
    /// to hand-roll; the payload it exists for is the **multi-line block**, which
    /// reindents to context (`*`-aligned interiors) or preserves its layout (any other),
    /// and propagates its break via a `MultilineText`, forcing the surrounding
    /// value/head/attribute to expand. That is what keeps a **non-owned** leading
    /// multi-line block idempotent: the bare authoring glues it to its operand (owned, so
    /// tsv_ts prints it and forces the break), but stripping a redundant grouping paren
    /// leaves it positional (a discarded `(` owns nothing) — and a verbatim source span
    /// emits it inline with no break, so a paren-stripped value stayed inline on pass 1
    /// and expanded only on pass 2 (an F1 non-idempotency). `build_comment_doc` tags the
    /// print-once ledger itself, so this builder must **not** tag again.
    ///
    /// Single-line block comments: `/*content*/ ` (with trailing space).
    /// Line comments: `// content\n` (with hardline).
    ///
    /// A **block** comment's separator is a hardline whenever it doesn't glue to what
    /// follows — see [`Self::leading_js_comment_separator`], which owns that rule. A `//`
    /// always takes the hardline: it runs to end of line, so the glued answer is never
    /// available to it.
    pub(super) fn build_leading_js_comment_doc(&self, comment: &Comment) -> DocId {
        let d = self.d();
        let doc = tsv_ts::build_comment_doc(d, comment, &self.ts_inputs());
        let separator = if comment.is_block {
            self.leading_js_comment_separator(comment)
        } else {
            d.hardline()
        };
        d.concat(&[doc, separator])
    }

    /// What separates a **block** leading comment from the construct it leads: a space when
    /// the author glued the two together and the comment is free to share that line, a
    /// hardline otherwise. Two independent reasons to break, one separator:
    ///
    /// - **The author left it on its own line** (a newline after the `*/`). This is
    ///   prettier's `printLeadingComment` rule, and it reads only the source right after the
    ///   comment (`hasNewline(text, locEnd(comment))`), never where the value starts. A
    ///   trailing space instead pulled the value up onto the comment's closing line,
    ///   reflowing a break the author gave a comment that cannot be reflowed.
    /// - **An honored directive**, whose placement floor makes a shared line inert — so the
    ///   freeze it earned on this pass would be gone on the next. Here the rule is about the
    ///   DIRECTIVE, not the target: a gap that doesn't freeze today can only ever start
    ///   honoring one if the placement survives to be read. *Placement* keys the freeze,
    ///   never the spelling.
    ///
    /// ⚠️ Same intent as `tsv_ts`'s `Printer::comment_hugs_next`, but **not** the same
    /// question, so this is deliberately not a call into it: that one is *anchored* (is the
    /// token at `next` on the comment's line, read from the line-break table), which the
    /// `<script>` gap emitters need because what follows a comment there may be another
    /// comment. Here the rule is prettier's unanchored form — what the author wrote
    /// immediately after the `*/` — so the two spellings are not interchangeable.
    fn leading_js_comment_separator(&self, comment: &Comment) -> DocId {
        let d = self.d();
        let glued =
            !tsv_lang::source_scan::has_newline_after_position(self.source, comment.span.end)
                && !self.is_honored_directive(comment);
        if glued { d.text(" ") } else { d.hardline() }
    }

    /// Whether a trailing comment **starts its own output line**: the comment immediately
    /// before it — whitespace only in between — is a LINE comment, whose emitted form ends
    /// in a `hardline`. The separator space then has nothing to separate; it would render as
    /// leading whitespace on a fresh line, an indent tsv emits nowhere else.
    ///
    /// The trailing twin of [`Self::leading_js_comment_separator`], and keyed the same way:
    /// on what actually gets emitted, not on the author's line treatment. A newline in
    /// source is the wrong test — two block comments the author split across lines
    /// (`a /* c */⏎/* d */`) are emitted on ONE line, where the space is a real separator.
    /// Only a `//` forces the break, so only a `//` before it can take the space away.
    ///
    /// Asked of the comment itself rather than threaded through each caller's run, so the
    /// three builders that emit trailing runs ([`Self::trailing_comment_docs`] for the value
    /// heads, the `bind:` sequence's comma gaps, `{@debug}`) cannot answer it differently — or
    /// forget to.
    fn trailing_comment_starts_line(&self, comment: &Comment) -> bool {
        // A `//` runs to end of line, so a line comment before this one necessarily left it
        // starting a fresh line — which the source bytes answer in a couple of steps. Almost
        // every trailing comment bails here, before the span search.
        if !tsv_lang::source_scan::has_newline_before_position(self.source, comment.span.start) {
            return false;
        }
        let idx = tsv_lang::find_first_comment_from(self.comments, comment.span.start);
        let Some(prev) = idx.checked_sub(1).map(|i| &self.comments[i]) else {
            return false;
        };
        !prev.is_block
            && self
                .source
                .get(prev.span.end as usize..comment.span.start as usize)
                .is_some_and(|between| between.trim().is_empty())
    }

    /// Build a Doc for a trailing JS comment (after content), before a closing
    /// `}` / `)` / ` as ` token emitted by the caller.
    ///
    /// The comment's own text comes from `tsv_ts::build_comment_doc`, exactly as in
    /// [`Self::build_leading_js_comment_doc`] — for single-line payloads the identical
    /// verbatim node, and for the **multi-line block** the `<script>` twin's form:
    /// TypeScript formatting is context-free, so a `*`-aligned interior reindents to
    /// context, any other interior is preserved verbatim, and the break propagates via a
    /// `MultilineText`. Emitting the verbatim source span instead would make the
    /// comment's interior columns a fixed point of whatever the author wrote — the same
    /// comment holding N distinct stable forms, one per authoring
    /// (`expr_trailing_multiline_prettier_divergence`). `build_comment_doc` tags the
    /// print-once ledger itself, so this builder must **not** tag again.
    ///
    /// Single-line block comments: ` /*content*/` (inline, leading space) — the closing
    /// token follows on the same line.
    /// Line comments: ` // content` + `hardline` — a `//` comment runs to end of
    /// line, so the closing token MUST drop to the next line; otherwise it would be
    /// swallowed into the comment and lost on reparse. Unlike a trailing line comment
    /// on a TypeScript statement (deferred past the `;` via `line_suffix`), here the
    /// brace stays in expression context — text past `}` is Svelte template text, so
    /// `line_suffix` would render the comment on the page. Keeping `}` on its own line
    /// is the only placement that preserves the comment and stays idempotent. See
    /// `docs/conformance_prettier.md` §Comment Position Philosophy and the
    /// `expr_trailing_line` divergence fixture.
    ///
    /// The leading space is a **separator from the content this comment trails**, so it is
    /// dropped when there is no such content on the line — see
    /// [`Self::trailing_comment_starts_line`].
    ///
    /// `dedent_break` — this comment ends the run **and** the run sits inside an
    /// `indent(…)`, so its `hardline` is emitted one level out. **The break decides the
    /// closing token's column**: the renderer writes a line's indentation from the line
    /// command that produced it, and the closing token that reuses this break (rather than
    /// adding a second one, which would render as a blank line) sits *outside* that indent —
    /// so without the dedent the `}` lands one level too deep.
    ///
    /// Dedenting the existing break rather than moving it to the closer is deliberate: a
    /// `break_parent` in the closer's place makes `fits()` fail for every group still open
    /// on the line, which re-breaks operands that were fitting fine.
    pub(super) fn build_trailing_js_comment_doc(
        &self,
        comment: &Comment,
        dedent_break: bool,
    ) -> DocId {
        let d = self.d();
        // Every payload takes the same leading separator; only the trailing one differs,
        // so the arms are one assembly rather than per-kind spellings of the comment.
        let mut parts = DocBuf::new();
        if !self.trailing_comment_starts_line(comment) {
            parts.push(d.text(" "));
        }
        parts.push(tsv_ts::build_comment_doc(d, comment, &self.ts_inputs()));
        if !comment.is_block {
            parts.push(if dedent_break {
                d.dedent(d.hardline())
            } else {
                d.hardline()
            });
        }
        d.concat(&parts)
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
        // Span-identity attribute name (`source[name_span]`), reused across the
        // branches below.
        let name_doc = d.source_span(attr.name_span, self.source);

        if let Some(value_parts) = &attr.value {
            // Check for shorthand: {name}
            if self.is_shorthand_attribute(attr, value_parts) {
                return d.braces(name_doc);
            }

            // Normalize whitespace in class attributes on HTML elements
            let normalize_class = is_html && attr.name(self.source) == "class";

            // Fast path: a single value part (the common `name="x"` / `name={x}`).
            // Build with a stack array instead of the per-attribute `parts` buffer.
            if value_parts.len() == 1 {
                let value_doc = if normalize_class {
                    self.build_class_attribute_value_doc(&value_parts[0], true)
                } else {
                    self.build_attribute_value_doc(&value_parts[0])
                };
                return if matches!(value_parts[0], internal::AttributeValue::ExpressionTag(_)) {
                    d.concat(&[name_doc, d.text("="), value_doc])
                } else {
                    let (open, close) = self.attribute_value_delims(value_parts);
                    d.concat(&[name_doc, open, value_doc, close])
                };
            }

            // General path: a multi-part value is always a quoted string (a pure
            // `{expr}` value is single-part and handled by the fast path above).
            let (open, close) = self.attribute_value_delims(value_parts);
            let mut parts: DocBuf = smallvec![name_doc, open];
            let last_idx = value_parts.len().saturating_sub(1);
            for (i, part) in value_parts.iter().enumerate() {
                if normalize_class {
                    parts.push(self.build_class_attribute_value_doc(part, i == last_idx));
                } else {
                    parts.push(self.build_attribute_value_doc(part));
                }
            }
            parts.push(close);

            d.concat(&parts)
        } else {
            // Boolean attribute
            name_doc
        }
    }

    /// Choose the delimiter for an attribute value from its raw text.
    ///
    /// Defaults to double quotes (`'value'` normalizes to `"value"`, matching
    /// prettier-plugin-svelte), but a value whose raw text contains a literal `"`
    /// cannot be double-quoted: HTML §13.1.2.3 requires a double-quoted value to
    /// hold no literal `"`, and re-wrapping one in `"` corrupts the markup (the
    /// interior `"` closes the value early). Such a value takes single-quote
    /// delimiters instead. `{expr}` parts are brace-delimited, so their interior
    /// quotes never reach the value boundary and don't affect the choice.
    ///
    /// A value carrying **both** quote kinds can take no delimiter at all, and is
    /// emitted unquoted. Only a top-level `<script>`/`<style>` head reaches this: the
    /// element reader's unquoted value stops at either quote and its quoted value
    /// cannot hold its own delimiter, so there a raw value has at most one kind — but
    /// the static reader's value alternative is `[^>\s]+`, which admits both
    /// (`<script a=x'y"z>`). The unquoted form is always available for exactly that
    /// value: having come from that alternative, it holds no whitespace and no `>`.
    fn attribute_value_delims(&self, parts: &[internal::AttributeValue<'_>]) -> (DocId, DocId) {
        let d = self.d();
        let mut has_double = false;
        let mut has_single = false;
        for part in parts {
            if let internal::AttributeValue::Text(text) = part {
                let raw = text.raw(self.source);
                has_double |= raw.contains('"');
                has_single |= raw.contains('\'');
            }
        }
        // Each arm spells its literals, so the pair is a constant at the arm rather than a
        // runtime `text()` at every attribute.
        match (has_double, has_single) {
            (true, true) => (d.text("="), d.text("")),
            (true, false) => (d.text("='"), d.text("'")),
            _ => (d.text("=\""), d.text("\"")),
        }
    }

    /// Build a Doc for an attribute value part
    fn build_attribute_value_doc(&self, value: &internal::AttributeValue<'_>) -> DocId {
        match value {
            internal::AttributeValue::Text(text) => {
                self.build_attribute_text_doc(text.raw(self.source), Some(text.raw_span))
            }
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                self.build_expression_tag_doc(expr_tag)
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
                self.build_expression_tag_doc(expr_tag)
            }
        }
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
        // The expression begins exactly `prefix.len()` bytes past the span start,
        // so the comment-scan anchor derives from the emitted prefix — the two
        // can't drift apart.
        let comment_start = span_start + prefix.len() as u32;

        // The prefix→value head: an own-line directive there freezes the whole value, and
        // the head takes the broken prefixed form that keeps the directive on its own line
        // (flush against the prefix it would be inert, and the freeze gone next pass).
        let frozen = self.honored_directive_in_gap(comment_start, expr.span().start);

        // Expression doc with any nested comments, under the host's own embed (this head
        // is measured where it sits, unlike an unprefixed `{…}` value) plus the
        // leading-cast reflow every hugging braced head owes, and the clarity parens an
        // assignment owes (`{...(a = b)}`, `{@attach (a = b)}`).
        let value_doc = self.build_head_value_doc(expr, frozen, &self.cannot_hang_embed());
        let value_doc = self.wrap_value_clarity_parens(expr, value_doc);

        // The head's CONTENT only — the prefix and the `}` are the assembler's
        // (`Printer::build_prefixed_head_doc`), which is what lets one shape serve both
        // verdicts.
        let head =
            self.assemble_head_expr(value_doc, comment_start, expr.span(), span_end - 1, frozen);
        self.build_prefixed_head_doc(prefix, head, self.d().text("}"))
    }

    //
    // Directive Doc builders
    //

    /// The head every directive starts with — `prefix` + name + `|modifier` run — as the
    /// parts buffer the caller appends its value to. The name is a span-identity source
    /// slice, so the emitted text is the author's.
    fn directive_head_parts(&self, prefix: DocId, name_span: Span, modifiers: &[&str]) -> DocBuf {
        let d = self.d();
        let mut parts: DocBuf = smallvec![prefix, d.source_span(name_span, self.source)];
        parts.extend(self.build_modifiers_doc(modifiers));
        parts
    }

    /// Build a Doc for a directive with an **expression** value: the head above plus an
    /// optional `={expr}`. Backs every directive except `style:`, whose value is a quoted
    /// text/tag list rather than an expression; `value` is the only thing they differ on
    /// beyond the prefix, so the per-kind half of the shorthand question is asked once here
    /// rather than per builder. The per-value half is
    /// [`Printer::value_collapses_to_shorthand`], shared with `style:` and with plain
    /// attributes.
    fn build_directive_doc(
        &self,
        prefix: DocId,
        name_span: Span,
        modifiers: &[&str],
        expression: Option<&Expression<'_>>,
        expression_tag_span: Option<Span>,
        value: DirectiveValue,
    ) -> DocId {
        let mut parts = self.directive_head_parts(prefix, name_span, modifiers);
        if let Some(expr) = expression
            // Shorthand (`class:foo={foo}` → `class:foo`) suppresses the value entirely.
            && !(value.has_shorthand_form()
                && self.value_collapses_to_shorthand(
                    expr,
                    name_span.extract(self.source),
                    expression_tag_span,
                ))
        {
            parts.extend(match value {
                // bind: uses {getter, setter} syntax where SequenceExpression is bare (no parens)
                DirectiveValue::ShorthandBind => {
                    self.build_expression_doc_parts_with_span_for_bind(expr, expression_tag_span)
                }
                _ => self.build_expression_doc_parts_with_span(expr, expression_tag_span),
            });
        }
        self.d().concat(&parts)
    }

    /// Build a Doc for on:event directive
    fn build_on_directive_doc(&self, dir: &internal::OnDirective<'_>) -> DocId {
        self.build_directive_doc(
            self.d().text("on:"),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
            DirectiveValue::Always,
        )
    }

    /// Build a Doc for bind:prop directive
    fn build_bind_directive_doc(&self, dir: &internal::BindDirective<'_>) -> DocId {
        self.build_directive_doc(
            self.d().text("bind:"),
            dir.name_span,
            dir.modifiers,
            Some(&dir.expression),
            dir.expression_tag_span,
            DirectiveValue::ShorthandBind,
        )
    }

    /// Build a Doc for class:name directive
    fn build_class_directive_doc(&self, dir: &internal::ClassDirective<'_>) -> DocId {
        self.build_directive_doc(
            self.d().text("class:"),
            dir.name_span,
            dir.modifiers,
            Some(&dir.expression),
            dir.expression_tag_span,
            DirectiveValue::Shorthand,
        )
    }

    /// Build a Doc for style:prop directive
    fn build_style_directive_doc(&self, dir: &internal::StyleDirective<'_>) -> DocId {
        let d = self.d();
        let name = dir.name_span.extract(self.source);
        let mut parts = self.directive_head_parts(d.text("style:"), dir.name_span, dir.modifiers);
        match &dir.value {
            internal::StyleDirectiveValue::True => {}
            internal::StyleDirectiveValue::ExpressionTag(tag) => {
                // Only include expression if not shorthand (style:color={color} → style:color)
                if !self.value_collapses_to_shorthand(&tag.expression, name, Some(tag.span)) {
                    parts.push(d.text("="));
                    parts.push(self.build_expression_tag_doc(tag));
                }
            }
            internal::StyleDirectiveValue::Parts(value_parts) => {
                let (open, close) = self.attribute_value_delims(value_parts);
                parts.push(open);
                for part in value_parts.iter() {
                    parts.push(self.build_attribute_value_doc(part));
                }
                parts.push(close);
            }
        }
        d.concat(&parts)
    }

    /// Build a Doc for use:action directive
    fn build_use_directive_doc(&self, dir: &internal::UseDirective<'_>) -> DocId {
        self.build_directive_doc(
            self.d().text("use:"),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
            DirectiveValue::Always,
        )
    }

    /// Build a Doc for transition/in/out directive
    fn build_transition_directive_doc(&self, dir: &internal::TransitionDirective<'_>) -> DocId {
        self.build_directive_doc(
            self.d().text(dir.direction.prefix_with_colon()),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
            DirectiveValue::Always,
        )
    }

    /// Build a Doc for animate:name directive
    fn build_animate_directive_doc(&self, dir: &internal::AnimateDirective<'_>) -> DocId {
        self.build_directive_doc(
            self.d().text("animate:"),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
            DirectiveValue::Always,
        )
    }

    /// Build a Doc for let:name directive
    fn build_let_directive_doc(&self, dir: &internal::LetDirective<'_>) -> DocId {
        self.build_directive_doc(
            self.d().text("let:"),
            dir.name_span,
            dir.modifiers,
            dir.expression.as_ref(),
            dir.expression_tag_span,
            DirectiveValue::Shorthand,
        )
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

    /// The value stage of an **unprefixed** `{…}` — an attribute value or an expression tag:
    /// [`Printer::build_head_value_doc`] under this context's `EmbedContext`, which sets
    /// `LayoutMode::Embedded` so a ROOT binary expression uses ContinuationIndent style.
    ///
    /// The embed starts from [`tsv_lang::EmbedContext::default`], not the host's `self.embed`
    /// — such a value is measured from its own `{`, not from the enclosing embedding. That is
    /// the one thing separating it from the prefixed heads' value stage.
    ///
    /// `host` carries the leading-JSDoc-cast verdict ([`UnprefixedHost`]): the expression
    /// tag hugs its braces, so it cannot hang the cast's own-line hardline and the break
    /// reflows; a directive value can take the block form, which gives the comment a
    /// properly indented line of its own, so the authoring survives. See
    /// [`Printer::build_head_value_doc`] and
    /// docs/conformance_prettier_svelte.md §Svelte: Own-line JSDoc cast at a braced head.
    ///
    /// Assignment expressions get the printer's clarity parens: `prop={(a = b)}`, on the
    /// frozen arm too — see [`Printer::wrap_value_clarity_parens`].
    fn build_unprefixed_value_doc(
        &self,
        expr: &Expression<'_>,
        frozen: bool,
        host: UnprefixedHost,
    ) -> DocId {
        let embed = tsv_lang::EmbedContext {
            mode: tsv_lang::LayoutMode::Embedded,
            jsdoc_cast_cannot_hang: host.cast_cannot_hang(),
            ..tsv_lang::EmbedContext::default()
        };
        let value_doc = self.build_head_value_doc(expr, frozen, &embed);
        self.wrap_value_clarity_parens(expr, value_doc)
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
        // The verdict comes back OUT of the content builder rather than going in — it
        // selects the value's doc AND its layout (block vs hug) below, and the two must
        // never disagree. See [`HeadExpr`]; one resolution, so there is no second to drift.
        let head =
            self.build_expression_content_with_comments(expr, tag_span, UnprefixedHost::Directive);

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

        // A run ENDING in a line comment already forces `}` onto its own line (its doc ends
        // in a hardline). Hug it directly — block structure would add its own softline
        // before `}`, leaving a stray blank line (`={\n\texpr // c\n\n}`). The question is
        // how the run ENDED, not whether it held a line comment anywhere: a block comment
        // after one (`{a // c⏎/* d */}`) leaves no break for the `}` to reuse, so that run
        // takes the ordinary block form. `head` carries the answer off the emitted run.
        let d = self.d();
        let inner = if head.layout.opens_own_line() {
            // Never hug a value whose content opens on its own line: the hug supplies its own
            // braces with no block to break, so the leading run would end up sharing the
            // `{`'s line — which relocates an own-line comment, and for a directive is inert
            // under the placement floor, losing the freeze on the next pass. The block itself
            // needs no forcing; the run's own hardline (see `build_leading_js_comment_doc`)
            // breaks the group from inside.
            self.wrap_in_block_structure(head.doc, head.ends_with_line_comment)
        } else if head.layout == HeadLayout::HangsAfterOpen {
            // The run stays on the `{`'s line, so the block form's softline — which breaks
            // BEFORE it — is the wrong shape; this is the same geometry with the break moved
            // past the run.
            self.wrap_hanging_head_braces(head)
        } else if is_hugged || head.ends_with_line_comment {
            // Hugged: the expression's internal doc handles wrapping — and this arm supplies
            // no indent of its own, so it is the one that owes the continuation indent.
            d.braces(self.hug_head_content(head))
        } else {
            // Block structure for other expressions
            self.wrap_in_block_structure(head.doc, false)
        };

        smallvec![d.text("="), inner]
    }

    /// Build expression content with leading/trailing comments — leading comments,
    /// the expression doc, trailing comments — paired with the
    /// [`Printer::honored_directive_in_gap`] verdict that produced them. An honored directive
    /// in the `{`→value gap freezes the value whole, and the caller owes it the block form,
    /// which is what keeps the directive on its own line. See [`HeadExpr`] for why the verdict
    /// travels back out rather than in.
    ///
    /// This is the **unprefixed** `{…}` value's head builder; the prefixed ones live in
    /// `nodes/helpers.rs` and return the same pair.
    ///
    /// A `tag_span` of `None` is a value with no braces: no gap to hold a directive, and no
    /// gap to hold a comment either, so the whole freeze-and-comment stage is the braced case.
    ///
    /// `host` — which unprefixed `{…}` host is building ([`UnprefixedHost`]), carrying the
    /// two per-host verdicts. Its `always_block` half (only `bind:`'s block-structure path)
    /// joins the freeze verdict as the [`Printer::trailing_comment_docs`]
    /// `closer_owns_break` question: an indented content's break cannot serve a closer
    /// sitting outside that indent. Its `cast_cannot_hang` half is forwarded to
    /// [`Self::build_unprefixed_value_doc`], whose doc carries that rule.
    ///
    /// A leading line comment hangs the value the same way
    /// ([`Printer::head_layout`]) and so answers that question too — but
    /// unlike the two above it cannot apply its own indent here, because which of this head's
    /// callers-of-a-caller supplies one is decided *after* this returns (`is_hugged` and
    /// `ends_with_line_comment` pick between hugged braces and block structure). It rides out
    /// on [`HeadExpr::owes_continuation_indent`] for whichever arm hugs.
    fn build_expression_content_with_comments(
        &self,
        expr: &Expression<'_>,
        tag_span: Option<Span>,
        host: UnprefixedHost,
    ) -> HeadExpr {
        let Some(span) = tag_span else {
            return HeadExpr {
                doc: self.build_unprefixed_value_doc(expr, false, host),
                layout: HeadLayout::Inline,
                ends_with_line_comment: false,
                owes_continuation_indent: false,
            };
        };
        // The `{`→value gap: everything from just past the brace to the value's first byte.
        let value_start = expr.span().start;
        let gap_start = span.start + 1;
        let frozen = self.honored_directive_in_gap(gap_start, value_start);

        let leading_comments = self.leading_comment_docs(gap_start, value_start);
        let expr_doc = self.build_unprefixed_value_doc(expr, frozen, host);
        // Every arm the caller can pick indents the content when this is true — the
        // block-wrapping ones by their own structure, the hugging ones by paying
        // `owes_continuation_indent` — so the closer's answer is the same either way, and it
        // is taken here, above the run.
        let layout = self.head_layout(gap_start, value_start, frozen);
        let (trailing_comments, ends_with_line_comment) = self.trailing_comment_docs(
            expr.span().end,
            span.end - 1,
            layout.indents_content() || host.always_block(),
        );

        HeadExpr {
            doc: self.concat_with_surrounding_comments(
                leading_comments,
                expr_doc,
                trailing_comments,
            ),
            layout,
            ends_with_line_comment,
            // An `OpensOwnLine` head takes the block form at every caller, whose `indent(…)`
            // IS the continuation indent — so the debt is already settled and claiming it
            // again would double the level. `HangsAfterOpen` keeps the debt: its caller emits
            // the `{`-line run itself and owes the indent below it
            // ([`Printer::wrap_hanging_head_braces`]).
            owes_continuation_indent: layout == HeadLayout::HangsAfterOpen && !host.always_block(),
        }
    }

    /// The [`HeadLayout::HangsAfterOpen`] twin of [`Self::wrap_in_block_structure`], for the
    /// unprefixed `{…}` hosts: `{ // c⏎\texpr⏎}`.
    ///
    /// Same geometry as the block form — content indented one level, closer on its own line at
    /// the brace's column — with the break moved **past** the run's first comment instead of
    /// before it, and a space in its place. The space is what the prefixed heads get for free
    /// from their opening literal (`{@html `, `{#if `); without it the comment welds to the
    /// delimiter (`{// c`) and reads as glued rather than as trailing the brace.
    ///
    /// `head.owes_continuation_indent` is the indent this arm pays
    /// ([`Printer::hug_head_content`]); a run-final `//` already ended the line, so the closer
    /// reuses that break rather than adding a second one.
    fn wrap_hanging_head_braces(&self, head: HeadExpr) -> DocId {
        let d = self.d();
        let ends_line = head.ends_with_line_comment;
        let content = self.hug_head_content(head);
        let close = if ends_line {
            d.text("}")
        } else {
            d.concat(&[d.hardline(), d.text("}")])
        };
        d.concat(&[d.text("{ "), content, close])
    }

    /// Wrap expression content in block structure: `{\n\texpr\n}`
    ///
    /// `content_ends_line` — the content's last emission is a line comment, so it already
    /// ended the line. The closing `}` then **reuses that break** instead of adding its own;
    /// a second one renders as a blank line above the `}`. This is the rule
    /// [`Printer::build_prefixed_head_doc`] applies one delimiter out, and the reason the
    /// answer travels on [`HeadExpr`] rather than being rescanned here: the run that was
    /// emitted is the only thing that knows how it ended.
    fn wrap_in_block_structure(&self, content: DocId, content_ends_line: bool) -> DocId {
        let d = self.d();
        let softline = d.softline();
        let inner = d.concat(&[softline, content]);
        let indented = d.indent(inner);
        let close = d.text("}");
        let concat = if content_ends_line {
            // The content's run-final `//` already broke the line, dedented to this level
            // (`build_trailing_js_comment_doc`), so the `}` reuses that break — a second
            // would render as a blank line above it.
            d.concat(&[d.text("{"), indented, close])
        } else {
            d.concat(&[d.text("{"), indented, softline, close])
        };
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
            // The per-operand path below is comment-blind, so any comment in the value —
            // leading (`{// c\n get, set}`), interior (`{get, /* c */ set}`), or trailing
            // (`{get, set /* c */}`) — is silently dropped there. Route the whole
            // comment-bearing case to the comment-aware builder.
            //
            // The gate spans the WHOLE value, `{` to `}`. Scanning only as far as the last
            // operand made a trailing comment invisible *to the gate*, so a document that had
            // one still took the comment-blind path and lost it — the rerouting inherits the
            // destination's blindness (docs/comments.md hazard 4). Prettier drops that comment,
            // but tsv preserves trailing comments at every other `{…}` value position
            // (conformance_prettier_svelte.md §Svelte: Attributes), and matching the drop here made
            // the sequence the one host that didn't.
            if let Some(span) = tag_span
                && self.has_comments_to_emit_between(span.start + 1, span.end - 1)
            {
                return smallvec![
                    d.text("="),
                    self.build_bind_sequence_with_comments_doc(seq, span),
                ];
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
            return smallvec![d.text("="), self.wrap_in_block_structure(items_doc, false)];
        }

        // For bind: directives, BinaryExpression should use block structure (not hugging).
        // This matches Prettier's behavior where bind: uses `={\n\texpr\n}` format.
        if let Expression::BinaryExpression(_) = expr {
            return self.build_expression_doc_parts_with_span_block_structure(expr, tag_span);
        }

        // For other expressions, use the standard method
        self.build_expression_doc_parts_with_span(expr, tag_span)
    }

    /// Build the bare (no-parens) function-binding sequence value when it carries any
    /// comment — leading, interior, or trailing — preserving each at the author's
    /// position. A line comment, or a multi-line block comment,
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
    /// comment bare and idempotent instead. A comment after the last operand is
    /// **preserved** where prettier deletes it — the trailing-position content loss
    /// tsv declines at every other `{…}` value, reaching the sequence host (see the
    /// `value_sequence_trailing_comment` divergence fixture).
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
        // An honored directive in the `{`→value gap leads the SEQUENCE node, not its first
        // operand, so the whole pair rides inside one verbatim slice — the value-head rule.
        // It stays bare: `bind:value={(get, set)}` is a grouped expression to Svelte, not a
        // getter/setter pair (see value_sequence_prettier_ignore_head_prettier_divergence).
        let head_frozen = self.honored_directive_in_gap(tag_span.start + 1, first_start);
        for comment in self.comments_to_emit_between(tag_span.start + 1, first_start) {
            if comment.is_block && comment.multiline {
                // Multi-line block: reindent-to-context through the shared comment
                // builder (matching `build_leading_js_comment_doc`), then a hardline
                // instead of the inline trailing space — the sequence's first operand
                // starts a fresh line, forcing the broken layout. `build_comment_doc`
                // tags the ledger itself.
                content.push(tsv_ts::build_comment_doc(d, comment, &self.ts_inputs()));
                content.push(d.hardline());
            } else {
                // Single-line block: `/*…*/ ` inline. Line comment: `//…` + hardline.
                content.push(self.build_leading_js_comment_doc(comment));
            }
        }

        if head_frozen {
            content.push(self.build_frozen_node_doc(seq.span));
            // The run PAST the slice, exactly as the unfrozen tail below emits it. A freeze
            // replaces the sequence's own doc; it does not own the gap between that doc's end
            // and the value's `}`, so returning here left that gap with no emitter at all —
            // the very hole the unfrozen tail names as its reason for existing. Scanning from
            // `seq.span.end` keeps it strictly outside the verbatim text, so a comment written
            // INSIDE the slice still rides in it rather than being printed twice.
            let (trailing_docs, ends_with_line_comment) =
                self.trailing_comment_docs(seq.span.end, tag_span.end - 1, true);
            content.extend(trailing_docs);
            return self.wrap_in_block_structure(d.concat(&content), ends_with_line_comment);
        }

        let mut items: DocBuf = DocBuf::new();
        for (i, sub_expr) in seq.expressions.iter().enumerate() {
            // Rule A: an honored directive in the comma gap freezes this operand. The
            // first operand has no such gap — a directive before it is the value head,
            // resolved as `head_frozen` above.
            let mut frozen = false;
            if i > 0 {
                let prev_end = seq.expressions[i - 1].span().end;
                let cur_start = sub_expr.span().start;
                frozen = self.honored_directive_in_gap(prev_end, cur_start);
                // The separator comma, located in source so a comment on either side
                // is attributed to the right operand (a comment's `,` can't fool it).
                let comma_pos =
                    find_char_skipping_comments(bytes, prev_end as usize, cur_start as usize, b',')
                        .map_or(prev_end, |c| c as u32);

                // Comments before the comma trail the previous operand.
                for comment in self.comments_to_emit_between(prev_end, comma_pos) {
                    items.push(self.build_trailing_js_comment_doc(comment, false));
                }

                items.push(d.text(","));

                // Comments after the comma: an all-block run leads the next operand
                // inline; otherwise the run is partitioned by the author's line
                // treatment — what shares the comma's line trails it, and from the first
                // OWN-LINE comment on the run leads the next operand on its own line.
                //
                // The partition is what keeps this gap from RELOCATING an own-line comment
                // up onto the comma's line (prettier keeps it own-line too) — and for an
                // honored directive the relocation is fatal, since a trailing placement is
                // inert, so the freeze would die on the next pass.
                let after: CommentRun<'_> = self
                    .comments_to_emit_between(comma_pos + 1, cur_start)
                    .collect();
                if after.is_empty() {
                    items.push(d.line());
                } else if after.iter().all(|c| c.is_block) {
                    items.push(d.line());
                    for comment in &after {
                        items.push(self.build_leading_js_comment_doc(comment));
                    }
                } else {
                    let mut pos = comma_pos;
                    let mut in_leading_run = false;
                    for comment in &after {
                        if !in_leading_run
                            && tsv_lang::printing::has_newline_between_scan(
                                self.source.as_bytes(),
                                self.line_table(),
                                pos,
                                comment.span.start,
                            )
                        {
                            in_leading_run = true;
                            items.push(d.hardline());
                        }
                        items.push(if in_leading_run {
                            self.build_leading_js_comment_doc(comment)
                        } else {
                            self.build_trailing_js_comment_doc(comment, false)
                        });
                        pos = comment.span.end;
                    }
                }
            }

            // Rule A resolved the operand's own freeze, so this is the ordinary value stage
            // under the host's embed — the operand is measured where it sits.
            items.push(self.build_head_value_doc(sub_expr, frozen, &self.embed));
        }

        // The operands sit in their own group so a forced break in the *surrounding*
        // `{ }` (a leading comment) doesn't break them — they break only when they
        // overflow or carry an interior forced break (a mid line comment, a block-body
        // arrow). Matches prettier.
        let items_doc = d.concat(&items);
        content.push(d.group(items_doc));

        // The run past the last operand — outside that group, so it never breaks the pair.
        // The whole reason this builder is reached for a trailing-only comment: the
        // comment-blind path has no emitter for this gap at all.
        let last_end = seq.expressions[seq.expressions.len() - 1].span().end;
        let (trailing_docs, ends_with_line_comment) =
            self.trailing_comment_docs(last_end, tag_span.end - 1, true);
        content.extend(trailing_docs);

        // Same bare block structure as the comment-free path: flat `{a, b}`, broken
        // `{\n\ta,\n\tb\n}`. Comment hardlines force the break; a lone inline block
        // comment leaves the operand group free to stay flat.
        self.wrap_in_block_structure(d.concat(&content), ends_with_line_comment)
    }

    /// Build Doc parts using block structure: `={\n\texpr\n}`
    ///
    /// Used for bind: directive expressions where Prettier always uses this format.
    fn build_expression_doc_parts_with_span_block_structure(
        &self,
        expr: &Expression<'_>,
        tag_span: Option<Span>,
    ) -> DocBuf {
        // Always the block form, so the freeze verdict changes nothing about the layout
        // here — and a leading cast's own-line comment keeps its line inside it.
        let head = self.build_expression_content_with_comments(
            expr,
            tag_span,
            UnprefixedHost::DirectiveBlock,
        );
        smallvec![
            self.d().text("="),
            self.wrap_in_block_structure(head.doc, head.ends_with_line_comment)
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
        // The same value-head content every unprefixed `{…}` builds — the tag always has its
        // braces, so the span is never absent. Only the assembly below is the tag's own: it
        // hugs its braces where an attribute value chooses between hug and block — which is
        // why a leading cast cannot hang here (`UnprefixedHost::Tag`, the reflow).
        let head = self.build_expression_content_with_comments(
            &tag.expression,
            Some(tag.span),
            UnprefixedHost::Tag,
        );

        if head.layout.opens_own_line() {
            // A value whose content opens on its own line takes the broken block form, which
            // supplies its own braces — so the leading run keeps the line the author gave it;
            // flush against the `{` an own-line comment is relocated, and a directive is inert
            // and its freeze lost on the second pass.
            return self.wrap_in_block_structure(head.doc, head.ends_with_line_comment);
        }
        if head.layout == HeadLayout::HangsAfterOpen {
            return self.wrap_hanging_head_braces(head);
        }
        d.concat(&[d.text("{"), self.hug_head_content(head), d.text("}")])
    }

    /// Whether an attribute prints as the shorthand `{name}` — its sole value part is an
    /// `ExpressionTag` that [`Printer::value_collapses_to_shorthand`] admits. The quoted
    /// spelling (`name="{name}"`) parses to the same single tag and so collapses too.
    fn is_shorthand_attribute(
        &self,
        attr: &internal::Attribute<'_>,
        value_parts: &[internal::AttributeValue<'_>],
    ) -> bool {
        let [internal::AttributeValue::ExpressionTag(expr_tag)] = value_parts else {
            return false;
        };

        self.value_collapses_to_shorthand(
            &expr_tag.expression,
            attr.name(self.source),
            Some(expr_tag.span),
        )
    }

    /// Whether a `name={name}` value may collapse to its **shorthand** form — `{name}` for a
    /// plain attribute, bare `name` for a `class:` / `bind:` / `let:` / `style:` directive.
    ///
    /// Two conjuncts, and the second is what keeps the collapse lossless: the value is an
    /// identifier spelling the attribute's own name, **and** no comment occupies the page
    /// inside the tag's braces. The shorthand has nowhere to put one, so collapsing over a
    /// comment DELETES it — a commented value therefore declines the collapse and prints the
    /// ordinary `{…}` form, the same bytes it already prints when the name and the identifier
    /// differ. Prettier collapses and drops; see `docs/conformance_prettier_svelte.md`
    /// §Svelte: Attributes.
    ///
    /// **on page**, not to-emit: a block comment glued to the identifier is *owned* by it and
    /// rides inside the expression's own doc, so an emit-keyed scan answers "no comment here"
    /// for exactly the leading position this rule exists for.
    ///
    /// `tag_span` is `None` where the author already wrote the bare form — the parser
    /// synthesizes the identifier and records no tag, so there are no braces to hold a
    /// comment and the collapse is a no-op.
    ///
    /// One predicate for all three collapse sites ([`Printer::is_shorthand_attribute`],
    /// [`Printer::build_directive_doc`] and the `style:` builder), so they cannot answer
    /// apart.
    fn value_collapses_to_shorthand(
        &self,
        expr: &Expression<'_>,
        name: &str,
        tag_span: Option<Span>,
    ) -> bool {
        matches!(expr, Expression::Identifier(id) if id.name(self.source) == name)
            && tag_span.is_none_or(|s| !self.has_comments_on_page_between(s.start, s.end))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_class_text;

    /// Every comment emitter in this crate prints a comment as its **whole span**, so the
    /// span must reproduce the delimiters the old per-part spelling assembled by hand:
    /// `//` + content for a line comment, `/*` + content + `*/` for a block. That is pure
    /// span arithmetic in the parser (`content_start = start + 2`, shared `end` for a line
    /// comment; `content_end = end - 2` for a block), and **no corpus can grade it** — a
    /// content span off by one still yields plausible output, and the formatter would
    /// simply start emitting different bytes. This is the check that would fail first.
    #[test]
    fn a_comment_span_reproduces_its_own_delimiters() {
        // Both payloads in every position a JS comment reaches: template expression,
        // block head, attribute value, attribute gap, `<script>` island, and a multi-line
        // block (whose span crosses a newline).
        let sources = [
            "{a // c\n}",
            "{a /* c */}",
            "{#if x // c\n}\n\ty\n{/if}",
            "<div class={x /* c */}></div>",
            "<div id=\"a\" // c\n></div>",
            "<div /* c */ id=\"a\"></div>",
            "<script>\n\tlet a = 1; // c\n</script>",
            "{@const a = b /* m1\n\t * m2 */}",
            "{@debug a /* c */}",
        ];
        for source in sources {
            let arena = bumpalo::Bump::new();
            let root = crate::parse(source, &arena)
                .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
            assert!(
                !root.comments.is_empty(),
                "no comment parsed from {source:?} — the case stopped exercising the rule"
            );
            for comment in &root.comments {
                let whole = comment.span.extract(source);
                let content = comment.content_span.extract(source);
                let rebuilt = if comment.is_block {
                    format!("/*{content}*/")
                } else {
                    format!("//{content}")
                };
                assert_eq!(
                    whole, rebuilt,
                    "comment span must equal delimiters + content, in {source:?}"
                );
            }
        }
    }

    /// The `<!-- -->` twin of the rule above, for the two HTML-comment emitters
    /// (`build_html_comment_doc` and the hoisted-section `print_comment`).
    ///
    /// It also pins the assumption those emitters now rest on: `parse_comment`'s
    /// `token_value.len() >= 7` guard has a fallback that collapses `content_span` to
    /// EMPTY, and a re-assembling emitter would then print `<!---->` for a shorter source
    /// run — a content rewrite. The guard is unreachable (`<!-->` and `<!--->` both fail
    /// as unterminated, so the shortest comment that parses is the 7-byte `<!---->`), and
    /// the degenerate cases below are here to fail loudly if that ever stops being true.
    #[test]
    fn an_html_comment_span_reproduces_its_own_delimiters() {
        use crate::ast::internal::FragmentNode;

        let sources = [
            "<!--c-->",
            "<!-- c -->",
            "<!---->",
            "<!----->",
            "<!--\n\tmulti\n\tline\n-->",
            "<div><!--nested--></div>",
        ];
        for source in sources {
            let arena = bumpalo::Bump::new();
            let root = crate::parse(source, &arena)
                .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
            let mut seen = 0;
            let mut stack = vec![&root.fragment];
            while let Some(fragment) = stack.pop() {
                for node in fragment.nodes {
                    match node {
                        FragmentNode::Comment(comment) => {
                            seen += 1;
                            let whole = comment.span.extract(source);
                            let content = comment.content_span.extract(source);
                            assert_eq!(
                                whole,
                                format!("<!--{content}-->"),
                                "HTML comment span must equal delimiters + content, in {source:?}"
                            );
                        }
                        FragmentNode::Element(el) => stack.push(&el.fragment),
                        _ => {}
                    }
                }
            }
            assert_eq!(
                seen, 1,
                "expected exactly one HTML comment in {source:?} — the case stopped exercising the rule"
            );
        }
    }

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
    fn separator_space_is_keyed_on_the_last_line() {
        // A continuation line's indentation before `{expr}` is leading whitespace, not
        // a separator: it stays as authored (a separator would grow it every pass).
        assert_eq!(normalize_class_text("a b\n    ", false), "a b\n    ");
        assert_eq!(normalize_class_text("a b\n\t", false), "a b\n\t");
        // Trailing whitespace before the newline is still dropped on the way.
        assert_eq!(normalize_class_text("a b  \n    ", false), "a b\n    ");
        // Content on the last line keeps the separator.
        assert_eq!(normalize_class_text("a\n  b ", false), "a\n  b ");
    }

    #[test]
    fn all_whitespace_passes_through() {
        // No non-whitespace ⇒ the separator-space rule doesn't apply.
        assert_eq!(normalize_class_text(" ", false), " ");
        assert_eq!(normalize_class_text("", true), "");
    }
}
