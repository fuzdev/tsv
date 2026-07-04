// Single-comment text-layout leaves.
//
// These render one comment's text: line / block / hashbang docs, the multi-line
// block-comment framing (indentable JSDoc vs preserved interior layout), and the
// trailing line/block comment docs (`line_suffix` vs inline).

use super::Printer;
use crate::ast::internal;
use smallvec::SmallVec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing;

/// Stack buffer for a block comment's split lines — inline up to 16 (covers the
/// common JSDoc/TSDoc) before spilling, so most multi-line comments split with
/// no heap allocation.
type CommentLines<'s> = SmallVec<[&'s str; 16]>;

impl<'a> Printer<'a> {
    /// Build a Doc for a single comment
    ///
    /// For multi-line block comments:
    /// - JSDoc comments (/**) always use hardline to apply context indent
    /// - Other comments: if continuation lines had indentation, use hardline; otherwise literalline
    pub(crate) fn build_comment_doc(&self, comment: &internal::Comment) -> DocId {
        let d = self.d();
        let content = comment.content(self.source);
        if comment.is_block {
            // Block comment: /* content */
            if !comment.multiline {
                // Single-line block comment — the full span is verbatim `/*…*/`,
                // so emit it as a source slice (no allocation).
                d.source_span(comment.span, self.source)
            } else {
                // Split once — the classifier and the indentable builder read
                // the same lines. (The rare preserved path re-derives its own
                // lines from the indentation-stripped body instead.)
                let lines: CommentLines<'_> = content.split('\n').collect();
                if printing::is_indentable_block_comment(&lines) {
                    self.build_indentable_block_comment_doc(content, &lines)
                } else {
                    self.build_preserved_block_comment_doc(comment)
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
        }
    }

    /// Build a multi-line *indentable* block comment (JSDoc `/** … */` and
    /// `*`-aligned `/* … */`, where every line begins with `*`).
    ///
    /// Continuation lines are reindented to a single leading space before the
    /// `*` — the context indent is supplied by the per-line hardline (here baked
    /// into a [`DocNode::MultilineText`]), and content after the `*` is
    /// untouched. Mirrors prettier's `printIndentableBlockComment`.
    fn build_indentable_block_comment_doc(&self, content: &str, lines: &[&str]) -> DocId {
        let d = self.d();
        // ≥2 lines: `build_comment_doc` only routes newline-containing content
        // here, pre-split (`lines` is `content.split('\n')`).
        #[allow(clippy::unreachable)] // content has a newline ⇒ split yields ≥2 lines
        let [first, middle @ .., last] = lines else {
            unreachable!("multi-line comment");
        };

        // Frame the whole comment as one `\n`-separated body — the `/*<first>`
        // opener, each continuation line reindented to a single leading space,
        // the `*/` closer — and emit it as a single `MultilineText` node, which
        // renders each `\n` as a context-indented hardline. Byte- and
        // position-identical to the former `concat([text, hardline, text, …])`,
        // but one string allocation instead of one per line.
        //
        // Capacity is an exact upper bound, so the push sequence never reallocs:
        // `content` already holds every line's text and the interior `\n`s;
        // framing adds `/*` + `*/` (4) and at most one leading space per line
        // (`lines.len()`), and the per-line trims only ever remove bytes.
        let mut body = String::with_capacity(content.len() + lines.len() + 4);
        body.push_str("/*");
        body.push_str(first.trim_end());
        for line in middle {
            body.push('\n');
            body.push(' ');
            body.push_str(line.trim());
        }
        // The last line (before `*/`) keeps trailing content via `trim_start`.
        body.push('\n');
        body.push(' ');
        body.push_str(last.trim_start());
        body.push_str("*/");

        d.multiline_text(&body)
    }

    /// Build a multi-line *non-indentable* block comment (at least one line does
    /// not begin with `*`) — preserved with its original interior layout rather
    /// than reindented.
    ///
    /// The comment's own leading indentation is stripped, then re-applied via
    /// `hardline` for `/**`-prefixed comments or comments whose lines were
    /// indented; other comments preserve their lines at column 0 (`literalline`).
    fn build_preserved_block_comment_doc(&self, comment: &internal::Comment) -> DocId {
        let d = self.d();
        let content = comment.content(self.source);
        let stripped =
            printing::strip_comment_indentation(self.source, content, comment.span.start);

        // A `/**`-prefixed comment that reached here is only partially starred
        // (some line lacks `*`); it still gets context indent. Otherwise use
        // context indent only when the comment's lines were indented.
        let use_context_indent = content.starts_with('*') || stripped.len() != content.len();

        // ≥2 lines: `build_comment_doc` only routes newline-containing content here.
        let lines: CommentLines<'_> = stripped.split('\n').collect();
        #[allow(clippy::unreachable)] // stripped retains the newline ⇒ split yields ≥2 lines
        let [first, middle @ .., last] = lines.as_slice() else {
            unreachable!("multi-line comment");
        };

        // Frame directly: the `/*<first>` opener, each continuation line on its
        // own line (context-indented `hardline`, or column-0 `literalline` for
        // blank lines and non-context-indented comments), then the `*/` closer.
        // Unlike the indentable path this mixes line kinds, so it stays a
        // per-line `concat` rather than a `MultilineText`.
        let mut docs = DocBuf::with_capacity((middle.len() + 1) * 2 + 2);
        docs.push(d.text_pooled(&format!("/*{}", first.trim_end())));
        for line in middle {
            // Blank lines stay truly empty (column 0); otherwise apply context
            // indent. Trailing whitespace is trimmed (matches prettier).
            docs.push(if line.is_empty() || !use_context_indent {
                d.literalline()
            } else {
                d.hardline()
            });
            docs.push(d.text_pooled(line.trim_end()));
        }
        // Closing line gets context indent; its content (the space before `*/`)
        // is preserved verbatim.
        docs.push(if use_context_indent {
            d.hardline()
        } else {
            d.literalline()
        });
        docs.push(d.text_pooled(last));
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
