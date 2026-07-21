// Single-comment text-layout leaves.
//
// These render one comment's text: line / block / hashbang docs, the multi-line
// block-comment framing (indentable JSDoc vs preserved interior layout), and the
// trailing line/block comment docs (`line_suffix` vs inline).

use super::Printer;
use crate::ast::internal;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing;

/// Slice one line out of a comment body by its `(start, end)` byte range — an
/// entry of the arena's line-spans scratch, filled by `build_comment_doc`.
#[inline]
fn line_slice(content: &str, (start, end): (u32, u32)) -> &str {
    &content[start as usize..end as usize]
}

impl<'a> Printer<'a> {
    /// Build a Doc for a single comment
    ///
    /// For multi-line block comments:
    /// - JSDoc comments (/**) always use hardline to apply context indent
    /// - Other comments: if continuation lines had indentation, use hardline; otherwise literalline
    pub(crate) fn build_comment_doc(&self, comment: &internal::Comment) -> DocId {
        let d = self.d();
        let content = comment.content(self.source);
        let doc = if comment.is_block {
            // Block comment: /* content */
            if !comment.multiline {
                // Single-line block comment — the full span is verbatim `/*…*/`,
                // so emit it as a source slice (no allocation).
                d.source_span(comment.span, self.source)
            } else {
                // One `split('\n')` pass fills the arena-parked line-offset
                // scratch (capacity retained across comments and files); the
                // classifier and builder then iterate the lines slice-cheap
                // with no per-comment line buffer.
                let mut line_spans = d.borrow_line_spans_scratch();
                let mut start = 0u32;
                for line in content.split('\n') {
                    let end = start + line.len() as u32;
                    line_spans.push((start, end));
                    start = end + 1; // step over the '\n'
                }
                let lines = line_spans.iter().map(|span| line_slice(content, *span));
                if printing::is_indentable_block_comment(lines) {
                    self.build_indentable_block_comment_doc(content, &line_spans)
                } else {
                    self.build_preserved_block_comment_doc(content, &line_spans)
                }
            }
        } else if comment.span.start == 0 && content.starts_with("#!") {
            // Hashbang comment: #!/usr/bin/env node (no // prefix). The span is
            // verbatim (content includes the #! prefix). Like a line comment it
            // runs to end-of-line, so tag it for the swallow check.
            d.line_comment_source_span(comment.span, self.source)
        } else {
            // Line comment: // content. The full span is verbatim `//…`. Tagged
            // for the swallow check (runs to EOL).
            d.line_comment_source_span(comment.span, self.source)
        };

        // The single comment-emission seam in this crate — every leading / trailing / gap
        // / owned comment routes through here, including the JSDoc cast's, which prints
        // from the copy on its `JsdocCast` node rather than from the shared array (the
        // ledger keys on the span, so the copy lands on the same entry). The renderer
        // records the emit when it reaches the node.
        #[cfg(feature = "comment_check")]
        d.tag_comment_doc(doc, comment.span, self.source);

        doc
    }

    /// Build a multi-line *indentable* block comment (JSDoc `/** … */` and
    /// `*`-aligned `/* … */`, where every line begins with `*`).
    ///
    /// Continuation lines are reindented to a single leading space before the
    /// `*` — the context indent is supplied by the per-line hardline (here baked
    /// into a [`DocNode::MultilineText`]), and content after the `*` is
    /// untouched. Mirrors prettier's `printIndentableBlockComment`.
    fn build_indentable_block_comment_doc(
        &self,
        content: &str,
        line_spans: &[(u32, u32)],
    ) -> DocId {
        let d = self.d();
        // ≥2 lines: `build_comment_doc` only routes newline-containing content
        // here, with `line_spans` holding each line's byte range in `content`.
        #[allow(clippy::unreachable)] // content has a newline ⇒ split yields ≥2 lines
        let [first, middle @ .., last] = line_spans else {
            unreachable!("multi-line comment");
        };
        let line = |span: &(u32, u32)| line_slice(content, *span);

        // Frame the whole comment as one `\n`-separated body — the `/*<first>`
        // opener, each continuation line reindented to a single leading space,
        // the `*/` closer — and emit it as a single `MultilineText` node, which
        // renders each `\n` as a context-indented hardline. Byte- and
        // position-identical to the former `concat([text, hardline, text, …])`,
        // streamed through the arena's pool writer (no transient `String`).
        //
        // The reserve is an exact upper bound, so the push sequence never
        // reallocs: `content` already holds every line's text and the interior
        // `\n`s; framing adds `/*` + `*/` (4) and at most one leading space per
        // line (`line_spans.len()`), and the per-line trims only ever remove bytes.
        let mut body = d.pool_writer();
        body.reserve(content.len() + line_spans.len() + 4);
        body.push_str("/*");
        body.push_str(line(first).trim_end());
        for span in middle {
            body.push('\n');
            body.push(' ');
            body.push_str(line(span).trim());
        }
        // The last line (before `*/`) keeps trailing content via `trim_start`.
        body.push('\n');
        body.push(' ');
        body.push_str(line(last).trim_start());
        body.push_str("*/");

        body.finish_multiline_text()
    }

