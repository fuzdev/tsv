// List- and body-level comment emitters.
//
// These handle comments across a member/element list or a body: leading/trailing
// comments with blank-line preservation, the open-delimiter trailing-comment
// divergence (delimiter-line prefix), empty-container comments, signature/body
// comment splitting, inline-block comment runs, and comma emission in forced-
// multiline lists.

use super::Printer;
use crate::ast::internal;
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Emit the comments in `[start, end)` between a class/interface header
    /// (after the last heritage item or type params) and the body `{`, preserving
    /// each comment on its own line. A line comment ends its line, so any comment
    /// following one is pushed to its own line via `hardline` — otherwise it would
    /// be absorbed into the line comment's text (`// c1 // c2` reparses as a single
    /// comment, a content/boundary loss). The first comment, and a block following a
    /// block, keep a leading space, matching the single-comment heritage form
    /// `J // c`.
    ///
    /// Returns `None` when the range has no comments. The caller appends the pre-`{`
    /// separator itself (`hardline` for a line comment, space/`line` otherwise).
    pub(crate) fn build_pre_body_comments_doc(&self, start: u32, end: u32) -> Option<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_is_line = false;
        for comment in comments_in_range(self.comments, start, end) {
            if prev_is_line {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(self.build_comment_doc(comment));
            prev_is_line = !comment.is_block;
        }
        if parts.is_empty() {
            None
        } else {
            Some(d.concat(&parts))
        }
    }

    /// Build docs for leading comments in a forced-multiline context.
    ///
    /// Comments between `start` and `end` (where `end` is the element start):
    /// - Block comments on the same line as the element: `/*content*/ ` (inline with trailing space)
    /// - Block comments on their own line: `/*content*/` + hardline
    /// - Line comments: `//content` + hardline (always on own line)
    ///
    /// Used when expanding comments force multiline formatting (unions, tuples, etc.)
    pub(crate) fn build_leading_comments_multiline(&self, start: u32, end: u32) -> Vec<DocId> {
        self.build_leading_comments_multiline_opt(start, end, None)
    }

    /// Like `build_leading_comments_multiline`, but when `skip_delim` is `Some(pos)`,
    /// comments sharing `pos`'s source line are skipped — they were already emitted
    /// as a trailing prefix on the opening delimiter's line (see
    /// `delimiter_line_comment_prefix`), so emitting them here too would duplicate
    /// them. Pass the `Option<u32>` that `delimiter_line_comment_prefix` returns
    /// (gated to the first element of the list; `None` for the rest).
    pub(in crate::printer) fn build_leading_comments_multiline_opt(
        &self,
        start: u32,
        end: u32,
        skip_delim: Option<u32>,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            if skip_delim.is_some_and(|pos| self.comment_on_delimiter_line(pos, comment)) {
                continue; // pulled onto the delimiter line
            }
            parts.push(self.build_comment_doc(comment));
            if comment.is_block && self.is_same_line(comment.span.end, end) {
                parts.push(d.text(" "));
            } else {
                parts.push(d.hardline());
            }
        }
        parts
    }

    /// Build docs for trailing comments in a forced-multiline context.
    ///
    /// Same-line comments (block or line): ` /*content*/` or ` //content` (inline with leading space)
    /// Own-line comments: hardline + comment (on their own line)
    ///
    /// Used when line comments force multiline formatting (unions, tuples, etc.)
    pub(crate) fn build_trailing_comments_multiline(&self, start: u32, end: u32) -> Vec<DocId> {
        self.build_trailing_comments_multiline_ext(start, end, false)
    }

    /// As `build_trailing_comments_multiline`, but when `suffix_same_line_lines` is set
    /// a same-line **line** comment is routed through `line_suffix` (zero width) so it
    /// can't force the preceding element to break. Only safe where the following
    /// separator lands on a *new* line (so the suffix flushes at that hardline without
    /// crossing the separator) — true for the union's leading-`|` form, but NOT the
    /// intersection's trailing-`&` form (a same-line `//` there would otherwise comment
    /// out the `&`; that case is handled as a comment-position divergence instead).
    pub(crate) fn build_trailing_comments_multiline_ext(
        &self,
        start: u32,
        end: u32,
        suffix_same_line_lines: bool,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            if self.is_same_line(start, comment.span.start) {
                if suffix_same_line_lines {
                    // Block → inline (width counted); line → line_suffix (zero width).
                    parts.push(self.build_trailing_comment_doc(comment));
                } else {
                    // Same line as start: trailing comment (block or line), inline.
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            } else {
                // Own line comment (block or line)
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }
        parts
    }
    /// Filter block comments between two positions based on whether they're on the same line as start
    ///
    /// # Arguments
    /// * `start` - Start position (e.g., end of previous chain element)
    /// * `end` - End position (e.g., start of next chain element)
    /// * `same_line` - If true, returns comments on same line as start; if false, returns comments on their own lines
    pub(crate) fn filter_block_comments(
        &self,
        start: u32,
        end: u32,
        same_line: bool,
    ) -> Vec<&internal::Comment> {
        comments_in_range(self.comments, start, end)
            .filter(|c| c.is_block)
            .filter(|c| same_line == self.is_same_line(start, c.span.start))
            .collect()
    }

    /// Check if there's a newline between start position and the first comment in the range
    ///
    /// Returns true if there's at least one comment in the range and a newline
    /// exists between `start` and the first comment's start position.
    /// Check if ALL comments in the range are inline block comments on the same line as `end`.
    ///
    /// Returns true when every comment is a block comment AND on the same line as `end`
    /// (the next expression). Used to keep `/** @type {T} */ arg` as a unit.
    /// Returns false for line comments or block comments on their own line.
    pub(crate) fn all_comments_are_inline_block(&self, start: u32, end: u32) -> bool {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);
        let mut found_any = false;
        for comment in self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= end)
        {
            found_any = true;
            if !comment.is_block || !self.is_same_line(comment.span.end, end) {
                return false;
            }
        }
        found_any
    }

    /// True when a block comment in `(search_start, end)` sits on its own line —
    /// i.e. not on the same source line as `line_ref`.
    ///
    /// Used to force a parameter/element list to multiline when an own-line block
    /// comment follows the last element (`line_ref` = `search_start` = last elem end)
    /// or fills the opening-delimiter→first-element gap (`line_ref` = the delimiter,
    /// `search_start` = just past it). Line comments in the same position are detected
    /// separately (they always force a break).
    pub(crate) fn has_own_line_block_comment_after(
        &self,
        line_ref: u32,
        search_start: u32,
        end: u32,
    ) -> bool {
        comments_in_range(self.comments, search_start, end)
            .any(|c| c.is_block && !self.is_same_line(line_ref, c.span.start))
    }

    /// Check if there's a block comment on its own line within a container.
    ///
    /// A "standalone" block comment is one that:
    /// - Is not on the same line as the opening brace
    /// - Is not on the same line as any item (start or end)
    ///
    /// Used to force multiline formatting for objects/type literals.
    pub(crate) fn has_standalone_block_comment(
        &self,
        container_start: u32,
        container_end: u32,
        item_spans: &[tsv_lang::Span],
    ) -> bool {
        let after_open_brace = container_start + 1;
        comments_in_range(self.comments, container_start, container_end).any(|c| {
            if !c.is_block {
                return false; // Line comments handled separately
            }
            // Must not be on same line as opening brace
            if self.is_same_line(after_open_brace, c.span.start) {
                return false;
            }
            // Must not be on same line as any item
            !item_spans.iter().any(|s| {
                self.is_same_line(s.start, c.span.start) || self.is_same_line(s.end, c.span.start)
            })
        })
    }

    /// Build docs for trailing same-line comments after a node
    ///
    /// Line comments are wrapped in `line_suffix` so they don't affect width
    /// calculations for preceding groups (matches Prettier behavior).
    /// Block comments are inline and do affect width.
    ///
    /// Returns a Vec of docs to append to the current parts.
    pub(crate) fn build_trailing_same_line_comment_docs(
        &self,
        after_pos: u32,
        upper_bound: u32,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut docs = Vec::new();
        // Track line reference — follows multi-line block comments to their
        // closing */ line (same logic as build_trailing_same_line_comments_doc in mod.rs)
        let mut line_ref = after_pos;
        for comment in comments_in_range(self.comments, after_pos, upper_bound) {
            if self.is_same_line(line_ref, comment.span.start) {
                if comment.is_block {
                    // Block comments are inline, affect width
                    docs.push(d.text(" "));
                    docs.push(self.build_comment_doc(comment));
                    // Follow multi-line block comments to their closing line
                    if !self.is_same_line(comment.span.start, comment.span.end) {
                        line_ref = comment.span.end;
                    }
                } else {
                    // Line comments go in line_suffix, don't affect width
                    docs.push(self.build_trailing_line_comment_doc(comment));
                }
            } else {
                break; // Only same-line comments
            }
        }
        docs
    }

    /// Build docs for leading comments before a node with blank line preservation.
    ///
    /// Handles comments that appear before a member/statement, preserving blank lines
    /// between consecutive comments and after the last comment. Returns a Vec of docs
    /// to append directly before the target node.
    ///
    /// Used by: class body members, block statement bodies, interface members, type literals.
    pub(crate) fn build_leading_comments_with_blank_lines(
        &self,
        comments: &[&internal::Comment],
        target_start: u32,
    ) -> Vec<DocId> {
        if comments.is_empty() {
            return Vec::new();
        }

        let d = self.d();

        // Check if there's a blank line after the last comment
        let has_blank_after_last_comment = comments
            .last()
            .is_some_and(|c| self.has_blank_line_between(c.span.end, target_start));

        let mut docs = Vec::new();
        let mut last_pos = comments[0].span.start;

        for (j, comment) in comments.iter().enumerate() {
            let is_last_comment = j == comments.len() - 1;

            // Check if there's a blank line after this comment
            // (to next comment or to target if last comment)
            let has_blank_after = if is_last_comment {
                has_blank_after_last_comment
            } else {
                self.has_blank_line_between(comment.span.end, comments[j + 1].span.start)
            };

            // Check if the next item (comment or target) is on the same line as this comment's end.
            // This handles multi-line block comments where the closing */ is followed by another
            // comment on the same line: `/*\nmulti\n*/ /* after */`
            let next_on_same_line = if is_last_comment {
                self.is_same_line(comment.span.end, target_start)
            } else {
                self.is_same_line(comment.span.end, comments[j + 1].span.start)
            };

            // For subsequent comments, determine separator from previous comment
            if j > 0 {
                if self.is_same_line(last_pos, comment.span.start) {
                    // Same line as previous comment's end — keep inline (space is
                    // handled by the previous comment's suffix, so no space here)
                } else if self.has_blank_line_between(last_pos, comment.span.start) {
                    docs.push(d.literalline());
                    docs.push(d.hardline());
                }
                // else: no separator needed (previous comment's suffix handled it)
            }

            docs.push(self.build_comment_doc(comment));

            if !comment.is_block {
                // Line comment: add hardline after unless there's a blank line after
                // (the blank line separator will handle it)
                if !has_blank_after {
                    docs.push(d.hardline());
                }
            } else if next_on_same_line {
                // Block comment on same line as next item - space before next
                docs.push(d.text(" "));
            } else if !has_blank_after {
                // Block comment on its own line: add hardline unless there's blank after
                docs.push(d.hardline());
            }
            last_pos = comment.span.end;
        }

        // Add blank line after last comment if present
        if has_blank_after_last_comment {
            docs.push(d.literalline());
            docs.push(d.hardline());
        }

        docs
    }

    /// Build docs for trailing comments at the end of a body (before closing `}`).
    ///
    /// Handles comments that appear after the last member/statement in a body,
    /// with blank line preservation between them. Returns a Vec of docs to append.
    ///
    /// Used by: class body, interface body, enum body, type literal, namespace body.
    pub(crate) fn build_trailing_body_comments_doc(
        &self,
        prev_end: u32,
        body_end: u32,
    ) -> Vec<DocId> {
        let trailing_comments: Vec<_> = comments_in_range(self.comments, prev_end, body_end)
            .filter(|c| !self.is_same_line(prev_end, c.span.start))
            .collect();

        if trailing_comments.is_empty() {
            return Vec::new();
        }

        let d = self.d();
        let mut docs = Vec::new();

        // Check for blank line before the first trailing comment
        let first_comment = trailing_comments[0];
        if self.has_blank_line_between(prev_end, first_comment.span.start) {
            docs.push(d.literalline());
        }
        docs.push(d.hardline());

        // Process each trailing comment
        let mut last_pos = prev_end;
        for (j, comment) in trailing_comments.iter().enumerate() {
            let is_last = j == trailing_comments.len() - 1;

            // Check for blank lines between comments
            if j > 0 && self.has_blank_line_between(last_pos, comment.span.start) {
                docs.push(d.literalline());
                docs.push(d.hardline());
            }

            // Check if there's a blank line after this comment (to next comment)
            let has_blank_after = !is_last
                && self
                    .has_blank_line_between(comment.span.end, trailing_comments[j + 1].span.start);

            docs.push(self.build_comment_doc(comment));

            // Line comment - add hardline after unless:
            // - It's the last comment (closing brace follows)
            // - There's a blank line after (the blank line separator handles it)
            if !comment.is_block && !is_last && !has_blank_after {
                docs.push(d.hardline());
            }
            // Block comments don't need hardline after in this context
            // (the closing brace follows immediately)

            last_pos = comment.span.end;
        }

        docs
    }

    /// Compute the "delimiter-line prefix" for the open-delimiter trailing-comment
    /// divergence (object literals, array literals, and block bodies).
    ///
    /// A comment on the same source line as the opening delimiter at `delim_pos`
    /// is kept on that line — instead of being relocated to its own line as the
    /// first element's leading comment (prettier's behavior). Returns the emitted
    /// prefix docs (` /* c */` / ` // c`, leading-space convention) and, when the
    /// pull fired, `Some(delim_pos)` — the position the caller passes back to
    /// exclude those same-line comments from the first element's leading set
    /// (`None` when nothing was pulled, so the prefix is empty).
    ///
    /// Gated on `should_force_expansion_for_comments`, so an inline block comment
    /// hugging the first element (`{ /* c */ a: 1 }`, `[/* c */ x]`) is left in
    /// place and the result is `(empty, None)`. See conformance_prettier.md
    /// §Comment relocation.
    pub(in crate::printer) fn delimiter_line_comment_prefix(
        &self,
        delim_pos: u32,
        first_elem_start: u32,
    ) -> (Vec<DocId>, Option<u32>) {
        let pc = super::calls::PartitionedComments::new(
            self.comments,
            self.line_breaks,
            delim_pos,
            first_elem_start,
        );
        let pull = (!pc.trailing_block.is_empty() || !pc.trailing_line.is_empty())
            && super::calls::should_force_expansion_for_comments(self, delim_pos, first_elem_start);
        let mut prefix = Vec::new();
        if pull {
            pc.emit_trailing_comments(&mut prefix, self);
        }
        (prefix, pull.then_some(delim_pos))
    }

    /// Whether `comment` was pulled onto the opening delimiter's line by
    /// `delimiter_line_comment_prefix` — i.e. it shares a source line with the
    /// delimiter at `delim_pos`.
    ///
    /// The prefix helper emits these comments on the delimiter's line; every
    /// consumer must then drop the same comments from the first element's
    /// leading-comment set so they aren't emitted twice. Centralizing the test
    /// keeps that exclusion in lockstep with what the prefix actually pulls.
    pub(in crate::printer) fn comment_on_delimiter_line(
        &self,
        delim_pos: u32,
        comment: &internal::Comment,
    ) -> bool {
        self.is_same_line(delim_pos, comment.span.start)
    }

    /// A first element/member's leading comments with the delimiter-line
    /// comments removed.
    ///
    /// `delimiter_line_comment_prefix` emits the comments sharing the opening
    /// delimiter's line as a prefix on that line, so every member-loop consumer
    /// must drop the same comments from the first element's leading set to avoid
    /// emitting them twice (see `comment_on_delimiter_line`). Returns `comments`
    /// unchanged when `delimiter_pull_pos` is `None` (nothing was pulled).
    pub(in crate::printer) fn first_member_leading_comments<'c>(
        &self,
        comments: Vec<&'c internal::Comment>,
        delimiter_pull_pos: Option<u32>,
    ) -> Vec<&'c internal::Comment> {
        match delimiter_pull_pos {
            Some(dpos) => comments
                .into_iter()
                .filter(|c| !self.comment_on_delimiter_line(dpos, c))
                .collect(),
            None => comments,
        }
    }

    /// Build a line_suffix doc for all comments between two positions
    ///
    /// Used for trailing comments on call arguments, where comments should stay
    /// on the same line but not affect width calculations for breaking decisions.
    /// Returns None if no comments exist in the range.
    ///
    /// Example: `fn(arg // comment)` - the comment becomes a line_suffix
    pub(crate) fn build_trailing_comments_line_suffix(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        let d = self.d();
        // Single binary search to find first comment
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);
        let first = self.comments.get(first_idx).filter(|c| c.span.end <= end)?;

        // Build parts starting from found comment
        let mut parts = Vec::new();
        for comment in std::iter::once(first).chain(
            self.comments[first_idx + 1..]
                .iter()
                .take_while(|c| c.span.end <= end),
        ) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        Some(d.line_suffix(d.concat(&parts)))
    }

    /// Build a Doc for an empty body (`{}`) that may contain comments.
    ///
    /// If comments exist between the braces, formats as:
    /// ```text
    /// {
    ///     // comment
    /// }
    /// ```
    ///
    /// If no comments, returns `{}`.
    ///
    /// Used by: interface body, class body, enum body, namespace body, object literal, object pattern.
    pub(crate) fn build_empty_body_with_comments_doc(&self, body_span: tsv_lang::Span) -> DocId {
        self.build_empty_delimited_with_comments_doc(body_span.start, body_span.end, "{", "}")
    }

    /// Build a Doc for an empty bracket body (`[]`) that may contain comments.
    ///
    /// If comments exist between the brackets, formats as:
    /// ```text
    /// [
    ///     // comment
    /// ]
    /// ```
    ///
    /// If no comments, returns `[]`.
    ///
    /// Used by: array literal, tuple type.
    pub(crate) fn build_empty_brackets_with_comments_doc(&self, span: tsv_lang::Span) -> DocId {
        self.build_empty_delimited_with_comments_doc(span.start, span.end, "[", "]")
    }

    /// Build a Doc for an empty bracket body with explicit bounds.
    ///
    /// Used when the bracket body ends before the full span (e.g., array pattern with type annotation).
    pub(crate) fn build_empty_brackets_with_comments_doc_range(
        &self,
        body_start: u32,
        body_end: u32,
    ) -> DocId {
        self.build_empty_delimited_with_comments_doc(body_start, body_end, "[", "]")
    }

    /// Build a Doc for an empty delimited container that may contain comments.
    ///
    /// Generic helper for both `{}` and `[]` containers.
    fn build_empty_delimited_with_comments_doc(
        &self,
        span_start: u32,
        span_end: u32,
        open: &'static str,
        close: &'static str,
    ) -> DocId {
        let d = self.d();
        let body_start = span_start + 1; // After opening delimiter
        let body_end = span_end.saturating_sub(1); // Before closing delimiter

        // Single binary search to find comments
        let first_idx = tsv_lang::find_first_comment_from(self.comments, body_start);
        let comments: Vec<_> = self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= body_end)
            .collect();

        if comments.is_empty() {
            return d.text_owned(format!("{open}{close}"));
        }
        let mut comment_parts = Vec::new();

        for (i, comment) in comments.iter().enumerate() {
            comment_parts.push(self.build_comment_doc(comment));
            // Add hardline after line comments, except for the last one
            // (the hardline before closing delimiter handles that)
            if !comment.is_block && i < comments.len() - 1 {
                comment_parts.push(d.hardline());
            }
        }

        d.concat(&[
            d.text(open),
            d.indent(d.concat(&[d.hardline(), d.concat(&comment_parts)])),
            d.hardline(),
            d.text(close),
        ])
    }

    /// Append a function/method body with comment splitting between signature and body.
    ///
    /// Block comments stay inline: `gen() /* c */ {}`
    /// Line comments get absorbed into the block body as leading content.
    pub(crate) fn append_body_with_sig_comments(
        &self,
        parts: &mut Vec<DocId>,
        sig_end: u32,
        body: &internal::BlockStatement,
    ) {
        let d = self.d();
        let body_start = body.span.start;
        if self.has_comments_between(sig_end, body_start) {
            let mut absorbed = Vec::new();
            for comment in comments_in_range(self.comments, sig_end, body_start) {
                if comment.is_block {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                } else {
                    absorbed.push(self.build_comment_doc(comment));
                }
            }
            parts.push(d.text(" "));
            parts.push(self.build_block_statement_with_outer_comments_doc(body, absorbed));
        } else {
            parts.push(d.text(" "));
            parts.push(self.build_block_statement_doc(body));
        }
    }

    /// Append the comments between a signature's last content token and the
    /// member's end (typically right before the printed `;`): after the return
    /// type, or after the params' closing `)` when there is no return type.
    ///
    /// Shared by method/call/construct signatures in interfaces and type
    /// literals, abstract/overload class methods, and declare functions.
    pub(crate) fn append_signature_end_comments(
        &self,
        parts: &mut Vec<DocId>,
        return_type: Option<&internal::TSTypeAnnotation>,
        paren_pos: Option<u32>,
        span_end: u32,
    ) {
        let content_end = return_type.map_or_else(
            || {
                paren_pos
                    .and_then(|p| self.find_closing_paren(p, span_end))
                    .unwrap_or(span_end)
            },
            |rt| rt.span.end,
        );
        // Break-safe: a line comment in the signature→`;` gap floats after `;` via
        // `line_suffix` instead of swallowing it (`m(): void; // c`).
        self.append_trailing_member_comments(parts, content_end, span_end);
    }

    /// Push every comment in `[start, end)` to `parts` as a **trailing** comment —
    /// the break-safe idiom for a type member's gap before its `;` terminator. Each
    /// goes through `build_trailing_comment_doc`, so a block trails inline
    /// (` /* c */`) and a line comment floats after the terminator via `line_suffix`
    /// instead of swallowing it. Shared by the property arm
    /// (`build_property_signature_member_doc`) and the signature arms
    /// (`append_signature_end_comments`) so every `TSTypeElement` trailing emission
    /// uses the one break-safe path.
    pub(crate) fn append_trailing_member_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
    ) {
        for comment in comments_in_range(self.comments, start, end) {
            parts.push(self.build_trailing_comment_doc(comment));
        }
    }

    /// Append leading inline block comments (`/*content*/ ` format) between two positions.
    ///
    /// Only emits block comments; line comments are skipped (they would have been
    /// detected earlier and routed to the multiline path). Counterpart of
    /// [`Self::append_trailing_inline_block_comments`].
    pub(crate) fn append_leading_inline_block_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text_owned(format!("/*{}*/ ", comment.content)));
            }
        }
    }

    /// Append trailing inline block comments (` /*content*/` format) between two positions.
    ///
    /// Only emits block comments; line comments are skipped (they would have been
    /// detected earlier and routed to the multiline path).
    pub(crate) fn append_trailing_inline_block_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text_owned(format!(" /*{}*/", comment.content)));
            }
        }
    }

    /// Split the last list-member's trailing inline block comments around a source
    /// comma (in `elem_end..end_boundary`): comments before the comma go to `before`,
    /// comments after it to `after`. Callers emit `after` past where the comma was
    /// (no trailing comma; trailingComma: 'none') so the comment is preserved after it
    /// rather than relocated before (see conformance_prettier.md §Comment relocation).
    pub(crate) fn append_last_trailing_block_comments_split(
        &self,
        before: &mut Vec<DocId>,
        after: &mut Vec<DocId>,
        elem_end: u32,
        end_boundary: u32,
    ) {
        match self
            .find_comma_after(elem_end)
            .filter(|cp| *cp < end_boundary)
        {
            Some(comma_pos) => {
                self.append_trailing_inline_block_comments(before, elem_end, comma_pos);
                self.append_trailing_inline_block_comments(after, comma_pos, end_boundary);
            }
            None => self.append_trailing_inline_block_comments(before, elem_end, end_boundary),
        }
    }

    /// Emit comma with surrounding comments for a non-last element in a forced-multiline list.
    ///
    /// Handles comment positioning around the comma between `elem_end` and `next_start`:
    /// 1. Trailing comments before comma (multiline layout)
    /// 2. Comma text
    /// 3. Same-line trailing comments after comma (line comments)
    /// 4. Hardline separator
    ///
    /// Returns the new `prev_end` position.
    pub(crate) fn emit_multiline_comma_with_comments(
        &self,
        parts: &mut Vec<DocId>,
        elem_end: u32,
        next_start: u32,
    ) -> u32 {
        let d = self.d();
        let comma_pos = self.find_list_comma(elem_end, next_start);

        // Trailing comments before comma
        parts.extend(self.build_trailing_comments_multiline(elem_end, comma_pos));

        // Comma
        parts.push(d.text(","));

        // Same-line trailing comments after comma (line comments that consume the line).
        // A line comment goes through `line_suffix` (zero width) so it never forces the
        // preceding element to break; it flushes at the hardline below (prettier's
        // `lineSuffix`). A block stays inline, width counted.
        let mut after_comma_end = comma_pos + 1;
        for comment in comments_in_range(self.comments, comma_pos + 1, next_start) {
            if self.is_same_line(elem_end, comment.span.start) {
                parts.push(self.build_trailing_comment_doc(comment));
                after_comma_end = comment.span.end;
            }
        }

        // Hardline to separate from next element
        parts.push(d.hardline());

        after_comma_end
    }
}
