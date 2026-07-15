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
// - **owned.rs**: The comment/paren binding seam — a comment glued to the token
//   after it is printed by the node that token begins, so a synthesized paren
//   can't land between the two (`Comment::owned_by_node`).
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
mod owned;
mod paren;
mod render;
mod scan;

pub(crate) use declarations::HeritageKeyword;

// Re-export for submodules to use `super::X` instead of `super::super::X`.
pub(super) use super::{Printer, calls, layout};

use smallvec::SmallVec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::{Comment, comments_to_emit_in_range};

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
/// `build_comment_doc` per comment, and a `line`/`hardline` toward the next
/// comment (or the terminal) for every comment that doesn't hug — so only this
/// glue test varies, and [`push_leading_comment_run`](Printer::push_leading_comment_run)
/// takes it as a mode.
#[derive(Debug, Clone, Copy)]
pub(crate) enum LeadingGlue {
    /// A block hugs when it shares a source line with whatever follows it — the
    /// *next* comment, or the terminal for the last one. Prettier's rule: its
    /// `printLeadingComment` reads only the source right after the comment's `*/`
    /// (`hasNewline(text, locEnd(comment))`), never where the terminal starts, so
    /// a run the author glued together stays glued (`/* a */ /* b */⏎X` keeps the
    /// pair on one line and breaks before `X`).
    Adjacent,
    /// `Adjacent`, plus a single-line block glued to the operator hugs the value
    /// across a source newline — prettier's assignment/call pull-up
    /// ([`build_rhs_comments_glued_opt`](Printer::build_rhs_comments_glued_opt)).
    AdjacentGlued,
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