    /// Build a multi-line *non-indentable* block comment (at least one line does
    /// not begin with `*`) — its interior layout preserved **verbatim**.
    ///
    /// Every continuation line renders through `literalline` (a newline with **no**
    /// context indent), so the comment's interior columns are kept exactly as
    /// authored, matching prettier's non-indentable-block-comment handling. This is
    /// idempotent by construction: because no context indent is added, a comment
    /// whose source interior is indented never compounds that indentation one level
    /// per format pass. (The former behavior re-applied context indent via
    /// `hardline` after stripping the comment's *start-line* indent — but when the
    /// comment renders at a different depth than its source line, e.g. a multi-line
    /// comment in a `for(…)` header that breaks, the stripped amount and the
    /// re-applied context indent differ, so the interior grew a tab every pass — an
    /// F1 non-idempotency.)
    fn build_preserved_block_comment_doc(&self, content: &str, line_spans: &[(u32, u32)]) -> DocId {
        let d = self.d();

        // ≥2 lines: `build_comment_doc` only routes newline-containing content
        // here, with `line_spans` holding each line's byte range in `content`.
        #[allow(clippy::unreachable)] // content retains the newline ⇒ split yields ≥2 lines
        let [first, middle @ .., last] = line_spans else {
            unreachable!("multi-line comment");
        };
        let line = |span: &(u32, u32)| line_slice(content, *span);

        // Frame directly: the `/*<first>` opener, each continuation line preserved
        // verbatim at its authored column via a `literalline` (no context indent),
        // then the `*/` closer. Trailing whitespace is trimmed on each interior
        // line (matches prettier); the final line keeps its content before `*/`.
        let mut docs = DocBuf::with_capacity((middle.len() + 1) * 2 + 2);
        let mut opener = d.pool_writer();
        opener.push_str("/*");
        opener.push_str(line(first).trim_end());
        docs.push(opener.finish_text());
        for span in middle {
            docs.push(d.literalline());
            docs.push(d.text_pooled(line(span).trim_end()));
        }
        docs.push(d.literalline());
        docs.push(d.text_pooled(line(last)));
        docs.push(d.text("*/"));
        d.concat(&docs)
    }

    /// Build a line_suffix doc for a trailing line comment (space + comment)
    ///
    /// Wrapping in line_suffix excludes the comment from width calculations,
    /// so elements can stay compact even when the trailing comment would push
    /// the line over print_width.
    pub(crate) fn build_trailing_line_comment_doc(&self, comment: &internal::Comment) -> DocId {
        let d = self.d();
        d.line_suffix(d.concat(&[d.text(" "), self.build_comment_doc(comment)]))
    }

    /// Build a doc for a single trailing comment (`expr /* c */` or `expr; // c`).
    ///
    /// A **block** comment is inline with a leading space — its width counts toward
    /// the line. A **line** comment goes through `line_suffix` (zero width), so a
    /// long trailing comment never forces a preceding group (e.g. a member's union
    /// type) to break. Shared by every spot that trails a comment on a member or
    /// inner type without semicolon-relative positioning.
    pub(crate) fn build_trailing_comment_doc(&self, comment: &internal::Comment) -> DocId {
        if comment.is_block {
            let d = self.d();
            d.concat(&[d.text(" "), self.build_comment_doc(comment)])
        } else {
            self.build_trailing_line_comment_doc(comment)
        }
    }
}
