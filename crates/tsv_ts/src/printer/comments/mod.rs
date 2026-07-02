// Comment handling for TypeScript printer
//
// This module handles all comment-related operations:
// - Building Doc representations for comments
// - Printing comments directly to buffer
// - Finding and filtering comments in ranges
// - Handling leading/trailing/inline comments
//
// ## Module Organization
//
// - **mod.rs** (this file): The `CommentSpacing` / `CommentFilter` enums and the
//   generic comment-emission primitives every other module builds on.
// - **render.rs**: Single-comment text-layout leaves (block-comment framing,
//   indentable / preserved block comments, trailing line/block comment docs).
// - **paren.rs**: Stripped-grouping-paren comment handling (promotion across `=`
//   / operators, trailing-paren comment preservation, removed-paren prepends).
// - **scan.rs**: Pure source span-math helpers (comma/angle/blank-line scanning).
// - **declarations.rs**: Member-keyword / modifier-marker / marker→colon /
//   heritage / keyword→name comment emitters.
// - **lists.rs**: List- and body-level comment emitters (leading/trailing body
//   comments, delimiter-line prefixes, empty-container comments, comma emission).
// - **element_comma.rs**: The single source of the `trailingComma: 'none'`
//   comment-position contract for inline element lists (block-before / comma /
//   block-after-on-last / line-suffix), shared by the object/array pattern and
//   object-literal builders.

mod declarations;
mod element_comma;
mod lists;
mod paren;
mod render;
mod scan;

pub(crate) use declarations::HeritageKeyword;

// Re-export for submodules to use `super::X` instead of `super::super::X`.
pub(super) use super::{Printer, calls, layout};

use smallvec::SmallVec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::{Comment, comments_in_range};

/// Small stack-allocated vector of comment references. Inline capacity 4 keeps
/// the common comment gaps off the heap: 0–2 comments are the bulk, and a short
/// stacked `//` block (3–4 lines, common in documented code) still fits inline.
/// A larger run spills to a single heap alloc — exactly what a `Vec` would do.
pub(crate) type CommentVec<'a> = SmallVec<[&'a Comment; 4]>;

/// Spacing style for comments in doc building
#[derive(Debug, Clone, Copy)]
pub(crate) enum CommentSpacing {
    /// Space before comment: ` /* c */`
    Leading,
    /// Space after comment: `/* c */ `
    Trailing,
    /// No spacing: `/* c */`
    None,
}

impl CommentSpacing {
    /// `Trailing` when followed by type params (`/* c */ <T>`),
    /// `Leading` when followed by parens (` /* c */()`).
    pub(crate) fn for_type_params(has_type_params: bool) -> Self {
        if has_type_params {
            Self::Trailing
        } else {
            Self::Leading
        }
    }
}

/// Filter for which comment types to include
#[derive(Debug, Clone, Copy)]
pub(crate) enum CommentFilter {
    /// Include all comments (block and line)
    All,
    /// Only include block comments (/* */)
    BlockOnly,
}

/// How a leading-comment run decides whether a *block* comment hugs the token
/// that follows it (a trailing space, `/* c */ X`) rather than dropping to its
/// own line. The rest of the run is identical across sites — one
/// `build_comment_doc` per comment, and a blank-preserving `hardline` toward the
/// next comment (or the terminal) for every comment that doesn't hug — so only
/// this glue test varies, and [`push_leading_comment_run`](Printer::push_leading_comment_run)
/// takes it as a mode.
#[derive(Debug, Clone, Copy)]
pub(crate) enum LeadingGlue {
    /// A block hugs when it shares a source line with the *next* comment (or the
    /// terminal, for the last one). The RHS-of-`=`/`:` / keyword-operand form
    /// ([`build_rhs_comments_opt`](Printer::build_rhs_comments_opt)).
    Adjacent,
    /// `Adjacent`, plus a single-line block glued to the operator hugs the value
    /// across a source newline — prettier's assignment/call pull-up
    /// ([`build_rhs_comments_glued_opt`](Printer::build_rhs_comments_glued_opt)).
    AdjacentGlued,
    /// A block hugs only when it shares a source line with the terminal member.
    /// The member-leading form (interface / intersection members). Differs from
    /// `Adjacent` only for a multi-comment run whose interior blocks share a line
    /// with each other but not with the member (`/* a */ /* b */⏎member`): this
    /// keeps each on its own line, where `Adjacent` would glue the leading pair.
    Terminal,
}