    /// Whether a *block* comment is glued to what follows it at `next` (`/* c */ X` —
    /// nothing but spaces after its `*/`), so it leads that token inline instead of
    /// taking its own line. Prettier's leading-comment rule, and the reason it is
    /// keyed on `next` rather than on the item the run leads: `printLeadingComment`
    /// reads only the source right after the comment (`hasNewline(text,
    /// locEnd(comment))`), so a run the author glued together stays glued
    /// (`/* a */ /* b */⏎X` → the pair shares a line, `X` starts a new one).
    ///
    /// The single statement of the rule. [`push_leading_comment_run`](Self::push_leading_comment_run)
    /// is the emitter for the sites whose surrounding loop is the shared one; a site
    /// whose separator policy genuinely differs (the union member's own-line run,
    /// which brackets the `| ` separator and preserves blanks in different positions)
    /// calls this directly rather than re-deriving it.
    pub(crate) fn comment_hugs_next(&self, comment: &Comment, next: u32) -> bool {
        comment.is_block && self.is_same_line(comment.span.end, next)
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

    /// Emit the whole gap between two comma-separated items when the gap contains a
    /// **line** comment (the forced-break case): the comma, the comments, and the
    /// break to the next item, leaving `parts` positioned to emit that item.
    ///
    /// The gap decomposes at the comma. Block comments before the first line comment
    /// trail the previous item inline (`= 0 /* c */`) and the comma is placed *before*
    /// the first line comment — a line comment runs to EOL, so a comma after it would
    /// be commented out. The first line comment then trails the comma iff it was
    /// authored on the comma's line (`comma_pos` → no intervening newline). Everything
    /// from there is the next item's **leading run**, emitted by the shared
    /// [`push_leading_comment_run`](Self::push_leading_comment_run) toward
    /// `next_start`, which also owns the final break: a block glued to the next item
    /// hugs it (`/* c */ b`), anything else drops to its own line. The break between
    /// the comma's line and the leading run is a bare `hardline` — prettier drops an
    /// author blank line there (it belongs to the item join, not to the run).
    ///
    /// `continuation` is emitted after each own-line break: the variable-declaration
    /// site passes `INDENT` text (its declarators aren't wrapped in `d.indent()`), the
    /// for-init and heritage sites pass an empty doc (their runs are). Shared by the
    /// variable-declarator, for-init, and heritage inter-item sites.
    ///
    /// Callers gate on the gap holding a line comment (`has_line_comments_between`) —
    /// a block-only gap has no forced break and belongs to their own path.
    pub(crate) fn push_inter_item_line_comment_gap(
        &self,
        parts: &mut DocBuf,
        prev_end: u32,
        comma_pos: u32,
        next_start: u32,
        continuation: DocId,
    ) {
        let d = self.d();
        let comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, prev_end, next_start).collect();
        // Everything before the first line comment trails the previous item, and the
        // comma is placed there rather than at its authored offset — a `//` runs to
        // EOL, so a comma after it would be commented out, and any block between the
        // two rides left with it (`a, /* c */ // x` → `a /* c */, // x`, matching
        // prettier). With no line comment (the callers' gate makes that unreachable)
        // this is 0 and the whole run simply leads the next item.
        let first_line_idx = comments.iter().position(|c| !c.is_block).unwrap_or(0);
        for comment in &comments[..first_line_idx] {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        parts.push(d.text(","));
        // The first line comment trails the comma when authored on the comma's line
        // (no newline between); an own-line one starts the leading run below. The
        // `is_block` test keeps this honest without leaning on the callers' gate:
        // only a *line* comment can trail the comma, since a block there would be
        // the caller's block-only path.
        //
        // Comment-adjacency read (real even in canonical mode): an own-line line
        // comment must drop below the comma, not merge into the `line_suffix` run.
        let trails_comma = comments.get(first_line_idx).is_some_and(|c| {
            !c.is_block && !self.comment_has_newline_between(comma_pos, c.span.start)
        });
        let run_start = if trails_comma {
            parts.push(self.build_trailing_comment_doc(comments[first_line_idx]));
            first_line_idx + 1
        } else {
            first_line_idx
        };
        parts.push(d.hardline());
        parts.push(continuation);
        self.push_leading_comment_run(
            parts,
            comments[run_start..].iter().copied(),
            next_start,
            LeadingGlue::Adjacent,
            continuation,
        );
    }

    /// A block comment after the comma that sits on the comma's own line (no
    /// newline between the comma and the comment) while a newline separates it
    /// from the next item — a **stranded** comment. It trails the comma,
    /// preserving the author's placement, rather than dropping to its own line;
    /// prettier relocates it *before* the comma. Mirrors the call-argument
    /// stranded rule (`calls/arg_comments.rs`). See conformance_prettier.md
    /// §Comment relocation. (A block that instead *hugs* the next item — no
    /// newline before it — leads that item and matches prettier, so it is not
    /// stranded.)
    pub(crate) fn is_stranded_after_comma_block(
        &self,
        comment: &Comment,
        comma_pos: u32,
        next_start: u32,
    ) -> bool {
        comment.is_block
            && !self.has_newline_between(comma_pos, comment.span.start)
            && !self.is_same_line(comment.span.end, next_start)
    }

    /// Emit the **before-comma** block comments in `[start, comma_pos)` trailing
    /// the preceding item (` /* c */`), preserving the author's side of the comma.
    /// The caller pushes the comma after this. Shared by the variable-declarator,
    /// for-init, and heritage inter-item sites; the after-comma counterparts are
    /// [`Self::push_stranded_after_comma_blocks`] (stranded, trails the comma) and
    /// the site's leading run (a block hugging the next item leads it).
    pub(crate) fn push_before_comma_blocks(&self, parts: &mut DocBuf, start: u32, comma_pos: u32) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, start, comma_pos) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
    }

    /// Emit the **stranded** after-comma block comments in `[comma_pos, next_start)`
    /// trailing the comma (` /* c */`), preserving the author's placement. The
    /// caller pushes the comma before this and handles the remaining (non-stranded)
    /// after-comma comments as leading comments on the next item. Shared by the
    /// variable-declarator, for-init, and heritage inter-item sites; see
    /// [`Self::is_stranded_after_comma_block`].
    pub(crate) fn push_stranded_after_comma_blocks(
        &self,
        parts: &mut DocBuf,
        comma_pos: u32,
        next_start: u32,
    ) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, comma_pos, next_start) {
            if self.is_stranded_after_comma_block(comment, comma_pos, next_start) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
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
    /// This is more efficient than `has_comments_to_emit_between` + `build_comments_between`
    /// because it uses a single binary search instead of two.
    pub(crate) fn build_comments_between_filtered_opt(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> Option<DocId> {
        let d = self.d();

        // Check if any comments exist in range (considering filter)
        let has_comments = comments_to_emit_in_range(self.comments, start, end)
            .any(|c| !matches!(filter, CommentFilter::BlockOnly) || c.is_block);

        if !has_comments {
            return None;
        }

        // Build docs for matching comments.
        //
        // A line comment ends its line, so whatever follows it (another comment, or
        // the caller's next token) must start a new line — else two line comments
        // merge onto one (`// c1 // c2` reparses as a single comment: boundary loss)
        // and a trailing line comment swallows the following token. So a `hardline`,
        // not the spacing separator, sits across any line-comment boundary. A block
        // comment keeps the inline spacing.
        let mut parts = DocBuf::new();
        let mut prev_was_line = false;
        let mut first = true;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            // Apply filter
            if matches!(filter, CommentFilter::BlockOnly) && !comment.is_block {
                continue;
            }

            match spacing {
                CommentSpacing::Leading => {
                    // Separator before this comment: the surrounding-indent `hardline`
                    // after a line comment (no leading space — it starts the line),
                    // else the inline leading space.
                    if !first && prev_was_line {
                        parts.push(d.hardline());
                    } else {
                        parts.push(d.text(" "));
                    }
                    parts.push(self.build_comment_doc(comment));
                }
                CommentSpacing::Trailing => {
                    parts.push(self.build_comment_doc(comment));
                    // Separator after this comment (before the next comment / the
                    // caller's token): a line comment forces the following content
                    // onto a new line, a block comment keeps the inline trailing space.
                    if comment.is_block {
                        parts.push(d.text(" "));
                    } else {
                        parts.push(d.hardline());
                    }
                }
                CommentSpacing::None => {
                    if !first && prev_was_line {
                        parts.push(d.hardline());
                    }
                    parts.push(self.build_comment_doc(comment));
                }
            }
            prev_was_line = !comment.is_block;
            first = false;
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
        for comment in comments_to_emit_in_range(self.comments, start, end) {
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
        for comment in comments_to_emit_in_range(self.comments, start, end) {
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
    /// Use this instead of `has_comments_to_emit_between` + `build_inline_comments_between_doc`
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

    /// Build a Doc for inline comments (trailing space), returning `None` if no comments.
    ///
    /// The `_opt` sibling of `build_inline_comments_between_doc_trailing_space`, matching
    /// the ones the other two spacings already have. Callers that push into a parts
    /// buffer want this rather than the `DocId` form: the `empty()` it would otherwise
    /// return is not free — `concat` keeps it as a child slot for the renderer and every
    /// `fits` pass to walk.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_trailing_space_opt(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        self.build_comments_between_filtered_opt(
            start,
            end,
            CommentSpacing::Trailing,
            CommentFilter::All,
        )
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
    /// the author's line break for a glued block (decorators, `await` operands,
    /// object property values, …) stay on the non-gluing `build_rhs_comments_opt`.
    ///
    /// `return`/`throw` arguments pull up here too, but for a stronger reason than
    /// layout: they are restricted productions, so keeping the break would be ASI and
    /// would change the program. See `build_keyword_argument_doc`.
    pub(crate) fn build_rhs_comments_glued_opt(&self, start: u32, end: u32) -> Option<DocId> {
        self.build_leading_comment_run_opt(start, end, LeadingGlue::AdjacentGlued)
    }

    /// Emit a run of leading comments before `terminal_pos` — the value, member,
    /// item, or body the comments lead. Each comment is emitted with
    /// `build_comment_doc`, followed by one of three separators — prettier's
    /// `printLeadingComment` (`src/main/comments/print.js`), which reads only the
    /// source around *this* comment, never where `terminal_pos` is:
    ///
    /// - **space** — no newline after the `*/` (per `glue`): the comment is glued to
    ///   what follows, so it leads it inline (`/* c */ X`). A run the author glued
    ///   together therefore stays glued (`/* a */ /* b */⏎X`).
    /// - **`line`** — a newline after the `*/` but none before the `/*`: soft, so what
    ///   follows pulls up onto the comment's line when the enclosing group fits and
    ///   drops below when it breaks.
    /// - **`hardline`** — a newline on *both* sides (an own-line comment), or any line
    ///   comment (it must break, or it would absorb what follows). Blank-preserving:
    ///   an author blank line before the value / next comment is kept, matching
    ///   prettier everywhere in this "comment before expression" position (RHS of
    ///   `=`/`:`, call args, `return`/`await`, unary operands, …).
    ///
    /// `continuation` is emitted after each break, for a site whose run is not already
    /// inside a `d.indent()` and so must carry explicit `INDENT` text (the
    /// variable-declarator gap); every other site passes `d.empty()`.
    ///
    /// The single leading-comment emitter: every site that puts comments before an
    /// item routes here, so the rule lives once. Behind
    /// [`build_rhs_comments_opt`](Self::build_rhs_comments_opt),
    /// [`build_rhs_comments_glued_opt`](Self::build_rhs_comments_glued_opt), the
    /// arrow-body run, the member-leading sites (interface / intersection members),
    /// and the comma-separated inter-item gaps (declarators, for-init, heritage,
    /// switch cases).
    pub(crate) fn push_leading_comment_run<'c>(
        &self,
        parts: &mut DocBuf,
        comments: impl Iterator<Item = &'c Comment>,
        terminal_pos: u32,
        glue: LeadingGlue,
        continuation: DocId,
    ) {
        let d = self.d();
        let mut comments = comments.peekable();
        while let Some(comment) = comments.next() {
            parts.push(self.build_comment_doc(comment));
            // The next thing after this comment is the following comment, or the
            // terminal (value/member/item/body) for the last one.
            let next = comments.peek().map_or(terminal_pos, |c| c.span.start);
            let hugs = match glue {
                LeadingGlue::Adjacent => self.comment_hugs_next(comment, next),
                // A glued (not own-line) single-line block hugs across a source
                // newline; the same-line-as-next case still hugs as in `Adjacent`.
                LeadingGlue::AdjacentGlued => {
                    comment.is_block
                        && (self.is_same_line(comment.span.end, next)
                            || !self.comment_forces_own_line(comment))
                }
            };
            if hugs {
                // Value (or next comment) shares the `*/` line — keep it glued.
                parts.push(d.text(" "));
            } else if comment.is_block
                && !self.is_own_line_comment(comment)
                && !self.has_blank_line_between(comment.span.end, next)
            {
                // A block with a newline *after* its `*/` but none before its `/*`:
                // prettier's `printLeadingComment` emits a soft `line` here, so what
                // follows pulls up onto the comment's line when the enclosing group
                // fits and drops below when it breaks. An own-line block (newline on
                // both sides) takes the `hardline` branch instead.
                parts.push(d.line());
                parts.push(continuation);
            } else {
                // Line comment, or an own-line block: keep them on separate lines
                // (preserve the author's layout; a line comment must break so it
                // can't absorb the value).
                self.push_blank_preserving_hardline(parts, comment.span.end, next);
                parts.push(continuation);
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
            comments_to_emit_in_range(self.comments, start, end),
            end,
            glue,
            self.d().empty(),
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
        if !self.has_comments_to_emit_between(eq_pos + 1, value_start) {
            return None;
        }
        if self.has_line_comments_between(eq_pos + 1, value_start) {
            // Line comment → mandatory break. Partition the run: a comment on the
            // `=`'s line trails it; the rest lead the value on their own lines.
            let after_eq: CommentVec<'_> =
                comments_to_emit_in_range(self.comments, eq_pos + 1, value_start).collect();
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
        } else if self
            .comments_on_page_between(eq_pos + 1, value_start)
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
