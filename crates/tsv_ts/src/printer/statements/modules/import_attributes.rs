// Import-attribute clause printing for module statements: the `with { … }`
// clause shared by import declarations and the two re-export hosts, plus the
// per-attribute key/value docs.

use super::Printer;
use crate::ast::internal;
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    /// Append the `with { … }` import-attributes clause to `parts`, if any, and
    /// return the byte offset just past the clause's closing `}` — the anchor for
    /// the caller's trailing pre-`;` comment scan (`source_end` when there is no
    /// `with` clause).
    ///
    /// Shared by the import declaration and the two re-export hosts
    /// (`export { … } from …` and `export * from …`). `attributes` is `None`
    /// when there is no `with` clause (nothing emitted) and `Some(_)` when one
    /// is present — `Some([])` emits an empty `with {}` (preserved to match
    /// acorn/prettier). `source_end` is the byte offset just past the source
    /// string literal — where the `with` keyword search begins; `stmt_end` is
    /// the declaration's span end (the `;`). PR #17329 (prettier 3.7): break
    /// attributes across lines when long.
    pub(super) fn push_import_attributes_clause(
        &self,
        parts: &mut DocBuf,
        attributes: Option<&[internal::ImportAttribute<'_>]>,
        source_end: u32,
        stmt_end: u32,
    ) -> u32 {
        let Some(attributes) = attributes else {
            return source_end;
        };
        let d = self.d();
        // Find the `with` keyword and the opening brace (forward search skips
        // `{`/keyword text inside comments). The search upper bound is the first
        // attribute, or the statement end for an empty `with {}`.
        let header_bound = attributes.first().map_or(stmt_end, |a| a.span.start);
        let with_end = self
            .find_keyword_end("with", source_end, header_bound)
            .unwrap_or(source_end);
        let with_start = with_end - "with".len() as u32;
        let brace_start = self
            .find_char_outside_comments(with_end, header_bound, b'{')
            .unwrap_or(0);
        // The closing `}`: the first `}` (outside comments) at or after the last
        // attribute — or after the opening `{` for an empty `with {}`. Returned
        // (plus one) as the content end for the trailing comment scan.
        let close_search_start = attributes.last().map_or(brace_start, |a| a.span.end);
        let brace_close = self.close_brace_offset(close_search_start, stmt_end);

        // Build the `{…}` clause doc (kept as a local so the comment-forced-break
        // handling below can wrap it in `indent`).
        let brace_doc = if attributes.is_empty() {
            // Empty `with {}` — preserved (acorn/prettier keep it). A comment between
            // the braces is kept in place (`with {/* c */}`); prettier instead
            // relocates it before `with` — a comment-position divergence, like the
            // `with`→`{` gap. See attributes_empty_comment_prettier_divergence.
            let mut inner: DocBuf = smallvec![d.text("{")];
            let mut last_was_line = false;
            let mut any_comment = false;
            for comment in comments_to_emit_in_range(self.comments, brace_start + 1, brace_close) {
                if any_comment {
                    inner.push(d.text(" "));
                }
                inner.push(self.build_comment_doc(comment));
                last_was_line = !comment.is_block;
                any_comment = true;
            }
            // A trailing line comment would swallow the `}`; push it to a new line.
            if last_was_line {
                inner.push(d.hardline());
            }
            inner.push(d.text("}"));
            d.concat(&inner)
        } else {
            // Expanding comments force the multiline path — line comments anywhere in the
            // list, and block comments the author isolated on their own line between the
            // braces (collapsing `with {⏎ /* c */⏎ type: 'json' }` inline would move the
            // comment). The attribute brace formerly asked only about line comments.
            let has_expanding_comments =
                self.has_line_comments_in_delimited_list(attributes, |a| a.span, stmt_end)
                    || self.has_line_comments_between(brace_start + 1, attributes[0].span.start)
                    || self.has_own_line_attribute_comments(attributes, brace_start, brace_close);
            if has_expanding_comments {
                // A same-line comment trailing the `with {` brace stays on the
                // brace line, like the import/export specifier brace
                // (`import { // c`). Prettier floats it past `;`; tsv keeps the
                // author's placement. See
                // with_open_brace_line_comment_prettier_divergence.
                self.build_braced_hardline_comma_list(
                    attributes,
                    brace_start,
                    stmt_end,
                    attributes[0].span.start,
                    |a| a.span,
                    |a| self.build_import_attribute_doc(a),
                )
            } else {
                let attr_doc = self.build_softline_comma_list(
                    attributes,
                    brace_start,
                    brace_close,
                    |a| a.span,
                    |a| self.build_import_attribute_doc(a),
                );
                if self.is_single_type_attribute(attributes, brace_start, brace_close) {
                    // Ordered FIRST because prettier applies `removeLines` *after*
                    // `printObject`: the never-break rule outranks the authored break
                    // below, so `with {⏎ type: 'json'⏎}` still collapses.
                    self.braced_flat(attr_doc)
                } else if self.has_newline_between(brace_start + 1, attributes[0].span.start) {
                    // `objectWrap: 'preserve'` — prettier's `hasNewLineAfterOpeningBrace`
                    // in `printObject`, which it applies to the attribute clause too
                    // (locating this same real `{`). A newline the author put after
                    // `with {` keeps the clause expanded even when it would fit, exactly
                    // as it does for an object literal (`build_object_doc`'s
                    // `has_source_newline`). A newline anywhere else in the clause — a
                    // broken import header above it, or one before the `}` — does not.
                    self.braced_group_break(attr_doc)
                } else {
                    // Own group so the braces fit independently of the outer statement —
                    // a preserved header line comment (source→`with` / `with`→`{`) forces
                    // the outer group to break but must not expand inline attributes.
                    self.braced_group(attr_doc)
                }
            }
        };

        // Header-gap comments (source→`with`, `with`→`{`), preserved in place.
        // Prettier keeps a source→`with` block inline, relocates a `with`→`{` block
        // before `with`, floats a `with`→`{` line past `;`, and *throws* on a
        // source→`with` line — so it can't be the oracle here; see
        // with_keyword_comment*_prettier_divergence and conformance_prettier_ts_comments.md.
        // A line comment forces its following token onto a new line, which the
        // shared helper indents one level (statement continuation).
        let with_clause = d.concat(&[
            d.text("with"),
            self.gap_comment_indented_continuation(with_end, brace_start, brace_doc),
        ]);
        parts.push(self.gap_comment_indented_continuation(source_end, with_start, with_clause));
        brace_close + 1
    }

    /// Prettier's `isSingleTypeImportAttributes` (its `printImportAttributes`): a lone
    /// `type: '…'` attribute — the `with { type: 'json' }` that is nearly every real
    /// clause — is printed through `removeLines`, so it never breaks, at any width.
    /// A second attribute, a non-`type` key, a non-string value, or a comment on the
    /// attribute all fall back to the ordinary breakable group.
    ///
    /// The key matches in either authored form (`type` / `'type'`), so the quoted one
    /// reaches the flat layout on the very pass that unquotes it rather than settling
    /// over two.
    ///
    /// Comments: prettier asks `hasComment` of the attribute, its key and its value —
    /// between the braces, and nowhere else. The `with`→`{` gap is deliberately not
    /// included (prettier relocates such a comment before `with` and still flattens),
    /// nor is the `}`→`;` gap. The question is ON-PAGE, so a glued block comment owned
    /// by the value counts, exactly as prettier's `hasComment(value)` does.
    fn is_single_type_attribute(
        &self,
        attributes: &[internal::ImportAttribute<'_>],
        brace_start: u32,
        brace_close: u32,
    ) -> bool {
        let [attribute] = attributes else {
            return false;
        };
        if !matches!(attribute.value.value, internal::LiteralValue::String(_)) {
            return false;
        }
        if self.has_comments_on_page_between(brace_start, brace_close + 1) {
            return false;
        }
        match &attribute.key {
            internal::ImportAttributeKey::Identifier(id) => {
                self.with_ident_name(id, |name| name == "type")
            }
            internal::ImportAttributeKey::Literal(lit) => match &lit.value {
                internal::LiteralValue::String(cooked) => {
                    cooked.resolve(lit.span, self.source) == "type"
                }
                _ => false,
            },
        }
    }

    /// Whether a comment in an attribute's `:`→value gap was authored on its OWN line —
    /// isolated from both the `:` before it and the value after it
    /// ([`Self::comment_isolated_from_neighbors`]). Such a comment can only keep its line
    /// if the value hangs, so it forces the same break-after-`:` layout a line comment
    /// does; collapsing it onto the `:` line would move it.
    ///
    /// Asked here rather than by [`Self::has_own_line_attribute_comments`], which skips
    /// everything *inside* an attribute's span: a comment there expands that attribute,
    /// not the whole clause. The `:`→value gap is one of the two regions it so skips —
    /// the key→`:` gap is the other, and takes the inline-trailing path instead.
    fn has_own_line_value_gap_comment(&self, colon_pos: u32, value_start: u32) -> bool {
        tsv_lang::comments_in_source_range(self.comments, colon_pos + 1, value_start)
            .any(|c| self.comment_isolated_from_neighbors(colon_pos, c, value_start))
    }

    /// Whether any comment in the `with { … }` braces was authored on its OWN line —
    /// isolated from both the attribute (or brace) before it and the one after
    /// ([`Self::comment_isolated_from_neighbors`]). Such a comment can only keep its line if
    /// the list breaks, so it is an expansion trigger.
    ///
    /// Spelled here rather than reusing `has_own_line_block_comments_in_bracket_list`: that
    /// predicate looks for the following element with a strict `>`, so a comment GLUED to the
    /// attribute after it (`/* c */attr`, whose end is exactly that attribute's start) reads
    /// as a dangling trailing comment and expands — and the expanded form reprints with the
    /// separating space, which then collapses again (non-idempotent).
    fn has_own_line_attribute_comments(
        &self,
        attributes: &[internal::ImportAttribute<'_>],
        brace_start: u32,
        brace_close: u32,
    ) -> bool {
        tsv_lang::comments_in_source_range(self.comments, brace_start + 1, brace_close).any(|c| {
            let inside_attribute = attributes
                .iter()
                .any(|a| c.span.start >= a.span.start && c.span.end <= a.span.end);
            if inside_attribute {
                return false;
            }
            let prev = attributes
                .iter()
                .map(|a| a.span.end)
                .take_while(|&end| end <= c.span.start)
                .last()
                .unwrap_or(brace_start);
            let next = attributes
                .iter()
                .map(|a| a.span.start)
                .find(|&start| start >= c.span.end)
                .unwrap_or(brace_close);
            self.comment_isolated_from_neighbors(prev, c, next)
        })
    }

    /// Build doc for an import attribute key: a bare identifier emits verbatim;
    /// a string-literal key follows prettier's `quoteProps: as-needed` — quotes
    /// are stripped when the value is a valid identifier with no escapes
    /// (`'type'` → `type`), else kept and normalized (`'resolution-mode'`).
    fn build_import_attribute_key_doc(&self, key: &internal::ImportAttributeKey<'_>) -> DocId {
        match key {
            internal::ImportAttributeKey::Identifier(id) => self.identifier_name_doc(id),
            internal::ImportAttributeKey::Literal(lit) => {
                if let internal::LiteralValue::String(cooked) = &lit.value {
                    let content = cooked.resolve(lit.span, self.source);
                    self.build_string_literal_key_doc(lit, content)
                } else {
                    // Attribute keys are only identifiers or string literals.
                    self.build_literal_doc(lit)
                }
            }
        }
    }

    /// Build doc for a single import attribute: `key: value`
    ///
    /// Handles comments between key, `:`, and value.
    fn build_import_attribute_doc(&self, attr: &internal::ImportAttribute<'_>) -> DocId {
        let d = self.d();
        let key_end = attr.key.span().end;
        let value_start = attr.value.span.start;
        let key_doc = self.build_import_attribute_key_doc(&attr.key);
        let value_doc = self.build_literal_doc(&attr.value);

        // Fast path: no comments around the `:`. The **on-page** gate short-circuits the
        // layout work below (an owned glued block comment on the value is printed by the
        // value's own doc, so it never needs the layout branches).
        if !self.has_comments_on_page_between(key_end, value_start) {
            return d.concat(&[key_doc, d.text(": "), value_doc]);
        }

        // Find `:` position to split comments
        #[allow(clippy::expect_used)] // colon must exist when key and value are present
        let colon_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            key_end as usize,
            value_start as usize,
            b':',
        )
        .expect("colon must exist in import attribute") as u32;

        // A line comment in the key→`:` gap stays trailing the key; `: value` drops to
        // a continuation line indented one level (`type // c⏎\t: 'json'`) — the uniform
        // forced-continuation indent shared with the object-property / type-member
        // key→`:` line-comment paths. Prettier instead relocates the comment past the
        // value to trail it. See attributes_key_colon_line_comment_prettier_divergence.
        if self.has_line_comments_between(key_end, colon_pos) {
            let value_doc = self.prepend_rhs_comments(value_doc, colon_pos + 1, value_start);
            let tail = d.concat(&[d.text(": "), value_doc]);
            return d.concat(&[
                key_doc,
                self.build_continuation_indent(key_end, colon_pos, tail),
            ]);
        }

        let mut parts = smallvec![key_doc];

        // Block comments between the key and `:` trail the key inline (`type /* c */:`).
        self.append_trailing_inline_block_comments(&mut parts, key_end, colon_pos);

        parts.push(d.text(":"));

        // Comments between `:` and the value.
        let after_colon = colon_pos + 1;
        if self.has_line_comments_between(after_colon, value_start)
            || self.has_own_line_value_gap_comment(colon_pos, value_start)
        {
            // A line comment can't trail the `:` inline without swallowing the
            // value, so break after `:` and indent the value one level, each gap
            // comment on its own line — `type:⏎\t// c⏎\t'json'`. Matches prettier's
            // value-leading break, which puts any block comment sharing the gap on
            // its own line too. A block comment the author isolated on its own line
            // takes the same layout, for the same reason it does in an object
            // property: collapsing it onto the `:` line would move it.
            let mut cont: DocBuf = smallvec![d.hardline()];
            for comment in comments_to_emit_in_range(self.comments, after_colon, value_start) {
                cont.push(self.build_comment_doc(comment));
                cont.push(d.hardline());
            }
            cont.push(value_doc);
            parts.push(d.indent(d.concat(&cont)));
        } else {
            // Block comments only (or none): trail the `:` inline (` /* c */ `),
            // matching prettier when the attribute fits on one line.
            parts.push(d.text(" "));
            for comment in comments_to_emit_in_range(self.comments, after_colon, value_start) {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
            parts.push(value_doc);
        }

        d.concat(&parts)
    }
}
