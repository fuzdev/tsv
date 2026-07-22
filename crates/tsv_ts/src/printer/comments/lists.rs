// List- and body-level comment emitters.
//
// These handle comments across a member/element list or a body: leading/trailing
// comments with blank-line preservation, the open-delimiter trailing-comment
// divergence (delimiter-line prefix), empty-container comments, signature/body
// comment splitting, inline-block comment runs, and comma emission in forced-
// multiline lists.

use super::{CommentVec, LeadingGlue, Printer};
use crate::ast::internal;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Which blank-line rule a comma-separated list's separator follows.
///
/// Prettier asks two different questions here, and a list belongs to exactly one of them —
/// so the caller names its kind rather than passing a bare "preserve?" bool that cannot say
/// which rule it meant. The split is not cosmetic: the two disagree precisely when the comma
/// and the blank line sit on different lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BlankRule {
    /// The list has no blank-preservation rule; the separator is a plain hardline.
    None,
    /// Prettier's `isNextLineEmpty` — measured from the **element's end**, so the blank must
    /// begin on the line that element ends on. Params, call arguments, object properties:
    /// prettier emits a `hardline` there, so a blank also forces the list to break.
    NextLineEmpty,
    /// Prettier's `isLineAfterElementEmpty` — advance to the **comma** first, then measure.
    /// Arrays and tuples: prettier emits a `softline`, so a blank never forces a break, and a
    /// blank before the comma is not preserved.
    AfterComma,
}

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
        let mut parts = DocBuf::new();
        let mut prev_is_line = false;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
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

    /// Build the leading-comment run over `[start, end)` for a list whose comments have
    /// forced it multiline (tuples, type params/args, function-type params, unions, the
    /// bracket-break shell, the broken cast).
    ///
    /// A thin adapter over the shared leading-comment emitter
    /// ([`Printer::push_leading_comment_run`]), so the separator after each comment
    /// follows prettier's `printLeadingComment` (space / soft `line` / hardline, keyed on
    /// the source around *that* comment, never on where `end` is).
    ///
    /// `skip_delim` drops the comments sharing `pos`'s source line: they were already
    /// emitted as a trailing prefix on the opening delimiter's line (see
    /// [`Self::delimiter_line_comment_prefix`]), so emitting them here too would
    /// **duplicate** them. Pass the `Option<u32>` that helper returns — gated to the list's
    /// first element, `None` for the rest, and `None` where no delimiter is involved.
    pub(in crate::printer) fn build_leading_comments_multiline(
        &self,
        start: u32,
        end: u32,
        skip_delim: Option<u32>,
    ) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::new();
        self.push_leading_comment_run(
            &mut parts,
            comments_to_emit_in_range(self.comments, start, end)
                .filter(|c| !skip_delim.is_some_and(|pos| self.comment_on_delimiter_line(pos, c))),
            end,
            LeadingGlue::Adjacent,
            d.empty(),
        );
        parts
    }

    /// Assemble one **element** of an array-like list: its leading-comment run plus the
    /// element, wrapped in a single `group`.
    ///
    /// The group is what lets a leading comment's soft `line` (prettier's
    /// `printLeadingComment`) be measured against *this element alone* and collapse
    /// (`/* c1 */ /* c2 */ a`) even though the list itself is broken. Prettier's
    /// `printArrayElements` (`src/language-js/print/array.js`) does the same — it pushes
    /// `group(print())` per element and `print()` carries the element's leading comments —
    /// and array literals, array patterns, and tuple types all route through it. A line
    /// comment (or an author blank line) in the run puts a `hardline` inside the group, so
    /// it breaks and the element drops below, also matching prettier.
    ///
    /// This grouping is what separates the array family from the params family: function /
    /// type-parameter / type-argument / call-argument lists use a bare `join([",", line])`
    /// with **no** per-element group, so the identical soft `line` rides the broken outer
    /// group and breaks. That one fact predicts the layout at every one of those sites —
    /// don't re-derive it. See conformance_prettier.md §Comment relocation.
    ///
    /// A list with holes passes only its real elements here; a hole carries no comments and
    /// takes no group.
    pub(crate) fn build_list_element_group(&self, mut leading: DocBuf, element: DocId) -> DocId {
        let d = self.d();
        leading.push(element);
        d.group(d.concat(&leading))
    }

    /// [`Self::build_list_element_group`] for a caller holding the element's leading
    /// comments as a list rather than a range — the array literal and array pattern, whose
    /// per-element filter (which same-line block comments trail the *previous* element
    /// across its comma) is too specific to express as a range.
    ///
    /// Builds the run here so the separator policy every array-family element shares
    /// (`LeadingGlue::Adjacent`, no continuation indent) is stated once rather than at each
    /// call. The range-holding sibling is
    /// [`Self::build_leading_comments_multiline`].
    pub(crate) fn build_list_element_group_from_comments<'c>(
        &self,
        comments: impl Iterator<Item = &'c internal::Comment>,
        element_start: u32,
        element: DocId,
    ) -> DocId {
        let mut leading = DocBuf::new();
        self.push_leading_comment_run(
            &mut leading,
            comments,
            element_start,
            LeadingGlue::Adjacent,
            self.d().empty(),
        );
        self.build_list_element_group(leading, element)
    }

    /// Build docs for trailing comments in a forced-multiline context.
    ///
    /// Same-line comments (block or line): ` /*content*/` or ` //content` (inline with leading space)
    /// Own-line comments: hardline + comment (on their own line)
    ///
    /// Used when line comments force multiline formatting (unions, tuples, etc.)
    pub(crate) fn build_trailing_comments_multiline(&self, start: u32, end: u32) -> DocBuf {
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
    ) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut prev_end = start;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
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
                // Own line comment (block or line), preserving an author blank line
                // before it (`elem⏎⏎/* c */` before the closing delimiter) — prettier
                // keeps one blank in every list position (tuple, function/-type params,
                // signatures, type args/params).
                self.push_blank_preserving_hardline(&mut parts, prev_end, comment.span.start);
                parts.push(self.build_comment_doc(comment));
            }
            prev_end = comment.span.end;
        }
        parts
    }
    /// Filter block comments between two positions based on whether they're on the same line as start
    ///
    /// # Arguments
    /// * `start` - Start position (e.g., end of previous chain element)
    /// * `end` - End position (e.g., start of next chain element)
    ///
    /// Returns the block comments on the same source line as `start`.
    pub(crate) fn filter_block_comments(&self, start: u32, end: u32) -> CommentVec<'_> {
        comments_to_emit_in_range(self.comments, start, end)
            .filter(|c| c.is_block)
            .filter(|c| self.is_same_line(start, c.span.start))
            .collect()
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
        self.comments_on_page_between(search_start, end)
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
        item_spans: &[Span],
    ) -> bool {
        let after_open_brace = container_start + 1;
        self.comments_on_page_between(container_start, container_end)
            .any(|c| {
                if !c.is_block {
                    return false; // Line comments handled separately
                }
                // Must not be on same line as opening brace
                if self.is_same_line(after_open_brace, c.span.start) {
                    return false;
                }
                // Must not be on same line as any item. An item *before* the comment
                // shares its line when the item's end and the comment's start match
                // (`item /* c */`); an item *after* the comment shares its line when the
                // comment's end and the item's start match (`/* c */ item`). Each
                // `is_same_line` call must pass its earlier position first — the helper
                // returns false for out-of-order args, so anchoring the leading-item check
                // on `s.start` (which follows the comment) wrongly reported "standalone"
                // for an inline-adjacent comment and force-expanded the container.
                !item_spans.iter().any(|s| {
                    self.is_same_line(s.end, c.span.start) || self.is_same_line(c.span.end, s.start)
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
    ) -> DocBuf {
        let d = self.d();
        let mut docs = DocBuf::new();
        // Track line reference — follows multi-line block comments to their
        // closing */ line (same logic as build_trailing_same_line_comments_doc in mod.rs)
        let mut line_ref = after_pos;
        for comment in comments_to_emit_in_range(self.comments, after_pos, upper_bound) {
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

    /// Build the leading-comment run before `target_start` — the member, statement, or
    /// element the comments lead.
    ///
    /// A thin adapter over the shared leading-comment emitter
    /// ([`Printer::push_leading_comment_run`]), for the callers holding their run as a
    /// slice rather than a range (each applies a per-site filter — dropping the previous
    /// statement's same-line trailing comments, or the ones pulled onto the delimiter
    /// line). The separator after each comment is prettier's `printLeadingComment`
    /// (space / soft `line` / blank-preserving hardline), keyed on the source around
    /// *that* comment.
    ///
    /// Every caller is an already-broken context — an expanded pattern, a multiline
    /// member prefix, a class/interface body, a statement list — so the soft `line`
    /// renders as a break here. That is why this reads as "hardline between own-line
    /// comments" at every site; it is the general rule landing in a broken group, not a
    /// policy of its own. A caller that ever measures this run inside a group that can
    /// *fit* gets prettier's collapse for free, which is the point of routing here.
    ///
    /// Used by: class body members, interface/enum members, block statement bodies,
    /// type literals, expanded object patterns. The orphaned-comment sibling is
    /// [`Self::push_orphaned_comment_run`]. Pushes into the caller's buffer
    /// (usually pooled) rather than returning a fresh `DocBuf` — a long run
    /// would spill the intermediate on every call just to be `extend`ed away.
    pub(crate) fn push_leading_comments_before(
        &self,
        parts: &mut DocBuf,
        comments: &[&internal::Comment],
        target_start: u32,
    ) {
        self.push_leading_comment_run(
            parts,
            comments.iter().copied(),
            target_start,
            LeadingGlue::Adjacent,
            self.d().empty(),
        );
    }

    /// Build the run for comments **orphaned by a dropped statement** — a bare `;`
    /// (`EmptyStatement`) never prints in a body list, so the comments in its gap have
    /// no node to lead. `gap_end` is a source position, not something that will be
    /// emitted.
    ///
    /// So the last comment must not glue to `gap_end`, and takes no separator at all —
    /// the caller's own next emission supplies it. Only an author blank line is recorded
    /// (a bare `literalline`, which the caller's hardline completes), because the
    /// caller's gap check starts later in the source and cannot rediscover it.
    ///
    /// Every *other* comment in the run leads the next comment, which is an ordinary
    /// leading run — so it routes through the shared emitter unchanged, and only the
    /// last comment is special-cased here.
    ///
    /// A sibling of [`Self::push_leading_comments_before`] rather than a flag on it:
    /// what differs is not a separator policy but whether the run has a target at all.
    pub(crate) fn push_orphaned_comment_run(
        &self,
        parts: &mut DocBuf,
        comments: &[&internal::Comment],
        gap_end: u32,
    ) {
        let Some((last, leading)) = comments.split_last() else {
            return;
        };
        self.push_leading_comment_run(
            parts,
            leading.iter().copied(),
            last.span.start,
            LeadingGlue::Adjacent,
            self.d().empty(),
        );
        parts.push(self.build_comment_doc(last));
        if self.has_blank_line_between(last.span.end, gap_end) {
            parts.push(self.d().literalline());
        }
    }

    /// Build docs for trailing comments at the end of a body (before closing `}`).
    ///
    /// Handles comments that appear after the last member/statement in a body,
    /// with blank line preservation between them. Returns a Vec of docs to append.
    ///
    /// Used by: class body, interface body, enum body, type literal, namespace body.
    pub(crate) fn build_trailing_body_comments_doc(&self, prev_end: u32, body_end: u32) -> DocBuf {
        let trailing_comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, prev_end, body_end)
                .filter(|c| !self.is_same_line(prev_end, c.span.start))
                .collect();

        if trailing_comments.is_empty() {
            return DocBuf::new();
        }

        let d = self.d();
        let mut docs = DocBuf::new();

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
    ) -> (DocBuf, Option<u32>) {
        self.delimiter_line_comment_prefix_impl(delim_pos, first_elem_start, false)
    }

    /// Object-literal variant of `delimiter_line_comment_prefix` that *also* pulls
    /// a block comment sharing the opening `{` line onto that line when the first
    /// property is on a later line (the object spans multiple lines). An object
    /// literal preserves its authored multi-line-ness, so a source newline before
    /// the first property means it will break, and the block trails `{` (like a
    /// line comment does) instead of dropping to the property's leading line.
    /// Collapsing containers (arrays, arg lists) keep the base behavior and call
    /// the plain form. The caller must treat a fired pull as forcing must-break
    /// (the prefix is only emitted on the break path).
    pub(in crate::printer) fn delimiter_line_comment_prefix_object(
        &self,
        delim_pos: u32,
        first_elem_start: u32,
    ) -> (DocBuf, Option<u32>) {
        self.delimiter_line_comment_prefix_impl(delim_pos, first_elem_start, true)
    }

    /// Assemble a computed `[…]` / `?.[…]` (or mapped-type `[K in T]`) that must BREAK
    /// because a line comment sits in the open→body gap: pull a `[`-line comment onto the
    /// open line (own-line ones keep their line, blank-preserving), emit `body`, and drop
    /// `]`. `body` is pre-built by the caller (key/index/interior plus any body→`]`
    /// trailing comments, per each printer's own rule), so this owns only the shared
    /// shell. `open` is the emitted bracket text (`[` or `?.[`); `bracket_char` is the
    /// source position of the `[` glyph (the scan/pull anchor), decoupled from `open` for
    /// the `?.[` form (`bracket_char + 1` is the first inside-bracket position). Shared by
    /// the computed-key, computed-member-access, and mapped-type break paths.
    pub(in crate::printer) fn build_bracket_line_comment_break(
        &self,
        open: &'static str,
        bracket_char: u32,
        body_start: u32,
        body: DocId,
    ) -> DocId {
        let d = self.d();
        let (line_prefix, pull_pos) = self.delimiter_line_comment_prefix(bracket_char, body_start);
        let mut inner =
            self.build_leading_comments_multiline(bracket_char + 1, body_start, pull_pos);
        inner.push(body);
        d.group_break(d.concat(&[
            d.text(open),
            d.concat(&line_prefix),
            d.indent_softline(d.concat(&inner)),
            d.softline(),
            d.text("]"),
        ]))
    }

    fn delimiter_line_comment_prefix_impl(
        &self,
        delim_pos: u32,
        first_elem_start: u32,
        pull_expanding_block: bool,
    ) -> (DocBuf, Option<u32>) {
        let pc = super::calls::PartitionedComments::new(
            self.comments,
            self.line_breaks,
            delim_pos,
            first_elem_start,
        );
        // The base rule gates the pull on forced expansion (a line comment, or a
        // block standalone on its own line). `pull_expanding_block` adds the
        // object case: a block on the delimiter line with the first element on a
        // later line — the object will break, so the block trails the `{`.
        let pull = (!pc.trailing_block.is_empty() || !pc.trailing_line.is_empty())
            && (super::calls::should_force_expansion_for_comments(
                self,
                delim_pos,
                first_elem_start,
            ) || (pull_expanding_block
                && !pc.trailing_block.is_empty()
                && !self.is_same_line(delim_pos, first_elem_start)));
        let mut prefix = DocBuf::new();
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
        comments: CommentVec<'c>,
        delimiter_pull_pos: Option<u32>,
    ) -> CommentVec<'c> {
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
        let mut in_range = comments_to_emit_in_range(self.comments, start, end).peekable();
        in_range.peek()?;

        let mut parts = DocBuf::new();
        for comment in in_range {
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
    /// Always breaks when a comment is present — used by the containers prettier
    /// keeps exploded (class body, interface body, namespace body). The
    /// containers that keep a fitting block comment inline (object literals and
    /// patterns, enum bodies, type literals) use the
    /// `build_empty_*_inline_with_comments_doc` helpers instead.
    pub(crate) fn build_empty_body_with_comments_doc(&self, body_span: Span) -> DocId {
        self.build_empty_delimited_with_comments_doc(body_span.start, body_span.end, "{}")
    }

    /// Build a Doc for an empty delimited container that may contain comments.
    ///
    /// Generic helper for both `{}` and `[]` containers. `pair` is the closed
    /// form (`"{}"` / `"[]"` — two single-byte delimiters), emitted whole on the
    /// comment-free path and sliced into its halves around the comment body.
    fn build_empty_delimited_with_comments_doc(
        &self,
        span_start: u32,
        span_end: u32,
        pair: &'static str,
    ) -> DocId {
        let d = self.d();
        let body_start = span_start + 1; // After opening delimiter
        let body_end = span_end.saturating_sub(1); // Before closing delimiter

        // Single binary search to find comments (no collect: peek covers both the
        // empty check and the is-last check).
        let mut comments =
            comments_to_emit_in_range(self.comments, body_start, body_end).peekable();

        if comments.peek().is_none() {
            return d.text(pair);
        }
        let mut comment_parts = DocBuf::new();

        while let Some(comment) = comments.next() {
            comment_parts.push(self.build_comment_doc(comment));
            // Add hardline after line comments, except for the last one
            // (the hardline before closing delimiter handles that)
            if !comment.is_block && comments.peek().is_some() {
                comment_parts.push(d.hardline());
            }
        }

        d.concat(&[
            d.text(&pair[..1]),
            d.indent(d.concat(&[d.hardline(), d.concat(&comment_parts)])),
            d.hardline(),
            d.text(&pair[1..]),
        ])
    }

    /// Build a Doc for an empty `{}` body whose only content is a dangling
    /// comment, keeping a fitting block comment inline with bracket spacing
    /// (`{ /* c */ }`).
    ///
    /// tsv applies bracket spacing uniformly: object literals, destructuring
    /// patterns, enum bodies, and type literals all print a comment-only empty
    /// body as `{ /* c */ }`. Prettier tightens every one of these to
    /// `{/* c */}`, so this is a divergence — see conformance_prettier.md
    /// §Empty-object comment bracket spacing. A truly empty `{}` (no comment)
    /// has no content to space and stays tight in both. See
    /// [`Self::build_empty_inline_with_comments_doc`].
    pub(crate) fn build_empty_braces_inline_with_comments_doc(&self, body_span: Span) -> DocId {
        let d = self.d();
        let sep = d.line();
        self.build_empty_inline_with_comments_doc(body_span.start, body_span.end, "{}", sep)
    }

    /// Build a Doc for an empty bracket `[]` body whose only content is a
    /// dangling comment, keeping a fitting block comment inline (`[/* c */]`).
    ///
    /// Used by array literals/patterns and tuple types. See
    /// [`Self::build_empty_inline_with_comments_doc`].
    pub(crate) fn build_empty_brackets_inline_with_comments_doc(&self, span: Span) -> DocId {
        self.build_empty_brackets_inline_with_comments_doc_range(span.start, span.end)
    }

    /// Build a Doc for an empty bracket `[]` body with explicit bounds (e.g. an
    /// array pattern with a type annotation). See
    /// [`Self::build_empty_brackets_inline_with_comments_doc`].
    pub(crate) fn build_empty_brackets_inline_with_comments_doc_range(
        &self,
        body_start: u32,
        body_end: u32,
    ) -> DocId {
        let d = self.d();
        let sep = d.softline();
        self.build_empty_inline_with_comments_doc(body_start, body_end, "[]", sep)
    }

    /// Build a Doc for an empty paren list whose only content is a dangling
    /// comment, keeping a fitting block comment inline (`fn(/* c */)`).
    ///
    /// The paren counterpart of [`Self::build_empty_brackets_inline_with_comments_doc`],
    /// shared by every empty paren list: call and `new` arguments (including the
    /// member-chain and optional-call `?.(` forms, hence the `opening` prefix),
    /// value parameter lists (function, method, arrow), and signature-level type
    /// params. A line comment inside `()` cannot stay inline — `//` runs to the end
    /// of the line and would swallow the `)` — so it forces the break; this is the
    /// one delimiter pair where inlining is a correctness bug rather than a layout
    /// choice.
    ///
    /// `paren_open` is the `(` position and `paren_close_after` the position past
    /// the `)` (as returned by `find_closing_paren`).
    // TODO: the sibling swallow in CALLEE position is NOT covered here. A line comment
    // between a callee and its `(` — `call // c⏎()`, and the optional-call `call?. // c⏎()`
    // — is emitted by the callee/member-chain path, not by this emitter, and still prints
    // inline (`call // c();`), swallowing the `()`. Same bug class, different mechanism
    // (callee-position trivia, not a dangling comment inside a delimiter pair), so it wants
    // its own fixtures-first fix. `swallow_audit` cannot see it — that gate runs over
    // `tests/fixtures` only, and no fixture carries the shape; prettier's own
    // `js/call/no-argument/no-arguments.js` does.
    pub(crate) fn build_empty_parens_inline_with_comments_doc(
        &self,
        paren_open: u32,
        paren_close_after: u32,
        opening: &'static str,
    ) -> DocId {
        let d = self.d();
        let sep = d.softline();
        self.build_empty_bracketed_with_comments_doc(
            paren_open,
            paren_close_after,
            d.text(opening),
            ")",
            sep,
        )
    }

    /// Build a Doc for an empty parameter list, preserving any dangling comments
    /// inside the parens (`fn(/* c */)`). Shared by every empty parameter list —
    /// value params (function, method, arrow) and signature-level type params — so
    /// the dangling rule of
    /// [`Self::build_empty_parens_inline_with_comments_doc`] reaches all of them.
    ///
    /// `search_limit` bounds the depth-tracked `)` search, which skips comment and
    /// string content so a `)` inside a comment can't be mistaken for the closer.
    /// Callers that know a tighter bound (an arrow's body start) pass it; the rest
    /// pass the source length. Yields a bare `()` when there is no `(` to anchor to.
    pub(crate) fn build_empty_params_with_comments_doc(
        &self,
        params_start: Option<u32>,
        search_limit: u32,
    ) -> DocId {
        if let Some(open) = params_start
            && let Some(close_after) = self.find_closing_paren(open, search_limit)
        {
            return self.build_empty_parens_inline_with_comments_doc(open, close_after, "(");
        }
        self.d().text("()")
    }

    /// Build a Doc for an empty delimited container whose only content is a
    /// dangling comment, matching prettier 3.9's `printDanglingCommentsInList`
    /// (prettier PRs #18617 / #18615): a block comment that fits stays inline
    /// (`[/* c */]`, `{/* c */}`); a line comment can't be inlined and forces
    /// the break, and an overflowing or multi-line block comment breaks via the
    /// enclosing group. `sep` is the open/close separator — `softline` (no
    /// space) for brackets, object literals/patterns, and enum bodies, `line`
    /// (bracket spacing) for type literals.
    ///
    /// Containers that always break with a dangling comment (class, interface,
    /// and namespace bodies) keep using
    /// [`Self::build_empty_delimited_with_comments_doc`] instead.
    fn build_empty_inline_with_comments_doc(
        &self,
        span_start: u32,
        span_end: u32,
        pair: &'static str,
        sep: DocId,
    ) -> DocId {
        let opening = self.d().text(&pair[..1]);
        self.build_empty_bracketed_with_comments_doc(span_start, span_end, opening, &pair[1..], sep)
    }

    /// Like [`Self::build_empty_inline_with_comments_doc`] but with an arbitrary
    /// `opening` doc (which may carry a prefix, e.g. a parenthesized-intersection
    /// `(A & {`) and a static `closing` string (`}`, `]`, `})`). The empty body
    /// stays delimiter-tight when comment-free (`{}` not `{ }`), so a union-member
    /// or paren-intersection object type that reaches the alignment path prints
    /// with no spurious bracket space and preserves any interior comment.
    pub(crate) fn build_empty_bracketed_with_comments_doc(
        &self,
        span_start: u32,
        span_end: u32,
        opening: DocId,
        closing: &'static str,
        sep: DocId,
    ) -> DocId {
        let d = self.d();
        let body_start = span_start + 1; // After opening delimiter
        let body_end = span_end.saturating_sub(1); // Before closing delimiter

        let comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, body_start, body_end).collect();

        if comments.is_empty() {
            return d.concat(&[opening, d.text(closing)]);
        }

        // Dangling comments join with hardline (prettier `printDanglingComments`).
        let mut comment_parts = DocBuf::new();
        for (i, comment) in comments.iter().enumerate() {
            if i > 0 {
                comment_parts.push(d.hardline());
            }
            comment_parts.push(self.build_comment_doc(comment));
        }

        // A line comment can't be inlined, so it forces the break; a fitting
        // block comment stays inline (the group breaks on overflow / a multi-line
        // block comment's own hardlines).
        let has_line = comments.iter().any(|c| !c.is_block);
        let close_sep = if has_line { d.hardline() } else { sep };

        d.group(d.concat(&[
            opening,
            d.indent(d.concat(&[sep, d.concat(&comment_parts)])),
            close_sep,
            d.text(closing),
        ]))
    }

    /// Append a function/method body with comment splitting between signature and body.
    ///
    /// Block comments stay inline: `gen() /* c */ {}`
    /// Line comments get absorbed into the block body as leading content.
    pub(crate) fn append_body_with_sig_comments(
        &self,
        parts: &mut DocBuf,
        sig_end: u32,
        body: &internal::BlockStatement<'_>,
    ) {
        let d = self.d();
        let body_start = body.span.start;
        if self.has_comments_to_emit_between(sig_end, body_start) {
            let mut absorbed = DocBuf::new();
            for comment in comments_to_emit_in_range(self.comments, sig_end, body_start) {
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
    /// Same-line comments stay with the member (a block inline, a line via
    /// `line_suffix`); an **own-line** comment is deferred to `deferred` (own line,
    /// blank preserved) for the caller to emit **after** the `;`, matching prettier
    /// (the member doc doesn't own the `;`). `deferred` is empty on the common
    /// no-comment path.
    ///
    /// Shared by method/call/construct signatures in interfaces and type literals
    /// and by declare functions (all use the type-member `;` binding —
    /// `split_member_terminator_gap_comments`).
    pub(crate) fn append_signature_end_comments(
        &self,
        parts: &mut DocBuf,
        return_type: Option<&internal::TSTypeAnnotation<'_>>,
        paren_pos: Option<u32>,
        span_end: u32,
        deferred: &mut DocBuf,
    ) {
        let content_end = return_type.map_or_else(
            || {
                paren_pos
                    .and_then(|p| self.find_closing_paren(p, span_end))
                    .unwrap_or(span_end)
            },
            |rt| rt.span.end,
        );
        deferred.extend(self.split_member_terminator_gap_comments(parts, content_end, span_end));
    }

    /// Partition the comments in a content→separator gap `[start, sep_pos)`, binding
    /// the separator (`,` / `;`) to the content the way prettier does:
    ///
    /// - a **same-line** comment is pushed to `parts` (before the separator) — a block
    ///   inline (`X /* c */<sep>`, preserved), a line via `line_suffix` (zero width, so
    ///   it floats past the separator to the next hardline → `X<sep> // c`) — *except*
    ///   that when `block_after_separator` is set a same-line **block** is instead
    ///   *returned* (deferred), so it trails **after** the separator (`X<sep> /* c */`);
    /// - an **own-line** comment is *returned* (not pushed), each on its own line
    ///   (`hardline` + comment), for the caller to emit **after** the separator so the
    ///   author's line break is kept and a `//` can't swallow the separator; when
    ///   `block_after_separator` (the `;`-terminator case), a single blank line before it
    ///   (relative to the content, then the previous comment) is also preserved
    ///   (`literalline`), matching prettier — the `,`-separator case keeps no blank
    ///   (prettier emits none in a list element→comma gap).
    ///
    /// `block_after_separator` is the prettier-3.9 behavior for the statement/member
    /// **`;` terminator** (the `;` is pure structure, so trailing a block past it is
    /// lossless — `expr; /* c */`); the list **`,` separator** passes `false` and keeps
    /// a same-line block before the comma (`X /* c */,`) — prettier did not change that.
    ///
    /// Caller idiom: `let after = self.split_separator_gap_comments(parts, start, sep,
    /// block_after_separator); parts.push(sep_text); parts.extend(after);`. Shared by the
    /// list `,` separator (`emit_multiline_comma_with_comments`, `false`) and the
    /// statement/member `;` terminator (variable / expression-statement / class-property,
    /// `true`). Emitting an own-line comment *before* the separator would put the
    /// separator on the comment's line — a `//` swallows it (content loss), a block just
    /// diverges from prettier.
    pub(crate) fn split_separator_gap_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        sep_pos: u32,
        block_after_separator: bool,
    ) -> DocBuf {
        // The two axes move together here: a `;` terminator (`true`) trails a same-line
        // block after the separator AND preserves a blank line, while a `,`/for-header
        // separator (`false`) does neither. The mixed `MemberTerminator` case (block
        // before, blank preserved) uses `split_member_terminator_gap_comments`.
        self.push_gap_comments(
            parts,
            start,
            sep_pos,
            block_after_separator,
            block_after_separator,
        )
    }

    /// The [`split_separator_gap_comments`](Self::split_separator_gap_comments) caller
    /// idiom for a **`;` terminator**, in one call: the gap's pre-`;` comments, the `;`,
    /// then the comments that belong after it.
    ///
    /// The ordering is the reason this exists rather than three lines at each site: the
    /// returned docs must be pushed *after* the `;` text, and a site that inlines the
    /// idiom can invert it silently — an own-line comment emitted before the separator
    /// puts the `;` on the comment's line, where a `//` swallows it outright.
    ///
    /// `block_after_separator` is the terminator's own axis, not a caller preference:
    /// `true` for a statement/member `;` (prettier trails a same-line block past it —
    /// `expr; /* c */`), `false` where the operand keeps it (`import =` / `export =` /
    /// `export as namespace`). A caller whose split is *conditional* keeps the raw idiom.
    pub(crate) fn push_semicolon_with_gap_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        semicolon_pos: u32,
        block_after_separator: bool,
    ) {
        let after =
            self.split_separator_gap_comments(parts, start, semicolon_pos, block_after_separator);
        parts.push(self.d().text(";"));
        parts.extend(after);
    }

    /// The **type-member `;`** variant of `split_separator_gap_comments`: a same-line
    /// block stays *before* the `;` (`a: A /* c */;`, like a list separator) **but** a
    /// blank line before an own-line comment IS preserved (like a statement terminator).
    /// This mixed binding is what prettier does for a type-literal / interface member
    /// terminator, which the single `block_after_separator` bool can't express. Same
    /// caller idiom (the returned own-line docs are emitted by the type-element *joiner*
    /// after its `;`, since the member doc doesn't own the `;`).
    pub(crate) fn split_member_terminator_gap_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        sep_pos: u32,
    ) -> DocBuf {
        self.push_gap_comments(parts, start, sep_pos, false, true)
    }

    /// Core of the gap-comment partition, with the two policy axes decoupled:
    /// `block_after` moves a **same-line block** past the separator (deferred), and
    /// `preserve_blank` keeps a single blank line before a deferred **own-line** comment
    /// (`literalline`). A same-line line comment always uses `line_suffix` (zero width,
    /// floats past the separator); an own-line comment is always deferred on its own
    /// `hardline`. `prev` tracks the content/prior-comment end for blank detection.
    fn push_gap_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        sep_pos: u32,
        block_after: bool,
        preserve_blank: bool,
    ) -> DocBuf {
        let d = self.d();
        let mut deferred = DocBuf::new();
        let mut prev = start;
        for comment in comments_to_emit_in_range(self.comments, start, sep_pos) {
            if self.is_same_line(start, comment.span.start) {
                if block_after && comment.is_block {
                    deferred.push(self.build_trailing_comment_doc(comment));
                } else {
                    parts.push(self.build_trailing_comment_doc(comment));
                }
            } else {
                if preserve_blank && self.has_blank_line_between(prev, comment.span.start) {
                    deferred.push(d.literalline());
                }
                deferred.push(d.hardline());
                deferred.push(self.build_comment_doc(comment));
            }
            prev = comment.span.end;
        }
        deferred
    }

    /// Append leading inline block comments (`/*content*/ ` format) between two positions.
    ///
    /// Only emits block comments; line comments are skipped (they would have been
    /// detected earlier and routed to the multiline path). Counterpart of
    /// [`Self::append_trailing_inline_block_comments`].
    pub(crate) fn append_leading_inline_block_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if comment.is_block {
                // One text node (`/*content*/ `) — callers may pass `parts` as
                // fill items, so the space can't split into its own node. The
                // full span is the verbatim `/*content*/` (delimiters included).
                let mut w = d.pool_writer();
                w.push_str(comment.span.extract(self.source));
                w.push(' ');
                let doc = w.finish_text();
                // A comment emission that can't route through `build_comment_doc` (the
                // trailing space must share the node), so it tags its own ledger node.
                #[cfg(feature = "comment_check")]
                d.tag_comment_doc(doc, comment.span, self.source);
                parts.push(doc);
            }
        }
    }

    /// Append trailing inline block comments (` /*content*/` format) between two positions.
    ///
    /// Only emits block comments; line comments are skipped (they would have been
    /// detected earlier and routed to the multiline path).
    pub(crate) fn append_trailing_inline_block_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if comment.is_block {
                // One text node (` /*content*/`) — callers may pass `parts` as
                // fill items, so the space can't split into its own node. The
                // full span is the verbatim `/*content*/` (delimiters included).
                let mut w = d.pool_writer();
                w.push(' ');
                w.push_str(comment.span.extract(self.source));
                let doc = w.finish_text();
                // A comment emission that can't route through `build_comment_doc` (the
                // leading space must share the node), so it tags its own ledger node.
                #[cfg(feature = "comment_check")]
                d.tag_comment_doc(doc, comment.span, self.source);
                parts.push(doc);
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
        before: &mut DocBuf,
        after: &mut DocBuf,
        elem_end: u32,
        end_boundary: u32,
    ) {
        // Zero-comment fast gate: with no comment in the window, both splits emit
        // nothing wherever the comma is — skip the comma scan entirely.
        if !self.has_comments_to_emit_between(elem_end, end_boundary) {
            return;
        }
        match self.find_comma_in_range(elem_end, end_boundary) {
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
    /// `preserve_blank_before` keeps a blank line the author left *before* the next
    /// element (or its own-line leading comment, `A,⏎⏎/* c */⏎B`). Prettier preserves
    /// it for **tuples** and **function-type param lists** (function/constructor
    /// types, method/call/construct signatures — same as regular function params) but
    /// collapses it for type-parameter / type-argument lists, so those two caller
    /// families pass `true` and the type-param/type-arg callers pass `false`.
    pub(crate) fn emit_multiline_comma_with_comments(
        &self,
        parts: &mut DocBuf,
        elem_end: u32,
        next_start: u32,
        blank_rule: BlankRule,
    ) -> u32 {
        let d = self.d();
        let comma_pos = self.find_list_comma(elem_end, next_start);

        // The comma binds to the element; same-line gap comments stay before it
        // (block inline, line via `line_suffix`), own-line ones defer to after it
        // (leading the next element). A same-line block stays *before* the comma
        // (`block_after_separator: false`) — prettier 3.9 only moved the `;` case.
        // See `split_separator_gap_comments`.
        let deferred_own_line =
            self.split_separator_gap_comments(parts, elem_end, comma_pos, false);
        parts.push(d.text(","));
        parts.extend(deferred_own_line);

        // Same-line trailing comments after comma (line comments that consume the line).
        // A line comment goes through `line_suffix` (zero width) so it never forces the
        // preceding element to break; it flushes at the hardline below (prettier's
        // `lineSuffix`). A block stays inline, width counted.
        let mut after_comma_end = comma_pos + 1;
        for comment in comments_to_emit_in_range(self.comments, comma_pos + 1, next_start) {
            if self.is_same_line(elem_end, comment.span.start) {
                parts.push(self.build_trailing_comment_doc(comment));
                after_comma_end = comment.span.end;
            }
        }

        // Hardline to separate from next element, optionally preserving an author blank line
        // before the next own-line leading comment. WHICH blank counts is the caller's list
        // kind, not this emitter's business — see [`BlankRule`].
        if blank_rule != BlankRule::None {
            // **in source**: `next_lead` bounds a raw blank-line scan, which cannot tell a
            // comment's own newlines from an author's blank line — so it must stop at every
            // comment in the gap, not just the ones this caller emits.
            let next_lead = self
                .comments_in_source_between(after_comma_end, next_start)
                .find(|c| !self.is_same_line(elem_end, c.span.start))
                .map_or(next_start, |c| c.span.start);
            let has_blank = match blank_rule {
                // Measured from the ELEMENT's end, so a blank the author put before the
                // comma still counts — and one after a comma pushed onto its own line
                // does not.
                BlankRule::NextLineEmpty => self.is_next_line_empty(elem_end, next_lead),
                // Measured from past the comma, prettier's array/tuple rule.
                BlankRule::AfterComma => self.has_blank_line_between(after_comma_end, next_lead),
                // Bailed out above; spelled here so the match stays total.
                BlankRule::None => false,
            };
            if has_blank {
                parts.push(d.literalline());
            }
        }
        parts.push(d.hardline());

        after_comma_end
    }
}
