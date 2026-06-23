// Single-comment text-layout leaves.
//
// These render one comment's text: line / block / hashbang docs, the multi-line
// block-comment framing (indentable JSDoc vs preserved interior layout), and the
// trailing line/block comment docs (`line_suffix` vs inline).

use super::Printer;
use crate::ast::internal;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing;

impl<'a> Printer<'a> {
    /// Build a Doc for a single comment
    ///
    /// For multi-line block comments:
    /// - JSDoc comments (/**) always use hardline to apply context indent
    /// - Other comments: if continuation lines had indentation, use hardline; otherwise literalline
    pub(crate) fn build_comment_doc(&self, comment: &internal::Comment) -> DocId {
        let d = self.d();
        if comment.is_block {
            // Block comment: /* content */
            if !comment.content.contains('\n') {
                // Single-line block comment
                d.text_owned(format!("/*{}*/", comment.content))
            } else if printing::is_indentable_block_comment(&comment.content) {
                self.build_indentable_block_comment_doc(&comment.content)
            } else {
                self.build_preserved_block_comment_doc(comment)
            }
        } else if comment.span.start == 0 && comment.content.starts_with("#!") {
            // Hashbang comment: #!/usr/bin/env node (no // prefix)
            // Content already includes the #! prefix. Like a line comment it runs
            // to end-of-line, so tag it for the swallow check.
            d.line_comment_text_owned(comment.content.clone())
        } else {
            // Line comment: // content. Tagged for the swallow check (runs to EOL).
            d.line_comment_text_owned(format!("//{}", comment.content))
        }
    }

    /// Frame a multi-line block comment's continuation docs (`inner`) with the
    /// `/*<first_line>` opener and the `*/` closer. `first_line` is the content
    /// of the line right after `/*` (trailing whitespace trimmed).
    fn frame_block_comment_doc(&self, first_line: &str, inner: Vec<DocId>) -> DocId {
        let d = self.d();
        let mut docs = Vec::with_capacity(inner.len() + 2);
        docs.push(d.text_owned(format!("/*{}", first_line.trim_end())));
        docs.extend(inner);
        docs.push(d.text("*/"));
        d.concat(&docs)
    }

    /// Build a multi-line *indentable* block comment (JSDoc `/** … */` and
    /// `*`-aligned `/* … */`, where every line begins with `*`).
    ///
    /// Continuation lines are reindented to a single leading space before the
    /// `*` — the context indent is supplied by the `hardline`, and content after
    /// the `*` is untouched. Mirrors prettier's `printIndentableBlockComment`.
    fn build_indentable_block_comment_doc(&self, content: &str) -> DocId {
        let d = self.d();
        // ≥2 lines: `build_comment_doc` only routes newline-containing content here.
        let lines: Vec<&str> = content.split('\n').collect();
        let [first, middle @ .., last] = lines.as_slice() else {
            unreachable!("multi-line comment");
        };

        let mut inner = Vec::with_capacity((middle.len() + 1) * 2);
        for line in middle {
            inner.push(d.hardline());
            inner.push(d.text_owned(format!(" {}", line.trim())));
        }
        // The last line (before `*/`) keeps trailing content via `trim_start`.
        inner.push(d.hardline());
        inner.push(d.text_owned(format!(" {}", last.trim_start())));

        self.frame_block_comment_doc(first, inner)
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
        let stripped =
            printing::strip_comment_indentation(self.source, &comment.content, comment.span.start);

        // A `/**`-prefixed comment that reached here is only partially starred
        // (some line lacks `*`); it still gets context indent. Otherwise use
        // context indent only when the comment's lines were indented.
        let use_context_indent =
            comment.content.starts_with('*') || stripped.len() != comment.content.len();

        // ≥2 lines: `build_comment_doc` only routes newline-containing content here.
        let lines: Vec<&str> = stripped.split('\n').collect();
        let [first, middle @ .., last] = lines.as_slice() else {
            unreachable!("multi-line comment");
        };

        let mut inner = Vec::with_capacity((middle.len() + 1) * 2);
        for line in middle {
            // Blank lines stay truly empty (column 0); otherwise apply context
            // indent. Trailing whitespace is trimmed (matches prettier).
            inner.push(if line.is_empty() || !use_context_indent {
                d.literalline()
            } else {
                d.hardline()
            });
            inner.push(d.text_owned(line.trim_end().to_string()));
        }
        // Closing line gets context indent; its content (the space before `*/`)
        // is preserved verbatim.
        inner.push(if use_context_indent {
            d.hardline()
        } else {
            d.literalline()
        });
        inner.push(d.text_owned((*last).to_string()));

        self.frame_block_comment_doc(first, inner)
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