impl<'a> Printer<'a> {
    /// Whether a comment between two neighbors can't share a line with either — any
    /// line comment (it runs to EOL), or a block comment isolated from *both* `prev`
    /// (at the comment's start) and `next` (at its end). The shared "isolated from
    /// both neighbors" rule behind the function-parameter-list expansion gate
    /// (`has_own_line_comment_between`) and the intersection-member break gate
    /// (`intersection_has_isolated_member_comment`): an adjacency on either side keeps
    /// the comment inline (`a /* c */ b`), matching prettier, which collapses both
    /// `a,⏎/* c */ b` and `a /* c */,⏎b` back to the inline form.
    pub(crate) fn comment_isolated_from_neighbors(
        &self,
        prev: u32,
        c: &Comment,
        next: u32,
    ) -> bool {
        !c.is_block
            || (!self.is_same_line(prev, c.span.start) && !self.is_same_line(c.span.end, next))
    }

    /// Emit a `hardline` after an own-line comment in a per-line comment list,
    /// preserving an author blank line as a leading `literalline` when the source
    /// left one between `comment_end` and `next` (the following own-line comment, or
    /// the element the comments lead). The blank-preserving counterpart to a bare
    /// `hardline` separator.
    pub(crate) fn push_blank_preserving_hardline(
        &self,
        parts: &mut DocBuf,
        comment_end: u32,
        next: u32,
    ) {
        let d = self.d();
        if self.has_blank_line_between(comment_end, next) {
            parts.push(d.literalline());
        }
        parts.push(d.hardline());
    }

    /// Emit a run of leading comments before a member/element starting at
    /// `member_start`: a block comment inline-adjacent to the member hugs it with a
    /// trailing space (`/* c */ X`); every other comment (own-line block, or line)
    /// takes its own line via a blank-preserving hardline (toward the next comment, or
    /// the member). Shared by the member-leading sites whose only difference is the
    /// member position (interface members, intersection members after `&`).
    pub(crate) fn emit_member_leading_comments(
        &self,
        parts: &mut DocBuf,
        comments: &[&Comment],
        member_start: u32,
    ) {
        self.push_leading_comment_run(
            parts,
            comments.iter().copied(),
            member_start,
            LeadingGlue::Terminal,
        );
    }

