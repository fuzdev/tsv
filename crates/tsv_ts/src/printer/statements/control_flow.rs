// Control flow statement printing for TypeScript

use smallvec::SmallVec;

use super::Printer;
use crate::ast::internal::{self, Expression, Statement};
use crate::printer::analysis::find_char_skipping_comments;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::DocId;

/// Small vector of comment references, stack-allocated for typical cases.
type CommentVec<'a> = SmallVec<[&'a tsv_lang::Comment; 2]>;

/// Span positions for a for loop header
///
/// Groups the optional span positions for init, test, and update clauses
/// to avoid passing many Option parameters.
struct ForHeaderSpans {
    open_paren: Option<u32>,
    init_start: Option<u32>,
    init_end: Option<u32>,
    test_start: Option<u32>,
    test_end: Option<u32>,
    update_start: Option<u32>,
    close_paren: Option<u32>,
}

/// Check if a statement can be printed inline after `if (cond)` without a newline.
///
/// Block, expression, break, continue, return, throw, and empty statements stay inline.
/// Other statements (if, for, while, etc.) go on a new line with indent.
fn is_inline_consequent(stmt: &Statement) -> bool {
    matches!(
        stmt,
        Statement::BlockStatement(_)
            | Statement::ExpressionStatement(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::ReturnStatement(_)
            | Statement::ThrowStatement(_)
            | Statement::EmptyStatement(_)
    )
}

/// Check if a statement can be printed inline after `else` without a newline.
///
/// Same as `is_inline_consequent` but also allows IfStatement for else-if chains.
fn is_inline_alternate(stmt: &Statement) -> bool {
    is_inline_consequent(stmt) || matches!(stmt, Statement::IfStatement(_))
}

impl<'a> Printer<'a> {
    /// Partition comments between two positions into inline vs own-line.
    ///
    /// Returns `(inline_with_prev, own_line, inline_with_next)` where:
    /// - `inline_with_prev`: Comments on the same line as `prev_end`
    /// - `own_line`: Comments on their own line (not same line as prev or next)
    /// - `inline_with_next`: Comments on the same line as `next_start`
    ///
    /// This helper reduces repetitive comment classification code throughout
    /// control flow statement printing.
    fn partition_comments_by_line(
        &self,
        prev_end: u32,
        next_start: u32,
    ) -> (CommentVec<'a>, CommentVec<'a>, CommentVec<'a>) {
        let mut inline_prev = SmallVec::new();
        let mut own_line = SmallVec::new();
        let mut inline_next = SmallVec::new();

        for comment in tsv_lang::comments_in_range(self.comments, prev_end, next_start) {
            let same_line_as_prev = self.is_same_line(prev_end, comment.span.start);
            let same_line_as_next = self.is_same_line(comment.span.end, next_start);

            if same_line_as_prev {
                inline_prev.push(comment);
            } else if same_line_as_next {
                inline_next.push(comment);
            } else {
                own_line.push(comment);
            }
        }

        (inline_prev, own_line, inline_next)
    }

    /// Build comments between a keyword and its `(`, preserving position.
    ///
    /// Returns a doc for comments between `keyword_end` and `open_paren` if any exist.
    /// Example: `if/* c */(a)` → `if /* c */ (a)` (comment stays between keyword and paren)
    fn build_keyword_paren_comments(
        &self,
        keyword_end: u32,
        open_paren: Option<u32>,
    ) -> Option<DocId> {
        open_paren.and_then(|op| self.build_inline_comments_between_doc_opt(keyword_end, op))
    }

    /// Build docs for comments between statement parts (e.g., between `}` and `else`).
    ///
    /// Handles:
    /// - Inline comments: added with leading space on same line
    /// - Own-line comments: added with hardline, preserving blank lines before them
    ///
    /// Returns the end position after the last comment (for tracking).
    fn build_comments_between_parts(
        &self,
        parts: &mut Vec<DocId>,
        inline_prev: &[&tsv_lang::Comment],
        own_line: &[&tsv_lang::Comment],
        prev_end: u32,
    ) -> u32 {
        let d = self.d();
        // Trailing comments stay on same line
        for comment in inline_prev {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        // Own-line comments: preserve blank lines before them
        let mut end = prev_end;
        for comment in own_line {
            if self.has_blank_line_between(end, comment.span.start) {
                // Blank line then comment: literalline (empty) + hardline (indented)
                parts.push(d.literalline());
                parts.push(d.hardline());
            } else {
                parts.push(d.hardline());
            }
            parts.push(self.build_comment_doc(comment));
            end = comment.span.end;
        }
        end
    }

    /// Append `) ` to parts, extracting any comments between the close paren and body.
    ///
    /// Used for block bodies: if, while, for-in/for-of `{ }`. For non-block bodies in
    /// for-in/for-of, use `append_close_paren_with_non_block_body` which also indents.
    /// Append `)` + comments + `;` for empty statement bodies.
    ///
    /// Handles comments between `)` and `;`:
    /// - Block comments: `if (a) /* comment */ ;`
    /// - Line comments: `if (a) // comment\n;`
    /// - No comments: `if (a);`
    fn append_close_paren_empty_stmt_with_comments(
        &self,
        parts: &mut Vec<DocId>,
        paren_end: u32,
        empty_start: u32,
    ) {
        let d = self.d();
        parts.push(d.text(")"));
        if self.has_comments_between(paren_end, empty_start) {
            let has_line = self.has_line_comments_between(paren_end, empty_start);
            let comment_doc =
                self.build_inline_comments_between_doc_no_leading_space(paren_end, empty_start);
            if has_line {
                parts.push(d.text(" "));
                parts.push(comment_doc);
                parts.push(d.hardline());
                parts.push(d.text(";"));
            } else {
                parts.push(d.text(" "));
                parts.push(comment_doc);
                parts.push(d.text(" ;"));
            }
        } else {
            parts.push(d.text(";"));
        }
    }

    /// Block comments are always inlined (trailing after `)`). Line comments preserve
    /// their position: trailing stays trailing, own-line stays on its own line (with
    /// blank line preservation). Line comments force a hardline before the body.
    fn append_close_paren_with_comments(
        &self,
        parts: &mut Vec<DocId>,
        paren_end: u32,
        body_start: u32,
    ) {
        let d = self.d();
        if self.has_comments_between(paren_end, body_start) {
            let (mut inline_prev, own_line, inline_next) =
                self.partition_comments_by_line(paren_end, body_start);

            // Own-line block comments become inline — block comments are flexible
            // and should normalize to trailing position (matches prettier).
            // Only line comments preserve own-line position.
            // inline_next (comments on same line as body `{`) are treated same as own_line.
            let mut own_line_lines: CommentVec = SmallVec::new();
            for comment in own_line.into_iter().chain(inline_next) {
                if comment.is_block {
                    inline_prev.push(comment);
                } else {
                    own_line_lines.push(comment);
                }
            }

            parts.push(d.text(")"));
            // Use the end of the last inline comment for blank-line detection in the
            // own-line loop — reclassified block comments shift the reference point.
            let effective_prev_end = inline_prev.last().map_or(paren_end, |c| c.span.end);
            self.build_comments_between_parts(
                parts,
                &inline_prev,
                &own_line_lines,
                effective_prev_end,
            );

            // Line comments force a hardline before body; block-only gets a space.
            if !own_line_lines.is_empty() || inline_prev.iter().any(|c| !c.is_block) {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
        } else {
            parts.push(d.text(") "));
        }
    }

    /// Append `)` + comments + non-block body for for-in/for-of statements.
    ///
    /// Unlike `append_close_paren_with_comments` (which handles block bodies where
    /// indentation isn't needed), this properly indents non-block bodies when line
    /// comments force a break. Also avoids placing block comments after line comments
    /// on the same line (which would absorb them into the line comment text).
    fn append_close_paren_with_non_block_body(
        &self,
        parts: &mut Vec<DocId>,
        paren_end: u32,
        body: &Statement,
    ) {
        let d = self.d();
        let body_start = body.span().start;
        let body_doc = self.build_statement_doc(body);

        if !self.has_comments_between(paren_end, body_start) {
            parts.push(d.text(")"));
            if matches!(body, Statement::EmptyStatement(_)) {
                // Prettier's `adjustClause` returns `";"` directly for an empty
                // body (no leading `line`) → `for (x of y);`, not `for (x of y) ;`.
                parts.push(body_doc);
            } else {
                // Mirror Prettier's `adjustClause`: `indent([line, body])`. The
                // enclosing for-in/for-of group (see `build_for_in/of_statement_with_body_doc`)
                // breaks on overflow, dropping the body to its own indented line;
                // when it fits, `line` is a space → `for (x of y) stmt;`.
                parts.push(d.indent_line(body_doc));
            }
            return;
        }

        let (inline_prev, own_line, inline_next) =
            self.partition_comments_by_line(paren_end, body_start);

        // Check if any line comment forces a break
        let has_line =
            inline_prev.iter().any(|c| !c.is_block) || own_line.iter().any(|c| !c.is_block);

        parts.push(d.text(")"));

        if has_line {
            // Emit trailing comments on the `)` line
            for comment in &inline_prev {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            // Remaining comments (own_line + inline_next) go indented before body
            let mut inner = vec![d.hardline()];
            for comment in own_line.into_iter().chain(inline_next) {
                inner.push(self.build_comment_doc(comment));
                if comment.is_block {
                    inner.push(d.text(" "));
                } else {
                    inner.push(d.hardline());
                }
            }
            inner.push(body_doc);
            parts.push(d.indent(d.concat(&inner)));
        } else {
            // Block comments only: adjustClause — `) /* a */ body` stays flat but the
            // comment(s) + body drop to their own indented line when the enclosing
            // for-in/for-of group breaks (overflow). Matches Prettier.
            let mut inner = Vec::new();
            for comment in inline_prev
                .iter()
                .chain(own_line.iter())
                .chain(inline_next.iter())
            {
                inner.push(self.build_comment_doc(comment));
                inner.push(d.text(" "));
            }
            inner.push(body_doc);
            parts.push(d.indent_line(d.concat(&inner)));
        }
    }

    /// Append an else body to parts, dispatching on statement type.
    ///
    /// When `comment_forced` is true, the layout was already determined by a preceding comment,
    /// so non-block bodies are emitted directly. When false, non-block/non-inline bodies get
    /// indented (Prettier's adjustClause behavior).
    fn append_else_body_doc(
        &self,
        parts: &mut Vec<DocId>,
        alternate: &Statement,
        comment_forced: bool,
    ) {
        if let Statement::BlockStatement(block) = alternate {
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else if comment_forced || is_inline_alternate(alternate) {
            parts.push(self.build_statement_doc(alternate));
        } else {
            let d = self.d();
            parts.push(d.indent(d.concat(&[d.hardline(), self.build_statement_doc(alternate)])));
        }
    }

    /// Append `else` clause on a new line for non-block/empty-statement consequent paths.
    ///
    /// Handles EmptyStatement alternate (`else;`), inline alternate (`else expr;`),
    /// block alternate (`else { ... }`), and non-inline alternate (indented).
    fn append_newline_else_clause(&self, parts: &mut Vec<DocId>, alternate: &Statement) {
        let d = self.d();
        parts.push(d.hardline());
        if matches!(alternate, Statement::EmptyStatement(_)) {
            parts.push(d.text("else;"));
        } else {
            parts.push(d.text("else "));
            if is_inline_alternate(alternate) {
                if let Statement::BlockStatement(block) = alternate {
                    parts.push(self.build_block_statement_expand_empty_doc(block));
                } else {
                    parts.push(self.build_statement_doc(alternate));
                }
            } else {
                parts.push(d.hardline());
                parts.push(d.indent(self.build_statement_doc(alternate)));
            }
        }
    }

    /// Build an adjust-clause doc with head-body comment handling for non-block bodies.
    ///
    /// Used by if/while for `stmt (cond) /* c */ fn();` and `stmt (cond) // c\n fn();`.
    /// Returns the full `keyword (condition) body` doc including comments when present.
    ///
    /// `head_parts` are the docs before the `)` (e.g., `["if (", condition_group]`).
    fn build_adjust_clause_with_comments(
        &self,
        head_parts: &[DocId],
        paren_end: u32,
        body_start: u32,
        body_doc: DocId,
    ) -> DocId {
        let d = self.d();
        if self.has_comments_between(paren_end, body_start) {
            let has_line = self.has_line_comments_between(paren_end, body_start);
            let comment_doc =
                self.build_inline_comments_between_doc_no_leading_space(paren_end, body_start);
            let mut parts = head_parts.to_vec();
            parts.push(d.text(")"));
            if has_line {
                // Line comment forces break: stmt (cond)\n\t// comment\n\tfn();
                parts.push(d.indent(d.concat(&[
                    d.hardline(),
                    comment_doc,
                    d.hardline(),
                    body_doc,
                ])));
                d.concat(&parts)
            } else {
                // Block comment stays with statement: stmt (cond) /* c */ fn();
                // When broken: stmt (cond)\n\t/* c */ fn();
                parts.push(d.indent(d.concat(&[d.line(), comment_doc, d.text(" "), body_doc])));
                d.group(d.concat(&parts))
            }
        } else {
            let mut parts = head_parts.to_vec();
            parts.push(d.text(")"));
            parts.push(d.indent_line(body_doc));
            d.group(d.concat(&parts))
        }
    }

    /// Append a space (or comments + space/hardline) between a keyword/token end and body start.
    ///
    /// Used for `try /* c */ {`, `catch (e) /* c */ {`, `catch /* c */ {`, `finally /* c */ {`.
    fn append_keyword_to_body_comments(
        &self,
        parts: &mut Vec<DocId>,
        token_end: u32,
        body_start: u32,
    ) {
        let d = self.d();
        if self.has_comments_between(token_end, body_start) {
            let has_line = self.has_line_comments_between(token_end, body_start);
            parts.push(self.build_inline_comments_between_doc(token_end, body_start));
            if has_line {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
        } else {
            parts.push(d.text(" "));
        }
    }

    /// Build else clause with comment extraction between `else` keyword and body.
    ///
    /// Handles block comments staying inline: `} else /* c */ {`
    /// and line comments forcing a break before the body.
    fn build_head_body_else_clause(
        &self,
        parts: &mut Vec<DocId>,
        alternate: &Statement,
        consequent_end: u32,
    ) {
        let d = self.d();
        let alt_start = alternate.span().start;

        // Find "else" keyword by scanning forward from consequent end, skipping comments
        let else_end = self.find_else_keyword_end_between(consequent_end, alt_start);

        if matches!(alternate, Statement::EmptyStatement(_)) {
            // Empty alternate: `} else;`, `} else /* c */ ;`, or `} else // c\n;`
            if let Some(else_end) = else_end
                && self.has_comments_between(else_end, alt_start)
            {
                let has_line = self.has_line_comments_between(else_end, alt_start);
                parts.push(d.text(" else"));
                parts.push(self.build_inline_comments_between_doc(else_end, alt_start));
                if has_line {
                    // Line comment: `} else // c\n;`
                    parts.push(d.hardline());
                    parts.push(d.text(";"));
                } else {
                    // Block comment: `} else /* c */ ;`
                    parts.push(d.text(" ;"));
                }
            } else {
                parts.push(d.text(" else;"));
            }
        } else if let Some(else_end) = else_end
            && self.has_comments_between(else_end, alt_start)
        {
            // Comments between `else` and body
            let has_line = self.has_line_comments_between(else_end, alt_start);
            let is_non_block_non_if = !matches!(
                alternate,
                Statement::BlockStatement(_) | Statement::IfStatement(_)
            );
            parts.push(d.text(" else"));
            parts.push(self.build_inline_comments_between_doc(else_end, alt_start));
            if has_line && is_non_block_non_if {
                // Line comment + non-block body: comment stays on else line, body indented
                // } else // c\n\texpr;
                let body_doc = self.build_statement_doc(alternate);
                parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
            } else if has_line {
                parts.push(d.hardline());
                self.append_else_body_doc(parts, alternate, true);
            } else {
                parts.push(d.text(" "));
                self.append_else_body_doc(parts, alternate, true);
            }
        } else {
            parts.push(d.text(" else "));
            self.append_else_body_doc(parts, alternate, false);
        }
    }

    /// Find the end position of the "else" keyword between two positions.
    ///
    /// Scans forward from `from` to `to`, skipping comment content so that
    /// "else" inside comments (e.g., `} else /* or else */ {`) is not matched.
    fn find_else_keyword_end_between(&self, from: u32, to: u32) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let mut i = from as usize;
        let end = to as usize;
        while i + 4 <= end {
            match bytes[i] {
                b'/' if i + 1 < end => match bytes[i + 1] {
                    b'*' => {
                        i += 2;
                        while i + 1 < end && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                            i += 1;
                        }
                        i += 2;
                        // Clamp in case block comment was unterminated
                        i = i.min(end);
                    }
                    b'/' => {
                        while i < end && bytes[i] != b'\n' {
                            i += 1;
                        }
                        i = (i + 1).min(end);
                    }
                    _ => i += 1,
                },
                b'e' if i + 4 <= end && &self.source[i..i + 4] == "else" => {
                    return Some((i + 4) as u32);
                }
                _ => i += 1,
            }
        }
        None
    }

    /// Build a condition group for if/while/for/switch statements
    ///
    /// Creates the standard Prettier condition structure:
    /// ```text
    /// group([indent([softline, condition]), softline])
    /// ```
    ///
    /// This group decides whether the condition breaks (operators go to new lines).
    /// Binary expressions use ungrouped version so this parent group controls their breaking.
    fn build_condition_group(&self, test_expr: &Expression) -> DocId {
        let d = self.d();
        let test_doc = self.build_condition_doc(test_expr);
        d.group(d.concat(&[d.indent_softline(test_doc), d.softline()]))
    }

    /// Build a condition group with comment support for if/while/do-while/switch statements
    ///
    /// Handles comments inside condition/discriminant parens:
    /// ```js
    /// if (
    ///     // before condition
    ///     x // inline with condition
    ///     // trailing after condition
    /// ) {
    /// ```
    fn build_condition_group_with_comments(
        &self,
        test_expr: &Expression,
        open_paren_pos: u32,
        close_paren_pos: u32,
    ) -> DocId {
        self.build_condition_group_with_comments_impl(
            test_expr,
            open_paren_pos,
            close_paren_pos,
            false, // normalize inline comments to own line
        )
    }

    /// Build condition group preserving inline comments after open paren
    ///
    /// Used for do-while where we intentionally differ from Prettier's behavior
    /// of moving comments outside the parens.
    fn build_condition_group_preserve_inline(
        &self,
        test_expr: &Expression,
        open_paren_pos: u32,
        close_paren_pos: u32,
    ) -> DocId {
        self.build_condition_group_with_comments_impl(
            test_expr,
            open_paren_pos,
            close_paren_pos,
            true, // preserve inline comments
        )
    }

    fn build_condition_group_with_comments_impl(
        &self,
        test_expr: &Expression,
        open_paren_pos: u32,
        close_paren_pos: u32,
        preserve_inline: bool,
    ) -> DocId {
        let d = self.d();
        let test_start = test_expr.span().start;
        let test_end = test_expr.span().end;

        // Check for comments before and after the condition
        let has_leading = self.has_comments_between(open_paren_pos + 1, test_start);
        let has_trailing = self.has_comments_between(test_end, close_paren_pos);

        if !has_leading && !has_trailing {
            // No comments - use the standard condition group
            return self.build_condition_group(test_expr);
        }

        // Build with comments
        let test_doc = self.build_condition_doc(test_expr);
        let mut inner_parts = Vec::new();

        // Collect leading comments
        // Classification based on position relative to open paren AND condition:
        // - "inline with open paren" = comment STARTS on same line as open paren
        // - "own line" = comment does NOT start on same line as open paren
        let leading_comments: Vec<_> = if has_leading {
            tsv_lang::comments_in_range(self.comments, open_paren_pos + 1, test_start).collect()
        } else {
            Vec::new()
        };

        // Check if there are own-line leading comments (not on same line as open paren)
        let has_own_line_leading = leading_comments
            .iter()
            .any(|c| !self.is_same_line(open_paren_pos, c.span.start));

        if preserve_inline {
            // Preserve inline comments after open paren (used for do-while divergence)
            let mut has_inline_comment_followed_by_newline = false;

            // Leading inline comments (on same line as open paren)
            for comment in &leading_comments {
                if self.is_same_line(open_paren_pos, comment.span.start) {
                    // Only add space if source has whitespace between ( and comment
                    let space_between =
                        &self.source[(open_paren_pos + 1) as usize..comment.span.start as usize];
                    if !space_between.is_empty() {
                        inner_parts.push(d.text(" "));
                    }
                    inner_parts.push(self.build_comment_doc(comment));
                    if !self.is_same_line(comment.span.end, test_start) {
                        has_inline_comment_followed_by_newline = true;
                    } else {
                        inner_parts.push(d.text(" "));
                    }
                }
            }

            if has_inline_comment_followed_by_newline {
                inner_parts.push(d.hardline());
            }

            // Own-line comments
            for comment in &leading_comments {
                if !self.is_same_line(open_paren_pos, comment.span.start) {
                    if !has_inline_comment_followed_by_newline {
                        inner_parts.push(d.hardline());
                    }
                    inner_parts.push(self.build_comment_doc(comment));
                    if !self.is_same_line(comment.span.end, test_start) {
                        inner_parts.push(d.hardline());
                    } else {
                        inner_parts.push(d.text(" "));
                    }
                }
            }

            if !has_inline_comment_followed_by_newline && !has_own_line_leading {
                inner_parts.push(d.softline());
            }
        } else {
            // Normalize comments based on their position:
            // - Comments on own line (not same line as open paren): force break with hardline
            // - Comments inline with open paren: allow collapsing with softline
            let mut added_comment = false;
            let mut last_comment_same_line_as_test = false;
            for comment in &leading_comments {
                let on_same_line_as_open = self.is_same_line(open_paren_pos, comment.span.start);

                if on_same_line_as_open {
                    // Comment is inline with open paren - use softline to allow collapse
                    inner_parts.push(d.softline());
                } else {
                    // Comment is on its own line - force break
                    inner_parts.push(d.hardline());
                }
                inner_parts.push(self.build_comment_doc(comment));
                added_comment = true;

                // Check if condition is on same line as comment end
                last_comment_same_line_as_test = self.is_same_line(comment.span.end, test_start);
                // Space if on same line, hardline if on different line
                if last_comment_same_line_as_test {
                    inner_parts.push(d.text(" "));
                } else if !comment.is_block {
                    // Line comment - need hardline before condition (next comment iteration will add it, or we add it below)
                }
            }

            // Add softline before condition if no comments were added
            // If we added comments and the last one wasn't on same line as test, we need hardline
            if !added_comment {
                inner_parts.push(d.softline());
            } else if !last_comment_same_line_as_test {
                inner_parts.push(d.hardline());
            }
        }

        // The condition itself
        inner_parts.push(test_doc);

        // Trailing comments use partition_comments_by_line since the classification matches:
        // inline = starts on same line as test_end (goes to inline_prev)
        // own line = doesn't start on same line as test_end
        let (trailing_inline, trailing_own_line, _) =
            self.partition_comments_by_line(test_end, close_paren_pos);

        // Trailing inline comments (same line as condition)
        for comment in &trailing_inline {
            inner_parts.push(d.text(" "));
            inner_parts.push(self.build_comment_doc(comment));
        }

        // Trailing comments on their own line (after condition)
        for comment in &trailing_own_line {
            inner_parts.push(d.hardline());
            inner_parts.push(self.build_comment_doc(comment));
        }

        // Structure: group([indent([softline/hardline, comments, condition, comments]), softline/hardline])
        // The closing softline/hardline is OUTSIDE the indent so `)` aligns with `(`
        // Force break when trailing inline line comments exist — flattening would cause
        // the // comment to swallow the closing `) {` producing unparseable output
        let has_trailing_line_comment = trailing_inline.iter().any(|c| !c.is_block);
        let closing =
            if has_own_line_leading || !trailing_own_line.is_empty() || has_trailing_line_comment {
                d.hardline()
            } else {
                d.softline()
            };

        d.group(d.concat(&[d.indent(d.concat(&inner_parts)), closing]))
    }

    /// Find the position of the opening paren for a keyword statement
    /// Returns the position of '(' after the keyword.
    ///
    /// Skips `(` characters inside comments and strings (`if /* (note) */ (cond)`),
    /// so a parenthesis in a leading comment can't be mistaken for the condition's
    /// open paren.
    fn find_open_paren_after(&self, start: u32) -> Option<u32> {
        find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32)
    }

    /// Build a doc for an if statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture from estree.js:
    /// ```js
    /// group([
    ///   "if (",
    ///   group([indent([softline, test]), softline]),  // inner group for condition
    ///   ")",
    ///   adjustClause(consequent),  // body handling
    /// ])
    /// ```
    fn build_if_statement_with_wrapping_doc(&self, stmt: &internal::IfStatement) -> DocId {
        let d = self.d();
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `if` keyword and `(` in place:
        //   if/* c */(a){} → if /* c */ (a) {}
        let if_keyword_end = stmt.span.start + 2; // "if" is 2 chars
        let keyword_comments = self.build_keyword_paren_comments(if_keyword_end, open_paren);

        // Build condition group (handles breaking within condition and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.test, open, close)
        } else {
            self.build_condition_group(&stmt.test)
        };

        if let Statement::BlockStatement(block) = stmt.consequent.as_ref() {
            // Block consequent: group(["if (" + condition + ") " + block])
            // Outer group controls whether the whole if statement breaks
            let mut parts = vec![d.text("if")];
            if let Some(kc) = keyword_comments {
                parts.push(kc);
            }
            parts.push(d.text(" ("));
            parts.push(condition_group);

            // Check for comments between ) and block body
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);

            parts.push(self.build_block_statement_expand_empty_doc(block));

            // Handle else clause
            if let Some(alternate) = &stmt.alternate {
                self.build_head_body_else_clause(&mut parts, alternate, block.span.end);
            }

            // Outer group for the whole if statement
            d.group(d.concat(&parts))
        } else if matches!(stmt.consequent.as_ref(), Statement::EmptyStatement(_)) {
            // Empty statement: `if (cond);` or `if (cond) /* comment */ ;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let empty_start = stmt.consequent.span().start;

            let mut empty_parts = vec![d.text("if")];
            if let Some(kc) = keyword_comments {
                empty_parts.push(kc);
            }
            empty_parts.push(d.text(" ("));
            empty_parts.push(condition_group);
            self.append_close_paren_empty_stmt_with_comments(
                &mut empty_parts,
                paren_end,
                empty_start,
            );

            // Handle else clause for empty-statement consequent
            if let Some(alternate) = &stmt.alternate {
                self.append_newline_else_clause(&mut empty_parts, alternate);
            }

            d.group(d.concat(&empty_parts))
        } else {
            // Non-block consequent: use adjustClause equivalent
            // Prettier's adjustClause returns: indent([line, clause])
            // - When flat: line becomes space -> `if (cond) a;`
            // - When broken: line becomes newline + indent -> `if (cond)\n\ta;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let body_start = stmt.consequent.span().start;
            let consequent_doc = self.build_statement_doc(&stmt.consequent);

            let mut head_parts = vec![d.text("if")];
            if let Some(kc) = keyword_comments {
                head_parts.push(kc);
            }
            head_parts.push(d.text(" ("));
            head_parts.push(condition_group);
            let head_and_body = self.build_adjust_clause_with_comments(
                &head_parts,
                paren_end,
                body_start,
                consequent_doc,
            );

            let mut parts = vec![head_and_body];

            // Handle else clause for non-block consequent
            if let Some(alternate) = &stmt.alternate {
                self.append_newline_else_clause(&mut parts, alternate);
            }

            d.concat(&parts)
        }
    }

    /// Build a doc for a condition expression (if/while/for test)
    ///
    /// For binary expressions, uses ungrouped version so parent group controls breaking.
    /// Logical operators (`&&`, `||`, `??`) break with the parent condition group.
    /// Non-logical operators (`<`, `===`, etc.) keep a sub-group for independent evaluation
    /// (e.g., `for (i = 0; i < len; i++)` — the `i < len` stays flat).
    /// Assignment expressions get double-parens for clarity: `while ((x = y))`
    fn build_condition_doc(&self, expr: &Expression) -> DocId {
        let inner = match expr {
            Expression::BinaryExpression(binary) => {
                self.build_binary_chain_doc_ungrouped_condition(binary)
            }
            _ => self.build_expression_doc(expr),
        };
        if super::needs_parens(expr, super::ParenContext::StatementTest) {
            let d = self.d();
            d.concat(&[d.text("("), inner, d.text(")")])
        } else {
            inner
        }
    }

    /// Build a complete for statement doc including the body
    ///
    /// This includes the body in the doc so the width calculation accounts for ` {`.
    fn build_for_statement_with_body_doc(&self, stmt: &internal::ForStatement) -> DocId {
        let d = self.d();
        let header_doc = self.build_for_header_doc(stmt);
        if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            // No space before empty statement: `for (...);`
            d.concat(&[header_doc, self.build_statement_doc(&stmt.body)])
        } else if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            // Block body: `for (...) { ... }`
            // Note: Unlike for-in/for-of, standard for loops keep empty blocks inline `{}`
            d.concat(&[
                header_doc,
                d.text(" "),
                self.build_block_statement_doc(block),
            ])
        } else {
            // Non-block body. Mirror Prettier's `adjustClause`: the body is
            // `indent([line, body])` wrapped with the header in an outer group.
            // Flat → `for (...) stmt;`. When the header force-breaks (a comment
            // hardline propagates via `will_break`) or the whole thing overflows,
            // the outer group breaks and the body drops to its own indented line;
            // the inner header group still decides its own flat/break, so a
            // width-only overflow keeps the header flat (matching Prettier).
            let body_doc = self.build_statement_doc(&stmt.body);
            d.group(d.concat(&[header_doc, d.indent_line(body_doc)]))
        }
    }

    /// Get the end position of a for loop header (position after the closing paren)
    fn get_for_header_end(&self, stmt: &internal::ForStatement) -> u32 {
        // Find the last expression end
        let last_expr_end = stmt
            .update
            .as_ref()
            .map(|u| u.span().end)
            .or_else(|| stmt.test.as_ref().map(|t| t.span().end))
            .or_else(|| stmt.init.as_ref().map(|i| self.get_for_init_span_end(i)));

        // Find the for header's closing paren via its open paren (depth-tracked, so
        // redundant parens or parens inside a clause don't yield a premature match).
        let search_start = last_expr_end.unwrap_or(stmt.span.start + 4);
        self.find_open_paren_after(stmt.span.start)
            .and_then(|open| self.matching_close_paren(open))
            .map_or(search_start, |p| p + 1)
    }

    /// Build a Doc for the for loop header with wrapping support
    ///
    /// Handles comments in each clause position:
    /// ```js
    /// for (
    ///     // before init
    ///     let i = 0; // inline with init
    ///     // before test
    ///     i < 10; // inline with test
    ///     // before update
    ///     i++ // inline with update
    /// ) {
    /// ```
    fn build_for_header_doc(&self, stmt: &internal::ForStatement) -> DocId {
        self.build_for_header_doc_impl(stmt, false, None)
    }

    /// Build doc for empty for (;;) with comments inside
    ///
    /// Preserves comments in their original positions (divergence from prettier).
    /// Format: for (\n\t; // comment\n\t; // comment\n\t// comment\n)
    fn build_for_empty_with_comments(&self, stmt: &internal::ForStatement) -> DocId {
        let d = self.d();
        let Some(open_paren) = self.find_open_paren_after(stmt.span.start) else {
            return d.text("for (;;)");
        };
        let Some(close_paren) = self.matching_close_paren(open_paren) else {
            return d.text("for (;;)");
        };

        // Find the two semicolons
        let first_semi = self.source[open_paren as usize..]
            .find(';')
            .map(|p| open_paren + p as u32);
        let second_semi = first_semi.and_then(|p| {
            self.source[(p + 1) as usize..]
                .find(';')
                .map(|off| p + 1 + off as u32)
        });

        let mut parts = vec![d.text("for (")];
        let mut inner_parts = Vec::new();

        // First semicolon line: ; // inline comment
        inner_parts.push(d.hardline());
        inner_parts.push(d.text(";"));
        if let (Some(semi1), Some(semi2)) = (first_semi, second_semi) {
            for comment in tsv_lang::comments_in_range(self.comments, semi1 + 1, semi2) {
                if self.is_same_line(semi1, comment.span.start) {
                    inner_parts.push(d.text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // Second semicolon line: ; // inline comment
        inner_parts.push(d.hardline());
        inner_parts.push(d.text(";"));

        // Comments after second semicolon: inline first, then own-line
        if let Some(semi2) = second_semi {
            let mut own_line_comments = Vec::new();
            for comment in tsv_lang::comments_in_range(self.comments, semi2 + 1, close_paren) {
                if self.is_same_line(semi2, comment.span.start) {
                    inner_parts.push(d.text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                } else {
                    own_line_comments.push(comment);
                }
            }
            for comment in own_line_comments {
                inner_parts.push(d.hardline());
                inner_parts.push(self.build_comment_doc(comment));
            }
        }

        parts.push(d.indent(d.concat(&inner_parts)));
        parts.push(d.hardline());
        parts.push(d.text(")"));

        d.concat(&parts)
    }

    fn build_for_header_doc_impl(
        &self,
        stmt: &internal::ForStatement,
        force_break: bool,
        keyword_comments: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let has_init = stmt.init.is_some();
        let has_test = stmt.test.is_some();
        let has_update = stmt.update.is_some();
        let has_any = has_init || has_test || has_update;

        // Build "for" + optional keyword comments + " (" prefix
        let for_open = if let Some(kc) = keyword_comments {
            d.concat(&[d.text("for"), kc, d.text(" (")])
        } else {
            d.text("for (")
        };

        // Check if there are any comments inside the for parens
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren_approx = open_paren.and_then(|p| self.matching_close_paren(p));
        let has_comments_inside =
            if let (Some(open), Some(close)) = (open_paren, close_paren_approx) {
                self.has_comments_between(open, close)
            } else {
                false
            };

        if !has_any && !has_comments_inside {
            // Empty for (;;) with no comments - no wrapping needed
            return d.concat(&[for_open, d.text(";;)")]);
        }

        if !has_any && has_comments_inside {
            // Empty for (;;) with comments - need to preserve them
            // This is a divergence from prettier (see for_empty_clauses_prettier_divergence)
            return self.build_for_empty_with_comments(stmt);
        }

        // Find paren position for detecting leading comments before init
        let open_paren = self.find_open_paren_after(stmt.span.start);

        // Determine spans for each part
        let init_end = stmt.init.as_ref().map(|i| self.get_for_init_span_end(i));
        let test_end = stmt.test.as_ref().map(|t| t.span().end);
        let update_end = stmt.update.as_ref().map(|u| u.span().end);

        let spans = ForHeaderSpans {
            open_paren,
            init_start: stmt.init.as_ref().map(|i| match i {
                internal::ForInit::VariableDeclaration(d) => d.span.start,
                internal::ForInit::Expression(e) => e.span().start,
            }),
            init_end,
            test_start: stmt.test.as_ref().map(|t| t.span().start),
            test_end,
            update_start: stmt.update.as_ref().map(|u| u.span().start),
            close_paren: open_paren.and_then(|o| self.matching_close_paren(o)),
        };

        // Check if we have any own-line comments that force expansion. A line
        // comment anywhere in the header also forces it: the `//` runs to end of
        // line, so the clauses after it must move to their own lines (matching
        // prettier) — otherwise the comment swallows the rest of the header.
        let has_line_comment_in_header =
            if let (Some(open), Some(close)) = (open_paren, spans.close_paren) {
                self.has_line_comments_in_range(open + 1, close)
            } else {
                false
            };
        let has_own_line_comments = force_break
            || has_line_comment_in_header
            || self.for_header_has_own_line_comments(&spans);

        // Extract span positions for use throughout this function
        let init_start = spans.init_start;
        let test_start = spans.test_start;
        let update_start = spans.update_start;
        let close_paren = spans.close_paren;

        let mut inner_parts = Vec::new();

        // Leading comments before init (after open paren)
        // Handles both own-line comments (with hardlines) and inline block comments
        if let (Some(open), Some(first_start)) =
            (open_paren, init_start.or(test_start).or(update_start))
        {
            let leading = self.build_for_clause_leading_comments(open + 1, first_start);
            if !leading.is_empty() {
                inner_parts.extend(leading);
            }

            // Inline block comments before the first clause (on the same line)
            // e.g., `for (/* before init */ let j = 0; ...)`
            for comment in tsv_lang::comments_in_range(self.comments, open + 1, first_start) {
                if comment.is_block && self.is_same_line(comment.span.end, first_start) {
                    inner_parts.push(self.build_comment_doc(comment));
                    inner_parts.push(d.text(" "));
                }
            }
        }

        // Find semicolon positions for proper comment boundary detection
        // The semicolons in `for (init; test; update)` are at specific positions in source
        let first_semi = self.source[stmt.span.start as usize..]
            .find(';')
            .map(|p| stmt.span.start + p as u32);
        let second_semi = first_semi.and_then(|p| {
            self.source[(p + 1) as usize..]
                .find(';')
                .map(|off| p + 1 + off as u32)
        });

        // Init part
        if let Some(init) = &stmt.init {
            if inner_parts.is_empty() {
                inner_parts.push(d.softline());
            }
            inner_parts.push(self.build_for_init_doc(init));
        }
        // Block comment trailing the init clause stays before its `;` (`a /* c */;`);
        // a line comment is relocated to after the `;` (`a; // c`).
        self.push_for_clause_trailing_comments(&mut inner_parts, init_end, first_semi, true);
        inner_parts.push(d.text(";"));
        self.push_for_clause_trailing_comments(&mut inner_parts, init_end, first_semi, false);

        // Inline comments after init (between semicolon and test, on same line as init)
        if let (Some(semi), Some(end)) = (first_semi, init_end) {
            let boundary = test_start
                .or(update_start)
                .or(close_paren)
                .unwrap_or(stmt.span.end);
            for comment in tsv_lang::comments_in_range(self.comments, semi + 1, boundary) {
                if self.is_same_line(end, comment.span.start) {
                    inner_parts.push(d.text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // Leading comments before test (own line, between first semi and test)
        if let Some(start) = test_start {
            let search_start = first_semi.map_or_else(
                || {
                    init_end.unwrap_or_else(|| {
                        open_paren.map_or_else(|| stmt.span.start + 5, |p| p + 1)
                    })
                },
                |s| s + 1,
            );
            let leading =
                self.build_for_clause_leading_comments_with_prev(search_start, start, init_end);
            if !leading.is_empty() {
                inner_parts.extend(leading);
            } else if has_init {
                inner_parts.push(d.line());
            }

            // Inline block comments before test (on same line)
            // e.g., `for (let i = 0; /* before test */ i < 10; ...)`
            for comment in tsv_lang::comments_in_range(self.comments, search_start, start) {
                if comment.is_block
                    && self.is_same_line(comment.span.end, start)
                    && init_end.is_none_or(|ie| !self.is_same_line(ie, comment.span.start))
                {
                    inner_parts.push(self.build_comment_doc(comment));
                    inner_parts.push(d.text(" "));
                }
            }
        } else if has_update {
            inner_parts.push(d.line());
        }

        // Test part
        if let Some(test) = &stmt.test {
            if !has_init && inner_parts.len() == 1 {
                // Only ";" so far, add line (becomes space in flat mode, newline when breaking)
                inner_parts.push(d.line());
            }
            // Wrap in group so binary chains (Ungrouped mode) have a tight parent
            // to evaluate fit against — matching how if/while use build_condition_group.
            // Without this, logical operators break with the for-header group (too wide)
            // instead of their own condition width.
            let condition_doc = self.build_condition_doc(test);
            inner_parts.push(d.group(condition_doc));
        }
        // Block comment trailing the test clause stays before its `;`; a line comment
        // is relocated to after it.
        self.push_for_clause_trailing_comments(&mut inner_parts, test_end, second_semi, true);
        inner_parts.push(d.text(";"));
        self.push_for_clause_trailing_comments(&mut inner_parts, test_end, second_semi, false);

        // Inline comments after test (between second semicolon and update, on same line as test)
        if let (Some(semi), Some(end)) = (second_semi, test_end) {
            let boundary = update_start.or(close_paren).unwrap_or(stmt.span.end);
            for comment in tsv_lang::comments_in_range(self.comments, semi + 1, boundary) {
                if self.is_same_line(end, comment.span.start) {
                    inner_parts.push(d.text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // Leading comments before update (own line, between second semi and update)
        if let Some(start) = update_start {
            let search_start = second_semi.map_or_else(
                || {
                    test_end.or(init_end).unwrap_or_else(|| {
                        open_paren.map_or_else(|| stmt.span.start + 5, |p| p + 1)
                    })
                },
                |s| s + 1,
            );
            let leading =
                self.build_for_clause_leading_comments_with_prev(search_start, start, test_end);
            if !leading.is_empty() {
                inner_parts.extend(leading);
            } else {
                inner_parts.push(d.line());
            }

            // Inline block comments before update (on same line)
            // e.g., `for (let i = 0; i < 10; /* before update */ i++)`
            for comment in tsv_lang::comments_in_range(self.comments, search_start, start) {
                if comment.is_block
                    && self.is_same_line(comment.span.end, start)
                    && test_end.is_none_or(|te| !self.is_same_line(te, comment.span.start))
                {
                    inner_parts.push(self.build_comment_doc(comment));
                    inner_parts.push(d.text(" "));
                }
            }
        }

        // Update part
        if let Some(update) = &stmt.update {
            if !has_init && !has_test && inner_parts.len() == 2 {
                // Only ";;" so far, add line (becomes space in flat mode)
                inner_parts.push(d.line());
            }
            inner_parts.push(self.build_for_update_doc(update));
            // Inline comments after update (on same line as update expression)
            if let Some(end) = update_end {
                let boundary = close_paren.unwrap_or(stmt.span.end);
                for comment in tsv_lang::comments_in_range(self.comments, end, boundary) {
                    if self.is_same_line(end, comment.span.start) {
                        inner_parts.push(d.text(" "));
                        inner_parts.push(self.build_comment_doc(comment));
                    }
                }
            }
        } else if has_test && !has_own_line_comments {
            // Prettier adds trailing space when update is None but test exists (no comments)
            inner_parts.push(d.if_break(d.empty(), d.text(" ")));
        }

        let closing = if has_own_line_comments {
            d.hardline()
        } else {
            d.softline()
        };

        d.group(d.concat(&[
            for_open,
            d.indent(d.concat(&inner_parts)),
            closing,
            d.text(")"),
        ]))
    }

    /// Build leading comments for a for clause (comments on their own line before the clause)
    ///
    /// `search_start` - where to start looking for comments
    /// `clause_start` - start of the next clause
    /// `prev_expr_end` - end of the previous expression (to filter out inline comments)
    fn build_for_clause_leading_comments_with_prev(
        &self,
        search_start: u32,
        clause_start: u32,
        prev_expr_end: Option<u32>,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in tsv_lang::comments_in_range(self.comments, search_start, clause_start) {
            // Only include comments that are:
            // 1. NOT on the same line as the next clause
            // 2. NOT on the same line as the previous expression (inline comments)
            let is_own_line_before_clause = !self.is_same_line(comment.span.end, clause_start);
            let is_own_line_after_prev =
                prev_expr_end.is_none_or(|end| !self.is_same_line(end, comment.span.start));
            if is_own_line_before_clause && is_own_line_after_prev {
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }
        if !parts.is_empty() {
            parts.push(d.hardline());
        }
        parts
    }

    /// Build leading comments for a for clause (comments on their own line before the clause)
    fn build_for_clause_leading_comments(&self, start: u32, clause_start: u32) -> Vec<DocId> {
        self.build_for_clause_leading_comments_with_prev(start, clause_start, None)
    }

    /// Check if for header has any own-line comments that force expansion
    fn for_header_has_own_line_comments(&self, spans: &ForHeaderSpans) -> bool {
        // Check for leading comments before first clause
        if let (Some(open), Some(first)) = (
            spans.open_paren,
            spans.init_start.or(spans.test_start).or(spans.update_start),
        ) {
            let (_, own_line, _) = self.partition_comments_by_line(open + 1, first);
            if !own_line.is_empty() {
                return true;
            }
        }

        // Check between init and test (or init and update if no test)
        let after_init = spans.test_start.or(spans.update_start);
        if let (Some(end), Some(start)) = (spans.init_end, after_init) {
            let (_, own_line, _) = self.partition_comments_by_line(end, start);
            if !own_line.is_empty() {
                return true;
            }
        }

        // Check between test and update
        if let (Some(end), Some(start)) = (spans.test_end, spans.update_start) {
            let (_, own_line, _) = self.partition_comments_by_line(end, start);
            if !own_line.is_empty() {
                return true;
            }
        }

        false
    }

    /// Emit comments in `start..end` matching `want_block`, each inline with a
    /// leading space. No-op unless both bounds are known.
    ///
    /// Used for the gap between a for-clause expression and its `;`: block comments
    /// stay before the `;` (`for (a /* c */; ...)`), line comments are relocated to
    /// after it (`a; // c`) — so the caller picks the kind and the insertion point.
    fn push_for_clause_trailing_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: Option<u32>,
        end: Option<u32>,
        want_block: bool,
    ) {
        let (Some(start), Some(end)) = (start, end) else {
            return;
        };
        let d = self.d();
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            if comment.is_block == want_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
    }

    /// Build a Doc for a for loop update expression
    fn build_for_update_doc(&self, expr: &Expression) -> DocId {
        let d = self.d();
        if let Expression::SequenceExpression(seq) = expr {
            d.join(
                seq.expressions.iter().map(|e| self.build_expression_doc(e)),
                ", ",
            )
        } else {
            self.build_expression_doc(expr)
        }
    }

    /// Build a complete for-in statement doc including the body
    fn build_for_in_statement_with_body_doc(&self, stmt: &internal::ForInStatement) -> DocId {
        let d = self.d();
        let left_start = self.get_for_in_of_left_start(&stmt.left);
        let left_end = self.get_for_in_of_left_end(&stmt.left);
        let right_start = stmt.right.span().start;
        let right_end = stmt.right.span().end;

        // Find 'in' keyword position (search with or without spaces)
        let in_pos = self
            .find_keyword_position(left_end, right_start, "in")
            .unwrap_or(left_end);

        // Preserve comments between `for` keyword and `(`
        let for_keyword_end = stmt.span.start + 3; // "for" is 3 chars
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));
        let keyword_comments = self.build_keyword_paren_comments(for_keyword_end, open_paren);

        // Check for line comments in the header - if present, use breaking layout
        let close = close_paren.unwrap_or(right_end + 1);
        let has_line_comments = if let Some(open) = open_paren {
            self.has_line_comments_in_range(open + 1, close)
        } else {
            self.has_line_comments_in_range(left_start, close)
        };

        if has_line_comments {
            return self.build_for_in_of_with_line_comments(
                &stmt.left,
                &stmt.right,
                &stmt.body,
                stmt.span.start,
                "in",
                in_pos,
                close_paren,
            );
        }

        let mut parts = if let Some(kc) = keyword_comments {
            vec![d.text("for"), kc, d.text(" (")]
        } else {
            vec![d.text("for (")]
        };

        // Comments between ( and left
        if let Some(open) = open_paren {
            for comment in tsv_lang::comments_in_range(self.comments, open + 1, left_start) {
                if comment.is_block {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
            }
        }

        parts.push(self.build_for_in_of_left_doc(&stmt.left));

        // Comments after left, before 'in'
        let has_left_comment = self.append_for_in_of_block_comments(&mut parts, left_end, in_pos);

        if has_left_comment {
            parts.push(d.text("in"));
        } else {
            parts.push(d.text(" in"));
        }

        // Comments after 'in', before right
        let in_keyword_end = in_pos + 2; // "in" is 2 chars
        let has_comment =
            self.append_for_in_of_block_comments(&mut parts, in_keyword_end, right_start);
        if !has_comment {
            parts.push(d.text(" "));
        }

        parts.push(self.build_expression_doc(&stmt.right));

        // Comments after right, before close paren (no trailing space needed)
        if let Some(close) = close_paren {
            self.append_for_in_of_trailing_comments(&mut parts, right_end, close);
        }

        // Check for comments between ) and body
        let paren_end = close_paren.map_or(right_end + 1, |p| p + 1);

        // Prettier expands empty blocks for for-in
        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            self.append_close_paren_with_non_block_body(&mut parts, paren_end, &stmt.body);
        }

        // Group so a non-block body's `adjustClause` line breaks on overflow
        // (matches Prettier's `printForXStatement`).
        d.group(d.concat(&parts))
    }

    /// Find a keyword position between two spans, skipping over comments
    ///
    /// Searches for the keyword with possible surrounding whitespace or comments.
    /// Returns the position where the keyword starts.
    fn find_keyword_position(&self, start: u32, end: u32, keyword: &str) -> Option<u32> {
        let search_range = &self.source[start as usize..end as usize];

        // First try to find " keyword " (with spaces) - outside of comments
        // We need to search manually to avoid matching inside comment content
        let keyword_bytes = keyword.as_bytes();
        let bytes = search_range.as_bytes();
        let len = bytes.len();
        let kw_len = keyword.len();
        let mut i = 0;

        while i + kw_len <= len {
            // Skip over comments
            if let Some(new_i) = crate::printer::analysis::skip_comment(bytes, i, len) {
                i = new_i;
                continue;
            }

            // Check if we found the keyword
            if &bytes[i..i + kw_len] == keyword_bytes {
                // Check it's not part of an identifier
                let before_ok =
                    i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
                let after_ok = i + kw_len >= len
                    || !bytes[i + kw_len].is_ascii_alphanumeric() && bytes[i + kw_len] != b'_';

                if before_ok && after_ok {
                    return Some(start + i as u32);
                }
            }
            i += 1;
        }

        None
    }

    /// Build a complete for-of statement doc including the body
    fn build_for_of_statement_with_body_doc(&self, stmt: &internal::ForOfStatement) -> DocId {
        let d = self.d();
        let left_start = self.get_for_in_of_left_start(&stmt.left);
        let left_end = self.get_for_in_of_left_end(&stmt.left);
        let right_start = stmt.right.span().start;
        let right_end = stmt.right.span().end;

        // Find 'of' keyword position (search with or without spaces)
        let of_pos = self
            .find_keyword_position(left_end, right_start, "of")
            .unwrap_or(left_end);

        // Preserve comments between keywords and `(`
        // for await: two gaps — for-to-await and await-to-paren
        // for (non-await): one gap — for-to-paren
        let for_keyword_end = stmt.span.start + 3; // "for" is 3 chars
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));
        let (for_await_comments, await_paren_comments) = if stmt.r#await {
            let await_pos = self.find_keyword_in_source(for_keyword_end, left_start, "await");
            let for_await_c = await_pos
                .and_then(|ap| self.build_inline_comments_between_doc_opt(for_keyword_end, ap));
            let await_paren_c = await_pos
                .map(|ap| ap + 5)
                .and_then(|ae| self.build_keyword_paren_comments(ae, open_paren));
            (for_await_c, await_paren_c)
        } else {
            (None, None)
        };
        let keyword_comments = if !stmt.r#await {
            self.build_keyword_paren_comments(for_keyword_end, open_paren)
        } else {
            None
        };

        // Check for line comments in the header - if present, use breaking layout
        // We check from open paren to close paren
        let close = close_paren.unwrap_or(right_end + 1);
        let has_line_comments = if let Some(open) = open_paren {
            self.has_line_comments_in_range(open + 1, close)
        } else {
            self.has_line_comments_in_range(left_start, close)
        };

        if has_line_comments {
            return self.build_for_in_of_with_line_comments(
                &stmt.left,
                &stmt.right,
                &stmt.body,
                stmt.span.start,
                "of",
                of_pos,
                close_paren,
            );
        }

        let mut parts = vec![d.text("for")];
        if let Some(kc) = keyword_comments {
            parts.push(kc);
        }
        if let Some(fac) = for_await_comments {
            parts.push(fac);
        }
        parts.push(d.text(" "));
        if stmt.r#await {
            parts.push(d.text("await"));
            if let Some(apc) = await_paren_comments {
                parts.push(apc);
            }
            parts.push(d.text(" "));
        }
        parts.push(d.text("("));

        // Comments between ( and left
        if let Some(open) = open_paren {
            for comment in tsv_lang::comments_in_range(self.comments, open + 1, left_start) {
                if comment.is_block {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
            }
        }

        parts.push(self.build_for_in_of_left_doc(&stmt.left));

        // Comments after left, before 'of'
        let has_left_comment = self.append_for_in_of_block_comments(&mut parts, left_end, of_pos);

        if has_left_comment {
            parts.push(d.text("of"));
        } else {
            parts.push(d.text(" of"));
        }

        // Comments after 'of', before right
        let of_keyword_end = of_pos + 2; // "of" is 2 chars
        let has_comment =
            self.append_for_in_of_block_comments(&mut parts, of_keyword_end, right_start);
        if !has_comment {
            parts.push(d.text(" "));
        }

        parts.push(self.build_expression_doc(&stmt.right));

        // Comments after right, before close paren (no trailing space needed)
        if let Some(close) = close_paren {
            self.append_for_in_of_trailing_comments(&mut parts, right_end, close);
        }

        // Check for comments between ) and body
        let paren_end = close_paren.map_or(right_end + 1, |p| p + 1);

        // Prettier expands empty blocks for for-of
        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            self.append_close_paren_with_non_block_body(&mut parts, paren_end, &stmt.body);
        }

        // Group so a non-block body's `adjustClause` line breaks on overflow
        // (matches Prettier's `printForXStatement`).
        d.group(d.concat(&parts))
    }

    /// Build for-in/for-of statement with line comments preserved in their positions
    ///
    /// This is our divergence from Prettier - we preserve line comments where
    /// the user wrote them rather than relocating them.
    #[allow(clippy::too_many_arguments)]
    fn build_for_in_of_with_line_comments(
        &self,
        left: &internal::ForInOfLeft,
        right: &Expression,
        body: &Statement,
        stmt_start: u32,
        keyword: &str, // "in" or "of"
        keyword_pos: u32,
        close_paren: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let left_start = self.get_for_in_of_left_start(left);
        let left_end = self.get_for_in_of_left_end(left);
        let right_start = right.span().start;
        let right_end = right.span().end;
        let open_paren = self.find_open_paren_after(stmt_start);
        let keyword_end = keyword_pos + keyword.len() as u32;

        // Check for "for await" by looking at source before open paren
        let has_await = if let Some(open) = open_paren {
            let before_paren = &self.source[stmt_start as usize..open as usize];
            before_paren.contains("await")
        } else {
            false
        };

        let mut parts = if has_await {
            vec![d.text("for await (")]
        } else {
            vec![d.text("for (")]
        };

        // Inner content with hardline breaks
        let mut inner = Vec::new();

        // Comments before left (after open paren)
        if let Some(open) = open_paren {
            for comment in tsv_lang::comments_in_range(self.comments, open + 1, left_start) {
                inner.push(d.hardline());
                inner.push(self.build_comment_doc(comment));
            }
        }

        // Left side (const y)
        inner.push(d.hardline());
        inner.push(self.build_for_in_of_left_doc(left));

        // Comments after left, before keyword — emit all (own-line comments normalize to inline)
        for comment in tsv_lang::comments_in_range(self.comments, left_end, keyword_pos) {
            inner.push(d.text(" "));
            inner.push(self.build_comment_doc(comment));
        }

        // Keyword with extra indent (hardline is INSIDE the indent so keyword gets extra indent)
        let keyword_doc = match keyword {
            "in" => d.text("in"),
            "of" => d.text("of"),
            _ => d.text("of"), // fallback
        };
        let mut keyword_parts = vec![d.hardline(), keyword_doc];

        // Comments after keyword, before right — emit all (own-line comments normalize to inline)
        for comment in tsv_lang::comments_in_range(self.comments, keyword_end, right_start) {
            keyword_parts.push(d.text(" "));
            keyword_parts.push(self.build_comment_doc(comment));
        }

        // Right side (items)
        keyword_parts.push(d.hardline());
        keyword_parts.push(self.build_expression_doc(right));

        // Comments after right, before close paren
        if let Some(close) = close_paren {
            for comment in tsv_lang::comments_in_range(self.comments, right_end, close) {
                keyword_parts.push(d.text(" "));
                keyword_parts.push(self.build_comment_doc(comment));
            }
        }

        inner.push(d.indent(d.concat(&keyword_parts)));

        parts.push(d.indent(d.concat(&inner)));
        parts.push(d.hardline());

        // Comments between ) and body (matching inline path)
        let paren_end = close_paren.map_or(right_end + 1, |p| p + 1);

        // Body
        if let Statement::BlockStatement(block) = body {
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            self.append_close_paren_with_non_block_body(&mut parts, paren_end, body);
        }

        // Group so the non-block body's `adjustClause` line breaks (the
        // hardline-broken header forces this group open via `will_break`).
        d.group(d.concat(&parts))
    }

    /// Get the end position of the left side of a for-in/for-of statement
    fn get_for_in_of_left_end(&self, left: &internal::ForInOfLeft) -> u32 {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => decl.span.end,
            internal::ForInOfLeft::Pattern(expr) => expr.span().end,
        }
    }

    /// Get the start position of the left side of a for-in/for-of statement
    fn get_for_in_of_left_start(&self, left: &internal::ForInOfLeft) -> u32 {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => decl.span.start,
            internal::ForInOfLeft::Pattern(expr) => expr.span().start,
        }
    }

    /// Check if there are any line comments in the given range
    fn has_line_comments_in_range(&self, start: u32, end: u32) -> bool {
        tsv_lang::comments_in_range(self.comments, start, end).any(|c| !c.is_block)
    }

    /// Append inline block comments for for-in/for-of statements.
    /// Emits ` comment` for each block comment, plus trailing ` ` if any were added.
    /// Own-line comments normalize to inline. Line comments are skipped (handled by
    /// the breaking layout path).
    /// Returns true if any comments were added.
    fn append_for_in_of_block_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
    ) -> bool {
        let d = self.d();
        let mut added = false;
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                added = true;
            }
        }
        if added {
            parts.push(d.text(" "));
        }
        added
    }

    /// Append trailing block comments for for-in/for-of statements.
    /// Own-line comments normalize to inline. No trailing space.
    fn append_for_in_of_trailing_comments(&self, parts: &mut Vec<DocId>, start: u32, end: u32) {
        let d = self.d();
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
    }

    /// Build a doc for a while statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the condition wraps to multiple lines
    /// when the `while (condition)` line exceeds print width.
    fn build_while_statement_with_wrapping_doc(&self, stmt: &internal::WhileStatement) -> DocId {
        let d = self.d();
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `while` keyword and `(` in place:
        //   while/* c */(a){} → while /* c */ (a) {}
        let while_keyword_end = stmt.span.start + 5; // "while" is 5 chars
        let keyword_comments = self.build_keyword_paren_comments(while_keyword_end, open_paren);

        // Build condition group (handles breaking within condition and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.test, open, close)
        } else {
            self.build_condition_group(&stmt.test)
        };

        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            // Block body: while (cond) { ... }
            // Uses append_close_paren_with_comments for consistency with if/for-in/for-of:
            // block comments stay inline, line comments become trailing.
            let mut parts = vec![d.text("while")];
            if let Some(kc) = &keyword_comments {
                parts.push(*kc);
            }
            parts.push(d.text(" ("));
            parts.push(condition_group);
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_doc(block));
            d.group(d.concat(&parts))
        } else if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            // Empty statement: `while (cond);` or `while (cond) /* comment */ ;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let empty_start = stmt.body.span().start;

            let mut empty_parts = vec![d.text("while")];
            if let Some(kc) = &keyword_comments {
                empty_parts.push(*kc);
            }
            empty_parts.push(d.text(" ("));
            empty_parts.push(condition_group);
            self.append_close_paren_empty_stmt_with_comments(
                &mut empty_parts,
                paren_end,
                empty_start,
            );

            d.group(d.concat(&empty_parts))
        } else {
            // Non-block body: use adjustClause equivalent
            // - When flat: line becomes space -> `while (cond) a;`
            // - When broken: line becomes newline + indent -> `while (cond)\n\ta;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let body_start = stmt.body.span().start;
            let body_doc = self.build_statement_doc(&stmt.body);

            let mut head_parts = vec![d.text("while")];
            if let Some(kc) = &keyword_comments {
                head_parts.push(*kc);
            }
            head_parts.push(d.text(" ("));
            head_parts.push(condition_group);
            self.build_adjust_clause_with_comments(&head_parts, paren_end, body_start, body_doc)
        }
    }

    /// Build a doc for a switch statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the discriminant wraps to multiple lines
    /// when the `switch (discriminant) {` line exceeds print width.
    fn build_switch_statement_with_wrapping_doc(&self, stmt: &internal::SwitchStatement) -> DocId {
        let d = self.d();
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `switch` keyword and `(` in place:
        //   switch/* c */(a){} → switch /* c */ (a) {}
        let switch_keyword_end = stmt.span.start + 6; // "switch" is 6 chars
        let keyword_comments = self.build_keyword_paren_comments(switch_keyword_end, open_paren);

        // Preserve comments between ) and { in place:
        //   switch(x)/* c */{} → switch (x) /* c */ {}
        let body_open_brace = close_paren.and_then(|close| {
            self.source[close as usize + 1..stmt.span.end as usize]
                .find('{')
                .map(|p| close + 1 + p as u32)
        });
        let paren_brace_comments = match (close_paren, body_open_brace) {
            (Some(close), Some(brace)) if self.has_comments_between(close + 1, brace) => {
                self.build_inline_comments_between_doc_opt(close + 1, brace)
            }
            _ => None,
        };

        // Build condition group (handles breaking within discriminant and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.discriminant, open, close)
        } else {
            self.build_condition_group(&stmt.discriminant)
        };

        // Build cases - they handle their own internal indentation
        // Join cases with hardlines, handling comments between cases
        let mut case_parts = Vec::new();
        // Start after the open brace to find comments between { and first case
        let brace_start = body_open_brace
            .unwrap_or_else(|| close_paren.map_or_else(|| stmt.discriminant.span().end, |p| p + 1));
        let mut prev_end = brace_start + 1;
        let mut prev_case_label_end: Option<u32> = None;
        let mut is_first_item = true;
        for (i, case) in stmt.cases.iter().enumerate() {
            // Handle comments between previous position and this case
            // (includes comments before first case, and between subsequent cases)
            // Skip inline comments that belong to the previous case label (fallthrough cases)
            let comments: Vec<_> =
                tsv_lang::comments_in_range(self.comments, prev_end, case.span.start).collect();
            let mut last_content_end = prev_end;
            for comment in &comments {
                // Skip comments that are on the same line as the previous case label
                // Those are inline comments for the case (e.g., `case 3: // fallthrough`)
                if prev_case_label_end
                    .is_some_and(|label_end| self.is_same_line(label_end, comment.span.start))
                {
                    continue;
                }
                // Add hardline before comment (except for very first item - body_doc handles that)
                // Preserve blank lines before comments (e.g., between `return;` and `// comment`)
                if !is_first_item {
                    if self.has_blank_line_between(last_content_end, comment.span.start) {
                        case_parts.push(d.literalline());
                    }
                    case_parts.push(d.hardline());
                }
                is_first_item = false;
                case_parts.push(self.build_comment_doc(comment));
                last_content_end = comment.span.end;
            }
            // Add hardline before case (except for very first item)
            // Preserve blank lines between cases (check from last content, not prev_end)
            if !is_first_item {
                // Check for blank line between last content (case or comment) and current case
                if self.has_blank_line_between(last_content_end, case.span.start) {
                    case_parts.push(d.literalline());
                }
                case_parts.push(d.hardline());
            }
            is_first_item = false;

            // Determine the end boundary for inline comments on this case
            // For empty cases (fallthrough), we need to look ahead to the next case
            let next_case_start = stmt.cases.get(i + 1).map(|c| c.span.start);
            let inline_comment_boundary = next_case_start.unwrap_or(stmt.span.end - 1);

            case_parts.push(self.build_switch_case_doc_inner(case, inline_comment_boundary));

            // Track case label end for filtering inline comments in next iteration
            prev_case_label_end = Some(self.get_case_label_end(case));
            prev_end = case.span.end;
        }

        // Handle trailing comments after the last case (before closing `}`)
        // Also handles comments in empty switch bodies
        let switch_end = stmt.span.end - 1; // Before '}'
        let mut last_trailing_end = prev_end;
        for comment in tsv_lang::comments_in_range(self.comments, prev_end, switch_end) {
            if !is_first_item {
                if self.has_blank_line_between(last_trailing_end, comment.span.start) {
                    case_parts.push(d.literalline());
                }
                case_parts.push(d.hardline());
            }
            is_first_item = false;
            case_parts.push(self.build_comment_doc(comment));
            last_trailing_end = comment.span.end;
        }

        // Structure: switch (...) { indent([hardline, cases...]) hardline }
        // The indent wraps the hardline so cases start at +1 indent level
        // For empty switch, just output {\n}
        let body_doc = if case_parts.is_empty() {
            d.hardline()
        } else {
            d.concat(&[
                d.indent(d.concat(&[d.hardline(), d.concat(&case_parts)])),
                d.hardline(),
            ])
        };

        let mut switch_parts = vec![d.text("switch")];
        if let Some(kc) = keyword_comments {
            switch_parts.push(kc);
        }
        switch_parts.push(d.text(" ("));
        switch_parts.push(condition_group);
        switch_parts.push(d.text(")"));
        if let Some(pbc) = paren_brace_comments {
            switch_parts.push(pbc);
        }
        switch_parts.push(d.text(" {"));
        switch_parts.push(body_doc);
        switch_parts.push(d.text("}"));
        d.group(d.concat(&switch_parts))
    }

    /// Get the end position of a case label (position after the colon)
    fn get_case_label_end(&self, case: &internal::SwitchCase) -> u32 {
        if let Some(test) = &case.test {
            // Find ':' after the test expression
            let test_end = test.span().end;
            self.source[test_end as usize..]
                .find(':')
                .map_or_else(|| test_end + 1, |p| test_end + p as u32 + 1)
        } else {
            // "default:" - find the actual ':' position
            self.source[case.span.start as usize..]
                .find(':')
                .map_or(case.span.start + 8, |p| case.span.start + p as u32 + 1)
        }
    }

    /// Build a doc for a switch case (without outer indent - that's handled by switch)
    ///
    /// `inline_comment_boundary` is the position up to which we should look for inline comments
    /// on this case label (typically the next case start or switch body end).
    fn build_switch_case_doc_inner(
        &self,
        case: &internal::SwitchCase,
        inline_comment_boundary: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // case X: or default:
        let case_label_end = self.get_case_label_end(case);

        if let Some(test) = &case.test {
            parts.push(d.text("case "));
            parts.push(self.build_expression_doc(test));
            // Comments between expression and colon: `case 1 /* c */:`
            let test_end = test.span().end;
            let colon_pos = find_char_skipping_comments(
                self.source.as_bytes(),
                test_end as usize,
                case_label_end as usize,
                b':',
            )
            .unwrap_or(case_label_end as usize - 1);
            parts.push(self.build_inline_comments_between_doc(test_end, colon_pos as u32));
            parts.push(d.text(":"));
        } else {
            // Comments between `default` keyword and colon: `default /* c */:`
            let default_keyword_end = case.span.start + 7; // "default".len()
            let colon_pos = find_char_skipping_comments(
                self.source.as_bytes(),
                default_keyword_end as usize,
                case_label_end as usize,
                b':',
            )
            .unwrap_or(case_label_end as usize - 1);
            parts.push(d.text("default"));
            parts.push(
                self.build_inline_comments_between_doc(default_keyword_end, colon_pos as u32),
            );
            parts.push(d.text(":"));
        }

        // Handle inline comments after case label (e.g., `case 1: // comment`)
        // For fallthrough cases (no consequent), use the boundary passed by the switch printer
        let first_stmt_start = case.consequent.first().map(|s| s.span().start);
        let inline_comment_end = first_stmt_start.unwrap_or(inline_comment_boundary);
        let mut has_inline_line_comment = false;
        for comment in
            tsv_lang::comments_in_range(self.comments, case_label_end, inline_comment_end)
        {
            if self.is_same_line(case_label_end, comment.span.start) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                if !comment.is_block {
                    has_inline_line_comment = true;
                }
            }
        }

        // Consequent statements (indented from case line)
        // Handle comments between statements like block statements do
        let mut prev_end = case_label_end;
        let mut prev_stmt_end: Option<u32> = None;

        // Check if first statement is a block - it hugs the case label: `case 'a': { ... }`
        let first_is_block = case
            .consequent
            .first()
            .is_some_and(|s| matches!(s, Statement::BlockStatement(_)));

        for (i, stmt) in case.consequent.iter().enumerate() {
            let stmt_start = stmt.span().start;

            // Check for comments between previous position and this statement
            let comments: Vec<_> =
                tsv_lang::comments_in_range(self.comments, prev_end, stmt_start).collect();

            // Filter out:
            // 1. Trailing same-line comments from the previous statement
            // 2. Inline comments after the case label (already handled above)
            let leading_comments: Vec<_> = if let Some(prev_stmt) = prev_stmt_end {
                comments
                    .iter()
                    .filter(|c| !self.is_same_line(prev_stmt, c.span.start))
                    .copied()
                    .collect()
            } else {
                // For first statement, filter out inline comments after case label
                comments
                    .iter()
                    .filter(|c| !self.is_same_line(case_label_end, c.span.start))
                    .copied()
                    .collect()
            };

            // First block statement hugs the case label: `case 'a': { ... }`
            // Unless there are line comments (inline after label or between label and block)
            if i == 0 && first_is_block {
                let has_leading_line_comment = leading_comments.iter().any(|c| !c.is_block);
                if !has_inline_line_comment && !has_leading_line_comment {
                    // Hug: `case 'a': { ... }`
                    for comment in &leading_comments {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                    parts.push(d.text(" "));
                    parts.push(self.build_statement_doc(stmt));
                } else if has_inline_line_comment && leading_comments.is_empty() {
                    // Inline line comment, no leading: `case 'a': // comment\n{`
                    // Block at case level (no indent)
                    parts.push(d.hardline());
                    parts.push(self.build_statement_doc(stmt));
                } else {
                    // Leading comments exist - indent both comments and block
                    // e.g., `case 'b':\n  // comment\n  {`
                    let mut stmt_parts = vec![d.hardline()];
                    for comment in &leading_comments {
                        stmt_parts.push(self.build_comment_doc(comment));
                        stmt_parts.push(d.hardline());
                    }
                    stmt_parts.push(self.build_statement_doc(stmt));
                    parts.push(d.indent(d.concat(&stmt_parts)));
                }
            } else {
                // Build the indented content for this statement
                let mut stmt_parts = vec![d.hardline()];

                // Preserve blank lines between statements within case consequent
                if prev_stmt_end.is_some() {
                    let check_end = leading_comments
                        .first()
                        .map_or(stmt_start, |c| c.span.start);
                    if self.has_blank_line_between(prev_end, check_end) {
                        stmt_parts.push(d.hardline());
                    }
                }

                // Print leading comments before this statement
                for comment in &leading_comments {
                    stmt_parts.push(self.build_comment_doc(comment));
                    if !comment.is_block {
                        // Line comment: add hardline after
                        stmt_parts.push(d.hardline());
                    } else if !self.is_same_line(comment.span.end, stmt_start) {
                        // Block comment not on same line as statement - add hardline
                        stmt_parts.push(d.hardline());
                    } else {
                        // Block comment on same line as statement - add space
                        stmt_parts.push(d.text(" "));
                    }
                }

                stmt_parts.push(self.build_statement_doc(stmt));

                parts.push(d.indent(d.concat(&stmt_parts)));
            }

            prev_end = stmt.span().end;
            prev_stmt_end = Some(stmt.span().end);
        }

        // Note: Trailing comments after the last statement are handled by the switch statement
        // printer since they fall outside the SwitchCase span.

        d.concat(&parts)
    }

    //
    // Control Flow Statement Doc Builders
    //

    pub(super) fn build_if_statement_doc(&self, stmt: &internal::IfStatement) -> DocId {
        // Check for comments between consequent and alternate that need special handling
        let has_if_else_comments = stmt.alternate.as_ref().is_some_and(|alt| {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alt.span().start;
            self.has_comments_between(consequent_end, alternate_start)
        });

        if has_if_else_comments {
            // Build doc with inline comments between } and else
            self.build_if_statement_with_comments_doc(stmt)
        } else {
            // Delegate to the sophisticated version that handles width-based wrapping
            self.build_if_statement_with_wrapping_doc(stmt)
        }
    }

    /// Build if statement doc with comments between consequent and alternate
    fn build_if_statement_with_comments_doc(&self, stmt: &internal::IfStatement) -> DocId {
        let d = self.d();
        // Build condition group (same as build_if_statement_with_wrapping_doc)
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));
        let if_keyword_end = stmt.span.start + 2;
        let keyword_comments = self.build_keyword_paren_comments(if_keyword_end, open_paren);
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.test, open, close)
        } else {
            self.build_condition_group(&stmt.test)
        };

        let mut parts = vec![d.text("if")];
        if let Some(kc) = keyword_comments {
            parts.push(kc);
        }
        parts.push(d.text(" ("));
        parts.push(condition_group);

        // Build consequent (with head-body comment extraction)
        let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
        if let Statement::BlockStatement(block) = stmt.consequent.as_ref() {
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else if matches!(stmt.consequent.as_ref(), Statement::EmptyStatement(_)) {
            let empty_start = stmt.consequent.span().start;
            self.append_close_paren_empty_stmt_with_comments(&mut parts, paren_end, empty_start);
        } else {
            // Non-block consequent: handle head-body comments between ) and body
            let body_start = stmt.consequent.span().start;
            let consequent_doc = self.build_statement_doc(&stmt.consequent);

            if self.has_comments_between(paren_end, body_start) {
                let has_line = self.has_line_comments_between(paren_end, body_start);
                let comment_doc =
                    self.build_inline_comments_between_doc_no_leading_space(paren_end, body_start);
                parts.push(d.text(")"));
                if has_line {
                    // Line comment forces break: if (cond)\n\t// comment\n\tbody;
                    parts.push(d.indent(d.concat(&[
                        d.hardline(),
                        comment_doc,
                        d.hardline(),
                        consequent_doc,
                    ])));
                } else {
                    // Block comment stays inline: if (cond) /* c */ body;
                    parts.push(d.text(" "));
                    parts.push(comment_doc);
                    parts.push(d.text(" "));
                    parts.push(consequent_doc);
                }
            } else if is_inline_consequent(&stmt.consequent) {
                parts.push(d.text(") "));
                parts.push(consequent_doc);
            } else {
                parts.push(d.text(")"));
                parts.push(d.indent(d.concat(&[d.hardline(), consequent_doc])));
            }
        }

        // Handle else with comments
        if let Some(alternate) = &stmt.alternate {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alternate.span().start;

            // Find "else" keyword to split comments into before-else and after-else
            let else_end = self.find_else_keyword_end_between(consequent_end, alternate_start);
            let else_start = else_end.map(|e| e - 4); // "else" is 4 chars

            // Comments between } and "else"
            let before_else_end = else_start.unwrap_or(alternate_start);
            if self.has_comments_between(consequent_end, before_else_end) {
                let (inline_prev, own_line, inline_next) =
                    self.partition_comments_by_line(consequent_end, before_else_end);

                // Merge inline_next (comments on same line as `else`) into own_line
                // so they're emitted before the `else` keyword rather than dropped.
                // e.g. `} \n /* b */ else {` → `}\n/* b */\nelse {`
                let mut all_own_line = own_line;
                all_own_line.extend(inline_next);

                self.build_comments_between_parts(
                    &mut parts,
                    &inline_prev,
                    &all_own_line,
                    consequent_end,
                );

                let has_inline_line_comment = inline_prev.iter().any(|c| !c.is_block);
                let is_block_consequent =
                    matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_));
                if is_block_consequent && all_own_line.is_empty() && !has_inline_line_comment {
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            } else if matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_)) {
                // Block body: `} else` on same line
                parts.push(d.text(" "));
            } else {
                // Empty statement or non-block body: `else` on new line
                parts.push(d.hardline());
            }

            // Comments between "else" and alternate body
            if matches!(alternate.as_ref(), Statement::EmptyStatement(_)) {
                // Empty alternate: `else;`, `else /* c */ ;`, or `else // c\n;`
                if let Some(else_e) = else_end
                    && self.has_comments_between(else_e, alternate_start)
                {
                    let has_line = self.has_line_comments_between(else_e, alternate_start);
                    parts.push(d.text("else"));
                    parts.push(self.build_inline_comments_between_doc(else_e, alternate_start));
                    if has_line {
                        // Line comment: `else // c\n;`
                        parts.push(d.hardline());
                        parts.push(d.text(";"));
                    } else {
                        // Block comment: `else /* c */ ;`
                        parts.push(d.text(" ;"));
                    }
                } else {
                    parts.push(d.text("else;"));
                }
            } else if let Some(else_e) = else_end
                && self.has_comments_between(else_e, alternate_start)
            {
                let has_line = self.has_line_comments_between(else_e, alternate_start);
                let is_non_block_non_if = !matches!(
                    alternate.as_ref(),
                    Statement::BlockStatement(_) | Statement::IfStatement(_)
                );
                parts.push(d.text("else"));
                parts.push(self.build_inline_comments_between_doc(else_e, alternate_start));
                if has_line && is_non_block_non_if {
                    // Line comment + non-block body: comment stays on else line, body indented
                    // else // c\n\texpr;
                    let body_doc = self.build_statement_doc(alternate);
                    parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
                } else if has_line {
                    parts.push(d.hardline());
                    self.append_else_body_doc(&mut parts, alternate, true);
                } else {
                    parts.push(d.text(" "));
                    self.append_else_body_doc(&mut parts, alternate, true);
                }
            } else {
                parts.push(d.text("else "));
                self.append_else_body_doc(&mut parts, alternate, false);
            }
        }

        d.concat(&parts)
    }

    pub(super) fn build_for_statement_doc(&self, stmt: &internal::ForStatement) -> DocId {
        let d = self.d();

        // Preserve comments between `for` keyword and `(` in place:
        //   for/* c */(;;){} → for /* c */ (;;) {}
        let for_keyword_end = stmt.span.start + 3; // "for" is 3 chars
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let keyword_comments = self.build_keyword_paren_comments(for_keyword_end, open_paren);
        let has_pre_paren_comments = keyword_comments.is_some();

        // Check for comments between ) and body (Prettier 3.7 #18108)
        let header_end = self.get_for_header_end(stmt);
        let body_start = stmt.body.span().start;

        if has_pre_paren_comments || self.has_comments_between(header_end, body_start) {
            // Check if we have line comments (need special handling)
            let has_line_comment = self.has_line_comments_between(header_end, body_start);

            // Build parts with proper comment handling. A line comment between `)` and
            // the body forces the header to break (block comments can stay inline).
            let mut parts =
                vec![self.build_for_header_doc_impl(stmt, has_line_comment, keyword_comments)];

            // Post-header comments. Non-block bodies use Prettier's `adjustClause`
            // (`indent([line, body])`) wrapped with the header in an outer group, so
            // the body drops to its own indented line when the header breaks (a
            // comment hardline propagates) or the whole thing overflows — while the
            // header group still decides its own flat/break.
            let is_block_body = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));
            let body_doc = self.build_statement_doc(&stmt.body);

            let (tail, group_it) = if self.has_comments_between(header_end, body_start) {
                let comment_doc =
                    self.build_inline_comments_between_doc_no_leading_space(header_end, body_start);
                if has_line_comment && !is_block_body {
                    // Line comment, non-block body: comment + body on their own lines.
                    // for (;;)\n\t// c\n\texpr;
                    (
                        d.indent(d.concat(&[d.hardline(), comment_doc, d.hardline(), body_doc])),
                        false,
                    )
                } else if has_line_comment {
                    // Line comment, block body: comment trailing `)`, block on next line.
                    (
                        d.concat(&[d.text(" "), comment_doc, d.hardline(), body_doc]),
                        false,
                    )
                } else if is_block_body {
                    // Block comment, block body: `) /* c */ {`
                    (
                        d.concat(&[d.text(" "), comment_doc, d.text(" "), body_doc]),
                        false,
                    )
                } else {
                    // Block comment, non-block body: adjustClause keeps `) /* c */ body`
                    // flat but drops `\n\t/* c */ body` when the header breaks.
                    (
                        d.indent_line(d.concat(&[comment_doc, d.text(" "), body_doc])),
                        true,
                    )
                }
            } else if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
                // Empty body attaches directly: `);` (no space, no adjustClause).
                // Matches the main path (`build_for_statement_with_body_doc`) and Prettier.
                (body_doc, false)
            } else if is_block_body {
                (d.concat(&[d.text(" "), body_doc]), false)
            } else {
                (d.indent_line(body_doc), true)
            };

            parts.push(tail);
            if group_it {
                d.group(d.concat(&parts))
            } else {
                d.concat(&parts)
            }
        } else {
            // Delegate to the sophisticated version that handles all edge cases
            self.build_for_statement_with_body_doc(stmt)
        }
    }

    fn build_for_init_doc(&self, init: &internal::ForInit) -> DocId {
        let d = self.d();
        match init {
            internal::ForInit::VariableDeclaration(decl) => {
                let mut parts = vec![d.text(decl.kind.as_str()), d.text(" ")];
                for (i, declarator) in decl.declarations.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    parts.push(self.build_expression_doc(&declarator.id));
                    if let Some(init) = &declarator.init {
                        let id_end = declarator.id.span().end;
                        let init_start = init.span().start;
                        let eq_pos = self.find_equals_position(id_end, init_start);
                        parts.push(d.text(" = "));
                        if let Some(comments) = self.build_rhs_comments_opt(eq_pos + 1, init_start)
                        {
                            parts.push(comments);
                        }
                        parts.push(self.build_expression_doc(init));
                    }
                }
                d.concat(&parts)
            }
            internal::ForInit::Expression(expr) => {
                // Sequence expressions in for loop init don't need outer parens
                // e.g., `for (i = 0, j = 0; ...)` not `for ((i = 0, j = 0); ...)`
                // Same handling as build_for_update_doc
                if let Expression::SequenceExpression(seq) = expr {
                    d.join(
                        seq.expressions.iter().map(|e| self.build_expression_doc(e)),
                        ", ",
                    )
                } else {
                    self.build_expression_doc(expr)
                }
            }
        }
    }

    pub(super) fn build_for_in_statement_doc(&self, stmt: &internal::ForInStatement) -> DocId {
        // Delegate to the sophisticated version that handles empty block expansion
        self.build_for_in_statement_with_body_doc(stmt)
    }

    pub(super) fn build_for_of_statement_doc(&self, stmt: &internal::ForOfStatement) -> DocId {
        // Delegate to the sophisticated version that handles empty block expansion
        self.build_for_of_statement_with_body_doc(stmt)
    }

    fn build_for_in_of_left_doc(&self, left: &internal::ForInOfLeft) -> DocId {
        let d = self.d();
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => {
                let mut parts = vec![d.text(decl.kind.as_str()), d.text(" ")];
                if let Some(declarator) = decl.declarations.first() {
                    parts.push(self.build_expression_doc(&declarator.id));
                }
                d.concat(&parts)
            }
            internal::ForInOfLeft::Pattern(expr) => self.build_expression_doc(expr),
        }
    }

    pub(super) fn build_while_statement_doc(&self, stmt: &internal::WhileStatement) -> DocId {
        // Delegate to the wrapping version for proper condition grouping
        self.build_while_statement_with_wrapping_doc(stmt)
    }

    pub(super) fn build_do_while_statement_doc(&self, stmt: &internal::DoWhileStatement) -> DocId {
        let d = self.d();
        let is_block = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));

        // Check for comments between `do` keyword and body
        let do_end = stmt.span.start + 2; // "do" is 2 chars
        let body_start = stmt.body.span().start;
        let mut parts = if self.has_comments_between(do_end, body_start) {
            let has_line = self.has_line_comments_between(do_end, body_start);
            let comment_doc =
                self.build_inline_comments_between_doc_no_leading_space(do_end, body_start);
            let body_doc = self.build_statement_doc(&stmt.body);
            let mut p = vec![d.text("do")];
            if has_line && !is_block {
                // Line comment with non-block body: indent comment + body
                // do\n\t// c\n\texpr;
                p.push(d.indent(d.concat(&[d.hardline(), comment_doc, d.hardline(), body_doc])));
            } else if has_line {
                // Line comment with block body: keep flat
                p.push(d.text(" "));
                p.push(comment_doc);
                p.push(d.hardline());
                p.push(body_doc);
            } else {
                p.push(d.text(" "));
                p.push(comment_doc);
                p.push(d.text(" "));
                p.push(body_doc);
            }
            p
        } else if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            // Prettier's `adjustClause` returns `";"` directly for an empty body
            // → `do;`, not `do ;`.
            vec![d.text("do"), self.build_statement_doc(&stmt.body)]
        } else {
            vec![d.text("do "), self.build_statement_doc(&stmt.body)]
        };

        // Find the while keyword position for comment handling
        // Search forward from body end, skipping over comments to find the actual keyword
        let body_end = stmt.body.span().end;
        let test_start = stmt.test.span().start;
        let while_pos = {
            let search = &self.source[body_end as usize..test_start as usize];
            let mut pos = 0;
            let mut found = None;
            while pos < search.len() {
                if search[pos..].starts_with("//") {
                    // Skip line comment
                    pos += search[pos..].find('\n').unwrap_or(search.len() - pos);
                } else if search[pos..].starts_with("/*") {
                    // Skip block comment
                    pos += search[pos + 2..]
                        .find("*/")
                        .map_or(search.len() - pos, |p| p + 4);
                } else if search[pos..].starts_with("while") {
                    found = Some(body_end + pos as u32);
                    break;
                } else {
                    pos += 1;
                }
            }
            found
        };

        // Check for comments between } and while, determine if while stays on same line
        let while_on_same_line = if let Some(while_start) = while_pos
            && self.has_comments_between(body_end, while_start)
        {
            let (inline_prev, own_line, inline_next) =
                self.partition_comments_by_line(body_end, while_start);

            // Merge inline_next (comments on same line as `while`) into own_line
            // so they're emitted before the `while` keyword rather than dropped.
            // e.g. `} \n /* c */ while (cond);` → `}\n/* c */\nwhile (cond);`
            let mut all_own_line = own_line;
            all_own_line.extend(inline_next);

            // Add comments preserving their position
            self.build_comments_between_parts(&mut parts, &inline_prev, &all_own_line, body_end);

            // While stays on same line only if: block body, no own-line comments, all inline are block comments
            let has_inline_line_comment = inline_prev.iter().any(|c| !c.is_block);
            is_block && all_own_line.is_empty() && !has_inline_line_comment
        } else {
            is_block
        };

        // Find paren positions for comment handling
        let open_paren = while_pos.and_then(|p| self.find_open_paren_after(p));
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `while` keyword and `(` in place:
        //   do{}while/* c */(a); → do {} while /* c */ (a);
        let keyword_comments = if let Some(wp) = while_pos {
            let while_keyword_end = wp + 5; // "while" is 5 chars
            self.build_keyword_paren_comments(while_keyword_end, open_paren)
        } else {
            None
        };

        if while_on_same_line {
            parts.push(d.text(" while"));
        } else {
            parts.push(d.hardline());
            parts.push(d.text("while"));
        }
        if let Some(kc) = keyword_comments {
            parts.push(kc);
        }
        parts.push(d.text(" ("));

        // Check for comments in the condition and use preserve_inline if present
        // Use preserve_inline for do-while to intentionally differ from Prettier
        // Prettier moves comments after `while (` to outside the parens - we keep them in place
        if let (Some(open), Some(close)) = (open_paren, close_paren)
            && (self.has_comments_between(open + 1, stmt.test.span().start)
                || self.has_comments_between(stmt.test.span().end, close))
        {
            parts.push(self.build_condition_group_preserve_inline(&stmt.test, open, close));
        } else {
            parts.push(self.build_expression_doc(&stmt.test));
        }

        // Preserve comments between the condition's `)` and the terminating `;` in
        // place: `} while (x) /* c */;` keeps the comment after `)` (Prettier
        // relocates it inside the parens — see close_paren_comment_prettier_divergence).
        // Mirrors the if-empty path's `append_close_paren_empty_stmt_with_comments`.
        if let Some(close) = close_paren {
            self.append_close_paren_empty_stmt_with_comments(&mut parts, close + 1, stmt.span.end);
        } else {
            parts.push(d.text(");"));
        }
        d.concat(&parts)
    }

    #[inline]
    pub(super) fn build_switch_statement_doc(&self, stmt: &internal::SwitchStatement) -> DocId {
        // Delegate to the wrapping version which handles proper indentation structure
        self.build_switch_statement_with_wrapping_doc(stmt)
    }

    pub(super) fn build_try_statement_doc(&self, stmt: &internal::TryStatement) -> DocId {
        let d = self.d();

        // try keyword to block: `try /* comment */ {`
        let try_keyword_end = stmt.span.start + 3; // "try" is 3 chars
        let block_start = stmt.block.span.start;
        let mut parts = vec![d.text("try")];
        self.append_keyword_to_body_comments(&mut parts, try_keyword_end, block_start);
        // Try block expands empty: `try {\n}` not `try {}`
        parts.push(self.build_block_statement_expand_empty_doc(&stmt.block));

        if let Some(handler) = &stmt.handler {
            // Check for comments between try block and catch keyword
            let try_end = stmt.block.span.end;
            // Use handler span start which is the position of "catch" keyword
            let catch_keyword_pos = handler.span.start;
            if self.has_comments_between(try_end, catch_keyword_pos) {
                let has_line_comment = self.has_line_comments_between(try_end, catch_keyword_pos);
                parts.push(self.build_inline_comments_between_doc(try_end, catch_keyword_pos));
                if has_line_comment {
                    parts.push(d.hardline());
                } else {
                    parts.push(d.text(" "));
                }
                parts.push(d.text("catch"));
            } else {
                parts.push(d.text(" catch"));
            }
            if let Some(param) = &handler.param {
                // Find paren positions for comment handling
                let catch_keyword_end = handler.span.start + 5; // "catch" is 5 chars
                let open_paren = self.find_open_paren_after(stmt.block.span.end);
                let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

                // Preserve comments between catch keyword and ( in place:
                //   catch/* comment */(e) → catch /* comment */ (e)
                let keyword_comments =
                    self.build_keyword_paren_comments(catch_keyword_end, open_paren);
                if let Some(kc) = keyword_comments {
                    parts.push(kc);
                }

                // Check for comments in catch parameter
                parts.push(d.text(" ("));
                if let (Some(open), Some(close)) = (open_paren, close_paren)
                    && (self.has_comments_between(open + 1, param.span().start)
                        || self.has_comments_between(param.span().end, close))
                {
                    parts.push(self.build_condition_group_with_comments(param, open, close));
                } else {
                    parts.push(self.build_expression_doc(param));
                }
                parts.push(d.text(")"));

                // Comments between ) and body: `catch (e) /* comment */ {`
                let paren_end = close_paren.unwrap_or_else(|| param.span().end) + 1;
                self.append_keyword_to_body_comments(
                    &mut parts,
                    paren_end,
                    handler.body.span.start,
                );
            } else {
                // No param: comments between catch keyword and body: `catch /* comment */ {`
                let catch_keyword_end = handler.span.start + 5; // "catch" is 5 chars
                self.append_keyword_to_body_comments(
                    &mut parts,
                    catch_keyword_end,
                    handler.body.span.start,
                );
            }
            // Catch block stays inline: `catch (e) {}`
            parts.push(self.build_block_statement_doc(&handler.body));
        }
        if let Some(finalizer) = &stmt.finalizer {
            // Check for comments before finally (after catch block or try block)
            let prev_end = stmt
                .handler
                .as_ref()
                .map_or(stmt.block.span.end, |h| h.body.span.end);
            // The finalizer span starts at the "finally" keyword
            // Note: finalizer is a BlockStatement, we need to find the keyword position
            // Search for "finally" in source (but avoid matching inside comments)
            // For safety, search backwards from finalizer start for "finally"
            let search_range = &self.source[prev_end as usize..finalizer.span.start as usize];
            let finally_keyword_pos = search_range
                .rfind("finally")
                .map_or(finalizer.span.start, |p| prev_end + p as u32);
            if self.has_comments_between(prev_end, finally_keyword_pos) {
                let has_line_comment =
                    self.has_line_comments_between(prev_end, finally_keyword_pos);
                parts.push(self.build_inline_comments_between_doc(prev_end, finally_keyword_pos));
                if has_line_comment {
                    parts.push(d.hardline());
                } else {
                    parts.push(d.text(" "));
                }
                parts.push(d.text("finally"));
            } else {
                parts.push(d.text(" finally"));
            }
            // Comments between finally keyword and body: `finally /* comment */ {`
            let finally_keyword_end = finally_keyword_pos + 7; // "finally" is 7 chars
            self.append_keyword_to_body_comments(
                &mut parts,
                finally_keyword_end,
                finalizer.span.start,
            );
            // Finally block expands empty: `finally {\n}` not `finally {}`
            parts.push(self.build_block_statement_expand_empty_doc(finalizer));
        }
        d.concat(&parts)
    }

    pub(super) fn build_throw_statement_doc(&self, stmt: &internal::ThrowStatement) -> DocId {
        self.build_keyword_argument_doc("throw", stmt.span.start, stmt.span.end, &stmt.argument)
    }

    pub(super) fn build_break_statement_doc(&self, stmt: &internal::BreakStatement) -> DocId {
        self.build_jump_statement_doc("break", stmt.span, stmt.label.as_ref())
    }

    pub(super) fn build_continue_statement_doc(&self, stmt: &internal::ContinueStatement) -> DocId {
        self.build_jump_statement_doc("continue", stmt.span, stmt.label.as_ref())
    }

    /// Shared builder for break/continue statements with optional label and trailing comments.
    fn build_jump_statement_doc(
        &self,
        keyword: &'static str,
        span: tsv_lang::Span,
        label: Option<&internal::Identifier>,
    ) -> DocId {
        let d = self.d();
        if let Some(label) = label {
            let keyword_end = span.start + keyword.len() as u32;
            // Comments between keyword and label (e.g., `break /* c */ loop;`)
            let pre_label_comment =
                self.build_inline_comments_between_doc_opt(keyword_end, label.span.start);
            // Comments between label and semicolon (e.g., `break loop /* c */;`)
            let post_label_comment =
                self.build_inline_comments_between_doc_opt(label.span.end, span.end);

            let mut parts = Vec::new();
            parts.push(d.text(keyword));
            if let Some(comment_doc) = pre_label_comment {
                parts.push(comment_doc);
            }
            parts.push(d.text(" "));
            parts.push(d.symbol(label.name.to_u32()));
            if let Some(comment_doc) = post_label_comment {
                parts.push(comment_doc);
            }
            parts.push(d.text(";"));
            d.concat(&parts)
        } else {
            let keyword_end = span.start + keyword.len() as u32;
            if let Some(comment_doc) =
                self.build_inline_comments_between_doc_opt(keyword_end, span.end)
            {
                d.concat(&[d.text(keyword), d.text(";"), comment_doc])
            } else {
                d.concat(&[d.text(keyword), d.text(";")])
            }
        }
    }

    pub(super) fn build_labeled_statement_doc(&self, stmt: &internal::LabeledStatement) -> DocId {
        let d = self.d();
        let label_end = stmt.label.span.end;
        let body_start = stmt.body.span().start;

        // Find actual colon position (skip comments between label and colon)
        let colon_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            label_end as usize,
            body_start as usize,
            b':',
        )
        .unwrap_or(label_end as usize);
        let colon_end = colon_pos as u32 + 1;

        let mut parts = vec![d.symbol(stmt.label.name.to_u32())];

        // Comments between label name and colon: `label /* c */:`
        parts.push(self.build_inline_comments_between_doc(label_end, colon_pos as u32));

        // Check for comments between colon and body
        if self.has_comments_between(colon_end, body_start) {
            let has_line_comment = self.has_line_comments_between(colon_end, body_start);
            parts.push(d.text(":"));
            parts.push(self.build_inline_comments_between_doc(colon_end, body_start));
            if has_line_comment {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(self.build_statement_doc(&stmt.body));
            d.concat(&parts)
        } else {
            // No space before empty statement: `label:;` not `label: ;`
            let separator = if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
                ":"
            } else {
                ": "
            };
            parts.push(d.text(separator));
            parts.push(self.build_statement_doc(&stmt.body));
            d.concat(&parts)
        }
    }

    /// Get the end position of a ForInit
    fn get_for_init_span_end(&self, init: &internal::ForInit) -> u32 {
        match init {
            internal::ForInit::VariableDeclaration(decl) => decl.span.end,
            internal::ForInit::Expression(expr) => expr.span().end,
        }
    }
}
