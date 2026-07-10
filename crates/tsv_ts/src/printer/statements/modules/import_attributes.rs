// Import-attribute clause printing for module statements: the `with { … }`
// clause shared by import declarations and the two re-export hosts, plus the
// per-attribute key/value docs.

use super::Printer;
use crate::ast::internal;
use smallvec::smallvec;
use tsv_lang::comments_in_range;
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
            for comment in comments_in_range(self.comments, brace_start + 1, brace_close) {
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
            // Check for line comments between/around attributes (force multiline)
            let has_line_comments =
                self.has_line_comments_in_delimited_list(attributes, |a| a.span, stmt_end)
                    || self.has_line_comments_between(brace_start + 1, attributes[0].span.start);
            if has_line_comments {
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
                // Own group so the braces fit independently of the outer statement —
                // a preserved header line comment (source→`with` / `with`→`{`) forces
                // the outer group to break but must not expand inline attributes.
                self.braced_group(attr_doc)
            }
        };

        // Header-gap comments (source→`with`, `with`→`{`), preserved in place.
        // Prettier keeps a source→`with` block inline, relocates a `with`→`{` block
        // before `with`, floats a `with`→`{` line past `;`, and *throws* on a
        // source→`with` line — so it can't be the oracle here; see
        // with_keyword_comment*_prettier_divergence and conformance_prettier.md.
        // A line comment forces its following token onto a new line, which the
        // shared helper indents one level (statement continuation).
        let with_clause = d.concat(&[
            d.text("with"),
            self.gap_comment_indented_continuation(with_end, brace_start, brace_doc),
        ]);
        parts.push(self.gap_comment_indented_continuation(source_end, with_start, with_clause));
        brace_close + 1
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

        // Check for comments between key and value (around the `:`)
        let has_comments = comments_in_range(self.comments, key_end, value_start)
            .next()
            .is_some();

        if !has_comments {
            // Fast path: no comments
            return d.concat(&[
                self.build_import_attribute_key_doc(&attr.key),
                d.text(": "),
                self.build_literal_doc(&attr.value),
            ]);
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

        let mut parts = smallvec![self.build_import_attribute_key_doc(&attr.key)];

        // Comments between key and `:`
        self.append_trailing_inline_block_comments(&mut parts, key_end, colon_pos);

        parts.push(d.text(":"));

        // Comments between `:` and value
        let after_colon = colon_pos + 1;
        parts.push(d.text(" "));
        for comment in comments_in_range(self.comments, after_colon, value_start) {
            if comment.is_block {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
        }

        parts.push(self.build_literal_doc(&attr.value));

        d.concat(&parts)
    }
}