    /// Emit the comma and inter-declarator comments for a declarator gap that
    /// contains a **line** comment (the forced-break case). Block comments before
    /// the first line comment trail the previous initializer inline
    /// (`= 0 /* c */`); the comma is placed before the first line comment; then
    /// each remaining comment either trails the comma on the same output line (a
    /// line comment via `line_suffix`, a same-line block inline) or drops onto its
    /// own line after a `hardline`. `continuation` is emitted right after each
    /// own-line `hardline`: the variable-declaration site passes `INDENT` text
    /// (its declarators aren't wrapped in `d.indent()`), the for-init site passes
    /// an empty doc (its run is). Does NOT emit the trailing break to the next
    /// declarator — the caller owns that, since the indent strategy differs.
    /// Shared by the variable-declarator and for-init inter-declarator sites.
    pub(crate) fn push_inter_declarator_line_comment_gap(
        &self,
        parts: &mut DocBuf,
        comments: &[&Comment],
        continuation: DocId,
    ) {
        let d = self.d();
        let first_line_idx = comments.iter().position(|c| !c.is_block).unwrap_or(0);
        for comment in &comments[..first_line_idx] {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        parts.push(d.text(","));
        // `needs_hardline` starts true when block comments precede the comma (it
        // then sits between them and the first line comment, so the next comment
        // drops to its own line).
        let mut needs_hardline = first_line_idx > 0;
        for comment in &comments[first_line_idx..] {
            if needs_hardline {
                parts.push(d.hardline());
                parts.push(continuation);
                parts.push(self.build_comment_doc(comment));
            } else {
                parts.push(self.build_trailing_comment_doc(comment));
            }
            needs_hardline = !comment.is_block;
        }
    }

    /// Build a Doc for inline comments between two positions with specified spacing and filter
    ///
    /// Returns a Doc containing all comments in the range with the specified spacing.
    /// Returns empty concat if no comments found.
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn build_comments_between(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
    ) -> DocId {
        self.build_comments_between_filtered(start, end, spacing, CommentFilter::All)
    }

    /// Build a Doc for inline comments with filtering
    pub(crate) fn build_comments_between_filtered(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> DocId {
        self.build_comments_between_filtered_opt(start, end, spacing, filter)
            .unwrap_or_else(|| self.d().empty())
    }

    /// Build a Doc for inline comments with filtering, returning None if no comments.
    ///
    /// This is more efficient than `has_comments_between` + `build_comments_between`
    /// because it uses a single binary search instead of two.
    pub(crate) fn build_comments_between_filtered_opt(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> Option<DocId> {
        let d = self.d();
        // Single binary search to find first comment
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);

        // Check if any comments exist in range (considering filter)
        let has_comments = self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= end)
            .any(|c| !matches!(filter, CommentFilter::BlockOnly) || c.is_block);

        if !has_comments {
            return None;
        }

        // Build docs for matching comments
        let mut parts = DocBuf::new();
        for comment in self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= end)
        {
            // Apply filter
            if matches!(filter, CommentFilter::BlockOnly) && !comment.is_block {
                continue;
            }

            match spacing {
                CommentSpacing::Leading => {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                CommentSpacing::Trailing => {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
                CommentSpacing::None => {
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }
        Some(d.concat(&parts))
    }

    /// Build a Doc for inline comments between two positions (leading space)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc(&self, start: u32, end: u32) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::Leading)
    }

    /// Build a Doc for trailing comments where a line comment must force the
    /// following content onto a new line.
    ///
    /// Like `build_comments_between(_, _, Trailing)` for block comments, but
    /// for line comments emits a hardline after the comment instead of a space.
    /// Use when the comment precedes content that must not be swallowed by the
    /// line comment (e.g., `=> // leading\nT`, `: // leading\nT`).
    pub(crate) fn build_trailing_comments_break_for_line(&self, start: u32, end: u32) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments_in_range(self.comments, start, end) {
            parts.push(self.build_comment_doc(comment));
            if comment.is_block {
                parts.push(d.text(" "));
            } else {
                parts.push(d.hardline());
            }
        }
        // `concat` short-circuits the no-comments-in-range case to `empty()`.
        d.concat(&parts)
    }

    /// Leading-spacing counterpart of `build_trailing_comments_break_for_line`: a
    /// leading space before each comment, and a line comment forces the *following*
    /// content onto a new line (`hardline`) so it can't be swallowed. A block comment
    /// glues to the following token (` /* c */X`), matching the inline `Leading` form.
    /// Use where the comment leads the next token across a gap that would otherwise
    /// glue it (e.g. an indexed-access object→`[` gap, `A // c⏎[K]`).
    pub(crate) fn build_leading_comments_break_for_line(&self, start: u32, end: u32) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments_in_range(self.comments, start, end) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
            if !comment.is_block {
                parts.push(d.hardline());
            }
        }
        // `concat` short-circuits the no-comments-in-range case to `empty()`.
        d.concat(&parts)
    }

