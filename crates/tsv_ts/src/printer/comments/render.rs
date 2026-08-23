// Single-comment text-layout leaves.
//
// These render one comment's text: line / block / hashbang docs, the multi-line
// block-comment framing (indentable JSDoc vs preserved interior layout), and the
// trailing line/block comment docs (`line_suffix` vs inline).

use super::Printer;
use crate::ast::internal;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::is_js_whitespace;
use tsv_lang::printing;

/// Slice one line out of a comment body by its `(start, end)` byte range — an
/// entry of the arena's line-spans scratch, filled by `build_comment_doc`.
#[inline]
fn line_slice(content: &str, (start, end): (u32, u32)) -> &str {
    &content[start as usize..end as usize]
}

/// The trailing-trim class every comment emitter here shares.
///
/// ⚠️ [`is_js_whitespace`], **not** `str::trim_end`: prettier trims comment text with
/// `String.prototype.trim*` (`printComment`, `printIndentableBlockComment`), which is the JS
/// `\s` class. Rust's `White_Space` disagrees at exactly two code points and got both wrong —
/// it **deletes a U+0085** prettier keeps (a character removed from the author's comment) and
/// **keeps a U+FEFF** prettier deletes. See [`is_js_whitespace`].
#[inline]
fn trim_comment_end(line: &str) -> &str {
    line.trim_end_matches(is_js_whitespace)
}

/// The leading half of [`trim_comment_end`], same class and same reason.
#[inline]
fn trim_comment_start(line: &str) -> &str {
    line.trim_start_matches(is_js_whitespace)
}