    /// Build a Doc for inline comments, returning None if no comments.
    ///
    /// Use this instead of `has_comments_between` + `build_inline_comments_between_doc`
    /// to avoid redundant binary searches.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_opt(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        self.build_comments_between_filtered_opt(
            start,
            end,
            CommentSpacing::Leading,
            CommentFilter::All,
        )
    }

    /// Build a Doc for inline comments between two positions (no spaces)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_no_leading_space(
        &self,
        start: u32,
        end: u32,
    ) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::None)
    }

    /// Build a Doc for inline comments (no spaces), returning None if no comments.
    ///
    /// Use this instead of `has_comments_between` + `build_inline_comments_between_doc_no_leading_space`
    /// to avoid redundant binary searches.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_no_leading_space_opt(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        self.build_comments_between_filtered_opt(
            start,
            end,
            CommentSpacing::None,
            CommentFilter::All,
        )
    }

    /// Build a Doc for inline comments between two positions (trailing space)
    ///
    /// Used when comments appear before an element and need a space after.
    /// Example: `{a, /* comment */ b}` - the comment needs a space after it.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_trailing_space(
        &self,
        start: u32,
        end: u32,
    ) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::Trailing)
    }

    /// Build inline comments between two positions with line-comment-safe trailing spacing.
    ///
    /// A block comment keeps the following value (or next comment) on its `*/`
    /// line when the source did (`/* comment */ expr`), and stays on its own line
    /// when the source broke (`/* comment */\nexpr`) — the author's layout is
    /// preserved. Line comments always get a hardline (`// comment\nexpr`) so they
    /// can't absorb the value as comment text.
    /// Use for any position where a comment appears before an expression (RHS of `=`,
    /// after keywords like `return`/`await`, after operators like `!`/`...`, etc.).
    pub(crate) fn build_rhs_comments_opt(&self, start: u32, end: u32) -> Option<DocId> {
        self.build_leading_comment_run_opt(start, end, LeadingGlue::Adjacent)
    }

    /// Like `build_rhs_comments_opt`, but a single-line block comment glued to the
    /// operator (not on its own line) hugs the value with a space even when the
    /// value follows on the next source line — prettier pulls the value up in the
    /// assignment/call layout (`= /* c */⏎v` → `= /* c */ v`). Positions that keep
    /// the author's line break for a glued block (decorators, `return`/`throw`/`await`
    /// keyword operands, object property values, …) stay on the non-gluing
    /// `build_rhs_comments_opt`.
    pub(crate) fn build_rhs_comments_glued_opt(&self, start: u32, end: u32) -> Option<DocId> {
        self.build_leading_comment_run_opt(start, end, LeadingGlue::AdjacentGlued)
    }

    /// Emit a run of leading comments before `terminal_pos` — the value, member,
    /// or body the comments lead. Each comment is emitted with `build_comment_doc`;
    /// a *block* comment that hugs the following token (per `glue`) gets a trailing
    /// space (`/* c */ X`), and every other comment (an own-line block, or any line
    /// comment) drops to its own line via a blank-preserving `hardline` toward the
    /// next comment (or `terminal_pos` for the last). Preserving an author blank
    /// line before the value / next comment matches prettier, which keeps one blank
    /// in this "comment before expression" position everywhere (RHS of `=`/`:`, call
    /// args, `return`/`await`, unary operands, …). The single loop behind
    /// [`build_rhs_comments_opt`](Self::build_rhs_comments_opt),
    /// [`build_rhs_comments_glued_opt`](Self::build_rhs_comments_glued_opt),
    /// [`emit_member_leading_comments`](Self::emit_member_leading_comments), and the
    /// arrow-body leading run.
    pub(crate) fn push_leading_comment_run<'c>(
        &self,
        parts: &mut DocBuf,
        comments: impl Iterator<Item = &'c Comment>,
        terminal_pos: u32,
        glue: LeadingGlue,
    ) {
        let d = self.d();
        let mut comments = comments.peekable();
        while let Some(comment) = comments.next() {
            parts.push(self.build_comment_doc(comment));
            // The next thing after this comment is the following comment, or the
            // terminal (value/member/body) for the last one.
            let next = comments.peek().map_or(terminal_pos, |c| c.span.start);
            let hugs = comment.is_block
                && match glue {
                    LeadingGlue::Adjacent => self.is_same_line(comment.span.end, next),
                    // A glued (not own-line) single-line block hugs across a source
                    // newline; the same-line-as-next case still hugs as in `Adjacent`.
                    LeadingGlue::AdjacentGlued => {
                        self.is_same_line(comment.span.end, next)
                            || !self.comment_forces_own_line(comment)
                    }
                    LeadingGlue::Terminal => self.is_same_line(comment.span.end, terminal_pos),
                };
            if hugs {
                // Value (or next comment) shares the `*/` line — keep it glued.
                parts.push(d.text(" "));
            } else {
                // Line comment, or a block whose value/next comment is on a later
                // source line: keep them on separate lines (preserve the author's
                // layout; a line comment must break so it can't absorb the value).
                self.push_blank_preserving_hardline(parts, comment.span.end, next);
            }
        }
    }

    /// Build a leading-comment run over `[start, end)` into a fresh `DocBuf`,
    /// returning `None` when the range holds no comments. The `Option`-returning
    /// form of [`push_leading_comment_run`](Self::push_leading_comment_run) that
    /// the RHS-comment wrappers use.
    fn build_leading_comment_run_opt(
        &self,
        start: u32,
        end: u32,
        glue: LeadingGlue,
    ) -> Option<DocId> {
        let mut parts = DocBuf::new();
        self.push_leading_comment_run(
            &mut parts,
            comments_in_range(self.comments, start, end),
            end,
            glue,
        );
        if parts.is_empty() {
            None
        } else {
            Some(self.d().concat(&parts))
        }
    }

    /// Prepend optional RHS leading comments — block comments in the gap between an
    /// `=`/`:` and the value (`build_rhs_comments_opt`) — to an already-built
    /// `value_doc`, returning `value_doc` unchanged when the gap carries none.
    /// Centralizes the `match build_rhs_comments_opt { Some(c) => concat([c, v]),
    /// None => v }` idiom shared by the initializer/property value sites (variable
    /// declarators, class properties, enum members, object property values).
    pub(crate) fn prepend_rhs_comments(
        &self,
        value_doc: DocId,
        start: u32,
        value_start: u32,
    ) -> DocId {
        match self.build_rhs_comments_opt(start, value_start) {
            Some(comments_doc) => self.d().concat(&[comments_doc, value_doc]),
            None => value_doc,
        }
    }

    /// Build the `= value` RHS for an initializer whose `=`→value gap
    /// (`eq_pos + 1 .. value_start`) holds a comment that forces break handling,
    /// or `None` when the caller should emit its normal inline `= value` form (no
    /// comment, or a single inline block that glues to the value). The returned doc
    /// begins at `" ="`; the caller emits the LHS (name/pattern) before it.
    /// `build_value` is called only when a break is forced, so a comment-free
    /// initializer never pays to build the value doc here.
    ///
    /// Shared by variable declarators and for-loop init clauses so both place a
    /// comment after `=` identically:
    ///
    /// - **Line comment** after `=`: mandatory break after `=`. A comment on the
    ///   `=`'s line trails it inline; a comment on its own line leads the value on
    ///   its own line (author blank lines preserved). Diverges from prettier, which
    ///   relocates the line comment to trail the whole statement — tsv preserves the
    ///   author's placement (see [`conformance_prettier.md` §Comment relocation]).
    /// - **Own-line / multiline block** after `=`: break-after-operator hang, the
    ///   comment on its own line (matches prettier's `hasLeadingOwnLineComment`).
    /// - **Inline block** glued to `=`, or no comment: `None` — the caller keeps the
    ///   value on the `=` line (`= /* c */ value`).
    pub(crate) fn build_eq_comment_break_rhs(
        &self,
        eq_pos: u32,
        value_start: u32,
        build_value: impl FnOnce() -> DocId,
    ) -> Option<DocId> {
        let d = self.d();
        if !self.has_comments_between(eq_pos + 1, value_start) {
            return None;
        }
        if self.has_line_comments_between(eq_pos + 1, value_start) {
            // Line comment → mandatory break. Partition the run: a comment on the
            // `=`'s line trails it; the rest lead the value on their own lines.
            let after_eq: CommentVec<'_> =
                comments_in_range(self.comments, eq_pos + 1, value_start).collect();
            let mut trailing = DocBuf::new();
            let mut leading = DocBuf::new();
            for (ci, comment) in after_eq.iter().enumerate() {
                if self.is_same_line(eq_pos, comment.span.start) {
                    trailing.push(d.text(" "));
                    trailing.push(self.build_comment_doc(comment));
                } else {
                    leading.push(self.build_comment_doc(comment));
                    // Preserve an author blank line before the next comment / value.
                    let next = after_eq.get(ci + 1).map_or(value_start, |c| c.span.start);
                    self.push_blank_preserving_hardline(&mut leading, comment.span.end, next);
                }
            }
            Some(d.concat(&[
                d.text(" ="),
                d.concat(&trailing),
                d.indent(d.concat(&[d.hardline(), d.concat(&leading), build_value()])),
            ]))
        } else if comments_in_range(self.comments, eq_pos + 1, value_start)
            .any(|c| self.comment_forces_own_line(c))
        {
            // Own-line / multiline block → break-after-operator hang.
            let comments_doc = self
                .build_rhs_comments_opt(eq_pos + 1, value_start)
                .unwrap_or_else(|| d.empty());
            Some(d.concat(&[
                d.text(" ="),
                layout::hang_after_operator(d, d.concat(&[comments_doc, build_value()])),
            ]))
        } else {
            // Only an inline block glued to `=`: caller emits `= /* c */ value`.
            None
        }
    }
}