/// Both halves — prettier's `line.trim()` on an indentable comment's interior line.
#[inline]
fn trim_comment(line: &str) -> &str {
    line.trim_matches(is_js_whitespace)
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
        } else {
            // Line comment (`//…`) or hashbang (`#!…`, which carries no `//` prefix and can
            // only sit at byte 0). Both are verbatim source spans and both run to end of
            // line, so both are tagged for the swallow check and both take the same
            // trailing trim — prettier's `isLineComment` covers the hashbang types too, so
            // one arm answers for both rather than two arms that must be kept identical.
            d.line_comment_source_span(self.line_comment_span(comment), self.source)
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

    /// A line-like comment's span with its trailing whitespace cut off.
    ///
    /// prettier's `printComment` emits `originalText.slice(locStart, locEnd).trimEnd()` for
    /// every comment that runs to end of line (`//` and `#!` alike), so the trim is part of
    /// the comment's text, not of the layout.
    ///
    /// ⚠️ It cannot be left to the renderer's own line-end trim
    /// (`tsv_lang`'s `doc/arena_render.rs`), which is `[' ', '\t']` — prettier's doc-printer
    /// `trim`, correctly mirrored, and a *narrower* class on purpose. Every other JS `\s`
    /// member (NBSP, U+FEFF, a form feed, the U+2000 family) passed straight through it and
    /// rode out in tsv's output where prettier deletes it. Trimming the SPAN rather than the
    /// text keeps the emit allocation-free; the ledger still tags the comment's full span.
    fn line_comment_span(&self, comment: &internal::Comment) -> Span {
        let text = comment.span.extract(self.source);
        Span {
            start: comment.span.start,
            end: comment.span.start + trim_comment_end(text).len() as u32,
        }
    }

    /// Build a multi-line *indentable* block comment (JSDoc `/** … */` and
    /// `*`-aligned `/* … */`, where every line begins with `*`).
    ///
    /// Continuation lines are reindented to a single leading space before the
    /// `*` — the context indent is supplied by the per-line hardline (here baked
    /// into a [`tsv_lang::doc::arena::DocNode::MultilineText`]), and content after the `*` is
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
        body.push_str(trim_comment_end(line(first)));
        for span in middle {
            body.push('\n');
            body.push(' ');
            body.push_str(trim_comment(line(span)));
        }
        // The last line (before `*/`) keeps trailing content via `trim_start`.
        body.push('\n');
        body.push(' ');
        body.push_str(trim_comment_start(line(last)));
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
        let Some((first, rest)) = line_spans.split_first() else {
            unreachable!("multi-line comment");
        };
        // `split_first` only refuses an EMPTY slice, where the `[first, .., last]` pattern this
        // replaced also refused a ONE-line one — so the ≥2 half of the invariant is asserted
        // rather than pattern-enforced. A one-line slice would print a correct `/*…*/` here and
        // hide the routing bug that produced it.
        debug_assert!(!rest.is_empty(), "multi-line comment has ≥2 lines");
        let line = |span: &(u32, u32)| line_slice(content, *span);

        // Frame directly: the `/*<first>` opener, each continuation line preserved
        // verbatim at its authored column via a `literalline` (no context indent),
        // then the `*/` closer.
        //
        // ⚠️ **No trim anywhere** — verbatim means verbatim. prettier's non-indentable arm
        // is `["/*", replaceEndOfLine(comment.value), "*/"]`, which touches nothing, and its
        // renderer (like tsv's) skips its line-end trim behind a LITERAL newline, so an
        // author's trailing space inside such a comment survives on both sides. Trimming
        // here edited the author's bytes on the one path whose whole contract is not to.
        //
        // That is also why the LAST line needs no arm of its own: it is emitted exactly as
        // every other continuation line is. Its indentable twin above still splits three
        // ways, because there the three positions really do take three different trims.
        let mut docs = DocBuf::with_capacity(rest.len() * 2 + 2);
        let mut opener = d.pool_writer();
        opener.push_str("/*");
        opener.push_str(line(first));
        docs.push(opener.finish_text());
        for span in rest {
            docs.push(d.literalline());
            docs.push(d.text_pooled(line(span)));
        }
        docs.push(d.text("*/"));
        d.concat(&docs)
    }

    /// Build a line_suffix doc for a trailing line comment (space + comment)
    ///
    /// Wrapping in line_suffix excludes the comment from width calculations,
    /// so elements can stay compact even when the trailing comment would push
    /// the line over print_width.
    ///
    /// Kind-agnostic like its own-line sibling
    /// ([`Self::build_trailing_comment_doc_own_line`]): a **block** takes it too where
    /// the run it belongs to has already deferred, since an inline emission there would
    /// render *ahead* of the buffered text and reorder the pair (`append_trailing_paren_comments`).
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

    /// The **own-line** variant of [`Self::build_trailing_comment_doc`]: the author
    /// put this comment on a line of its own rather than trailing the previous token,
    /// so it takes a break instead of a space — and the break travels **inside** the
    /// `line_suffix` (prettier's `printTrailingComment`, the
    /// `hasNewline(…, { backwards: true })` branch).
    ///
    /// Inside is the load-bearing part, and it is why this cannot be a `hardline`
    /// pushed between two `build_trailing_comment_doc`s. A trailing gap's comments are
    /// deferred to end of line; a real break emitted between them would land in the
    /// *enclosing construct* — splitting the brackets of `T[K // c1⏎// c2]`, which the
    /// comments are supposed to escape — while a break buffered with them replays at
    /// flush time, after the construct has closed (`T[K]; // c1⏎// c2`). A shell that
    /// *does* want the construct broken gets that from the run's line comments, via
    /// [`Self::push_trailing_comments_in_range`]'s return.
    ///
    /// Applies to a block comment as well as a line one: what is being preserved is the
    /// author's line, not an end-of-line hazard. The `line_suffix` is inert for a block
    /// that trails on the same line — that one keeps the inline form above.
    pub(crate) fn build_trailing_comment_doc_own_line(&self, comment: &internal::Comment) -> DocId {
        self.d()
            .line_suffix(self.deferred_own_line_comment_inner(comment, false))
    }

    /// [`Self::build_trailing_comment_doc_own_line`] with an author BLANK line above the
    /// comment preserved — prettier's `printTrailingComment` emits a second `hardline`
    /// from `isPreviousLineEmpty(locStart(comment))`, asked of the COMMENT rather than of
    /// the gap ([`Self::push_trailing_run_separator`] carries the same rule at the
    /// non-deferred sites). The caller answers that question; this only emits it.
    ///
    /// ⚠️ The blank is a **second `hardline`**, not the `literalline` + `hardline` pair
    /// every non-deferred site uses ([`Self::push_blank_preserving_separator`]). Those
    /// sites emit their blank while the previous line is still open, so the `literalline`
    /// is the break that ends it; here the run's own leading `hardline` has already ended
    /// it, and a `literalline` after that one would leave its indent on the blank line
    /// (it is the one line kind the renderer does not trim behind) and start the comment
    /// at column 0.
    pub(crate) fn build_trailing_comment_doc_own_line_blank(
        &self,
        comment: &internal::Comment,
        blank_above: bool,
    ) -> DocId {
        self.d()
            .line_suffix(self.deferred_own_line_comment_inner(comment, blank_above))
    }

    /// The payload every deferred own-line member shares — the break (and an author
    /// blank's second `hardline`; the rule and its ⚠️ literalline caveat are
    /// [`Self::build_trailing_comment_doc_own_line_blank`]'s) riding INSIDE the
    /// suffix ahead of the comment. One builder so the blank rule cannot drift
    /// between the plain and dedented spellings.
    fn deferred_own_line_comment_inner(
        &self,
        comment: &internal::Comment,
        blank_above: bool,
    ) -> DocId {
        let d = self.d();
        if blank_above {
            d.concat(&[d.hardline(), d.hardline(), self.build_comment_doc(comment)])
        } else {
            d.concat(&[d.hardline(), self.build_comment_doc(comment)])
        }
    }

    /// The clause-tail spelling of
    /// [`Self::build_trailing_comment_doc_own_line_blank`]: the break still rides
    /// inside the `line_suffix`, wrapped in `dedent` levels of `DocArena::dedent`.
    ///
    /// A clause body sits `dedent` `indent` wraps inside the construct whose break
    /// flushes its tail (`StatementContext::clause_body`'s count), and a suffix's
    /// interior break renders at the indent it was QUEUED at — the body's — so the
    /// freed comment would land one construct too deep and settle only on the second
    /// pass, when the reparse reads it from the construct's own gap. The dedent bakes
    /// the settled level into the payload at build time; the renderer's flush-indent
    /// policy is untouched (neither the queued nor the flush indent generalizes —
    /// see `doc/arena_render_suffix.rs`).
    ///
    /// Precondition: the suffix must not be queued inside a sub-tab `align` run —
    /// `RenderIndent::dedented` debug-asserts on pending aligns. Statement positions
    /// never sit inside one today (`align` is type-printing-only), which is what makes
    /// the wrap safe here.
    pub(crate) fn build_clause_tail_comment_doc(
        &self,
        comment: &internal::Comment,
        blank_above: bool,
        dedent: u8,
    ) -> DocId {
        let d = self.d();
        let mut inner = self.deferred_own_line_comment_inner(comment, blank_above);
        for _ in 0..dedent {
            inner = d.dedent(inner);
        }
        d.line_suffix(inner)
    }
}
