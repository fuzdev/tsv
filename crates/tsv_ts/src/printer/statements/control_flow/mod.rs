// Control flow statement printing for TypeScript
//
// Statement families live in submodules; this mod.rs keeps the helpers they
// share (comment partitioning, keyword/paren comment placement, and the
// condition-group builders used across the statement families).
//
// - if_else.rs: if/else statements and else-clause layout
// - loops/: for / for-in / for-of headers and bodies (for_loop.rs), while, do-while (while_loop.rs)
// - switch.rs: switch statements and case bodies
// - try_jump.rs: try/catch/finally, throw, break/continue, labeled statements

mod if_else;
mod loops;
mod switch;
mod try_jump;

use smallvec::SmallVec;

use crate::ast::internal::{Expression, Statement, UnaryOperator};
use crate::printer::{CommentVec, Printer};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{Comment, comments_to_emit_in_range};

impl<'a> Printer<'a> {
    /// Build a control-flow *body* whose empty block form collapses (`do {} while (cond)`,
    /// C-style `for (…) {}`). The generic `build_statement_doc` dispatch EXPANDS a
    /// statement-position empty block to `{\n}`, so a collapse-context body must build a
    /// `BlockStatement` directly via the collapse path; a non-block body keeps the generic
    /// dispatch (a non-empty block is identical either way — `expand_empty` only affects the
    /// empty case). The `while` handler and `catch` inline their own block builds (extra
    /// close-paren handling / an always-block body), so they don't route through here.
    fn build_collapsing_body_doc(&self, body: &Statement<'_>) -> DocId {
        if let Statement::BlockStatement(block) = body {
            self.build_block_statement_doc(block)
        } else {
            // Non-block body: its container is the control-flow statement
            // itself, never Program/BlockStatement, so a bare string statement
            // here is never directive-prologue eligible.
            self.build_statement_doc(body, false)
        }
    }

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

        for comment in comments_to_emit_in_range(self.comments, prev_end, next_start) {
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

    /// Does a header→body gap's comment run force the body onto its own line?
    ///
    /// Two independent reasons, and the second is easy to miss: a **line** comment must
    /// break wherever it sits (a `//` would otherwise swallow the body), and **any**
    /// comment the author put on its own line must break to keep that line. Only a
    /// block comment trailing the anchor leaves the body free to stay inline.
    ///
    /// The gap emitters answer this by construction once they have partitioned; this is
    /// for the caller that must choose an inline-capable layout *before* building one
    /// ([`Self::build_adjust_clause_with_comments`]).
    fn header_to_body_gap_breaks(&self, gap_start: u32, body_start: u32) -> bool {
        comments_to_emit_in_range(self.comments, gap_start, body_start)
            .any(|comment| !comment.is_block || !self.is_same_line(gap_start, comment.span.start))
    }

    /// Build comments between a keyword and its `(`, preserving position.
    ///
    /// Returns the full keyword→`(` transition (leading space included) for comments
    /// between `keyword_end` and `open_paren`, or `None` when there are none. A block
    /// comment glues with a trailing space (`if /* c */ ` → `if /* c */ (a)`); a line
    /// comment trails the keyword and breaks so the `(` drops to the next line and
    /// can't be swallowed (`if // c` + hardline → `if // c⏎(a)`). The caller pushes a
    /// bare `(` after this (no leading space) — see [`Self::build_keyword_paren_comments`]
    /// call sites. Prettier relocates the comment instead (into the parens for
    /// `if`/`while`/`switch`, past the header for `for`); tsv preserves the authored
    /// position uniformly.
    fn build_keyword_paren_comments(
        &self,
        keyword_end: u32,
        open_paren: Option<u32>,
    ) -> Option<DocId> {
        let op = open_paren?;
        if !self.has_comments_to_emit_between(keyword_end, op) {
            return None;
        }
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments_to_emit_in_range(self.comments, keyword_end, op) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
            // Block: glue with a trailing space so the `(` follows directly. Line: a
            // hardline drops the `(` to the next line so the `//` can't swallow it.
            if comment.is_block {
                parts.push(d.text(" "));
            } else {
                parts.push(d.hardline());
            }
        }
        Some(d.concat(&parts))
    }

    /// Push a control-flow head opener — `keyword`, any `keyword`→`(` comment
    /// (`keyword_comments`, which already carries its own trailing space/break), then
    /// `(`. With no comment it emits a plain `keyword (`. Shared by `if`/`while`/
    /// `switch`/`catch` and the plain-`for` header so the `keyword`→`(` line-comment
    /// break is uniform (`if // c⏎(a)`) rather than swallowing the `(`.
    fn push_keyword_open_paren(
        &self,
        parts: &mut DocBuf,
        keyword: &'static str,
        kc: Option<DocId>,
    ) {
        let d = self.d();
        parts.push(d.text(keyword));
        if let Some(kc) = kc {
            parts.push(kc);
            parts.push(d.text("("));
        } else {
            parts.push(d.text(" ("));
        }
    }

    /// Emit a `}`→continuation-keyword gap: its comments, then the separator before
    /// the keyword. The caller pushes the keyword itself.
    ///
    /// The single place that question is answered, for every keyword that continues a
    /// construct across a `}` — `else`, `catch`, `finally`, and a do-while's `while`.
    /// Comments keep their authored position (trailing stays trailing, own-line keeps
    /// its own line), and the keyword hugs the `}` only when the previous part was a
    /// block, every comment trailed it, and none was a `//` — a line comment there
    /// would swallow the keyword.
    ///
    /// A blank above the first own-line comment **survives** here (`blank_seed` =
    /// `Some`): there is no body `{` to sit below it, so it separates two branches of a
    /// chain and is real authoring intent. Prettier agrees at `else`; at a do-while's
    /// `while` it relocates the comment into the condition parens instead, so it is no
    /// oracle there and tsv's own stance governs. The mirror of
    /// [`Self::push_header_to_body_gap`], which drops it.
    ///
    /// `prev_is_block` is false only for an `if` with a non-block consequent
    /// (`if (a) expr;⏎else …`); a `try`/`catch` block and a do-while body block are
    /// always blocks.
    fn push_block_to_keyword_gap(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        keyword_start: u32,
        prev_is_block: bool,
    ) {
        let d = self.d();
        if !self.has_comments_to_emit_between(gap_start, keyword_start) {
            parts.push(if prev_is_block {
                d.text(" ")
            } else {
                d.hardline()
            });
            return;
        }

        let (inline_prev, own_line, inline_next) =
            self.partition_comments_by_line(gap_start, keyword_start);

        // Merge `inline_next` (comments sharing the keyword's line) into the own-line
        // run so they're emitted before the keyword rather than dropped.
        // e.g. `} \n /* b */ else {` → `}\n/* b */\nelse {`
        let mut all_own_line = own_line;
        all_own_line.extend(inline_next);

        self.build_comments_between_parts(parts, &inline_prev, &all_own_line, Some(gap_start));

        let keyword_hugs_brace =
            prev_is_block && all_own_line.is_empty() && inline_prev.iter().all(|c| c.is_block);
        parts.push(if keyword_hugs_brace {
            d.text(" ")
        } else {
            d.hardline()
        });
    }

    /// Emit a header→body gap: its comments, then the separator before the body. The
    /// caller pushes the anchor token (`)`, or the `try`/`catch`/`finally` keyword)
    /// before this, and the body after.
    ///
    /// The single place that question is answered, for every construct whose body
    /// follows a header — `if` / `while` / `for-in` / `for-of` (via
    /// [`Self::append_close_paren_with_comments`]), the C-style `for` (whose `)` is
    /// already inside its header doc), and `try` / `catch (e)` / bare `catch` /
    /// `finally`. Only the anchor differed between them, which is why it stays the
    /// caller's job.
    ///
    /// Every comment keeps its **authored position** — trailing the anchor stays
    /// trailing, own-line keeps its own line — regardless of kind. A line comment
    /// additionally forces a hardline wherever it sits, so the `//` can't swallow the
    /// body.
    ///
    /// ⚠️ A block comment is **not** flexible here. This emitter used to normalize an
    /// own-line block comment to trail the anchor (`if (a)⏎/* b */⏎{` → `if (a) /* b */ {`)
    /// on the premise that prettier does the same; measured, prettier **preserves** it in
    /// every construct and body kind, and the line-comment path was already preserving.
    /// Own-line-ness is authoring signal, so relocating it also cut against tsv's own
    /// comment-position stance. Pinned by
    /// `syntax/comments/head_body_own_line_block_comment`.
    ///
    /// A blank above the first own-line comment is **dropped** (`blank_seed` = `None`),
    /// so a body block's `{` never sits below one. That is consistent with tsv's own
    /// handling when `{` is on the header line (`if (a) {⏎⏎// c` also collapses), and
    /// prettier drops it here too. The mirror of [`Self::push_block_to_keyword_gap`],
    /// which keeps it.
    ///
    /// **Precondition**: the gap holds at least one comment to emit. Each caller's
    /// no-comment fast path differs (`") "` vs a bare space), so it stays theirs.
    fn push_header_to_body_gap(&self, parts: &mut DocBuf, gap_start: u32, body_start: u32) {
        let d = self.d();
        let (inline_prev, mut own_line, inline_next) =
            self.partition_comments_by_line(gap_start, body_start);

        // `inline_next` (a comment sharing the body's line) is treated the same as
        // own-line — it is not trailing the anchor, so it gets its own line.
        own_line.extend(inline_next);

        self.build_comments_between_parts(parts, &inline_prev, &own_line, None);

        // Anything on its own line already forced a break; otherwise only a `//` does,
        // since it would swallow the body.
        if !own_line.is_empty() || inline_prev.iter().any(|c| !c.is_block) {
            parts.push(d.hardline());
        } else {
            parts.push(d.text(" "));
        }
    }

    /// The header→body gap for a **non-block** body: the comment run, then the body on
    /// its own line, all sharing the body's indent (`if (a)⏎↹// c⏎↹fn();` — prettier's
    /// `adjustClause` shape). The caller pushes its own anchor (`)` / `else`) first.
    ///
    /// ⚠️ **Position-agnostic for a `//` only**, unlike [`Self::push_header_to_body_gap`];
    /// a block comment keeps its authored line here too. For the `)` gap that split is
    /// prettier's, measured on all four combinations:
    ///
    /// | body | authored trailing `)` | authored own-line |
    /// | --- | --- | --- |
    /// | **non-block**, `//` | moved to its own line | own line |
    /// | **non-block**, `/* */` | stays trailing | own line |
    /// | **block**, either | stays trailing | own line |
    ///
    /// So only the non-block `//` normalizes. That is not tsv relocating a comment
    /// against its own stance: a non-block body has no `{` for a trailing `//` to anchor
    /// against, and prettier does the same, so there is nothing to diverge over. Routing
    /// *that* case through the position-preserving emitter regressed
    /// `if/head_body_nonblock_comment`, which pins the trailing authoring normalizing to
    /// the own-line form under **both** formatters.
    ///
    /// A blank line between two own-line comments survives via the shared comment-run
    /// builder ([`Self::push_gap_blank_before`]'s counterpart in `build_comments_between`).
    ///
    /// ⚠️ The **`else`**→non-block gap does NOT share this — see
    /// [`Self::push_indented_else_to_body_gap`], which preserves the authored position.
    fn push_indented_header_to_body_gap(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        body_start: u32,
        body_doc: DocId,
    ) {
        let d = self.d();
        let (anchor_line, mut own_line, inline_next) =
            self.partition_comments_by_line(gap_start, body_start);
        own_line.extend(inline_next);

        // Only a **block** comment can stay on the anchor's line. A line comment authored
        // trailing `)` normalizes to its own line — that is the position-agnostic half
        // this gap is pinned to (`if/head_body_nonblock_comment`), and it is prettier's
        // behavior too: a non-block body has no `{` for a trailing comment to anchor
        // against. Source order survives the split because a comment trailing `)`
        // precedes every own-line one.
        let mut inline_prev: CommentVec<'a> = SmallVec::new();
        let mut run: CommentVec<'a> = SmallVec::new();
        for comment in anchor_line {
            if comment.is_block {
                inline_prev.push(comment);
            } else {
                run.push(comment);
            }
        }
        run.extend(own_line);

        let mut inner = DocBuf::new();
        self.build_comments_between_parts(&mut inner, &inline_prev, &run, None);
        inner.push(d.hardline());
        inner.push(body_doc);
        parts.push(d.indent(d.concat(&inner)));
    }

    /// The `else`→**non-block** body gap: the comment run keeps its **authored
    /// position** and the body is indented beneath (`} else // c⏎↹expr;`).
    ///
    /// ⚠️ Deliberately position-**preserving**, where the `)`→non-block gap
    /// ([`Self::push_indented_header_to_body_gap`]) normalizes. Prettier drops a
    /// trailing comment onto its own line in *both*, so keeping it here is a
    /// **sanctioned divergence** (`if/else_line_comment_nonblock_prettier_divergence`,
    /// with a README and an `output_prettier`), while the `)` gap deliberately matches
    /// prettier (`if/head_body_nonblock_comment`, a plain fixture). Two gaps, one
    /// question, two pinned answers — **do not merge them**; routing `else` through the
    /// normalizing helper silently deletes the sanctioned behavior.
    fn push_indented_else_to_body_gap(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        body_start: u32,
        body_doc: DocId,
    ) {
        let d = self.d();
        let mut inner = DocBuf::new();
        self.push_header_to_body_gap(&mut inner, gap_start, body_start);
        inner.push(body_doc);
        parts.push(d.indent(d.concat(&inner)));
    }

    /// Build docs for comments between statement parts (e.g., between `}` and `else`).
    ///
    /// - `inline_prev` — emitted on the anchor's line, each after a space
    /// - `own_line` — emitted each on its own line; an authored blank line *between* two
    ///   of them always survives (it separates two distinct remarks)
    /// - `blank_seed` — whether an authored blank *above the first* own-line comment
    ///   survives, and the position to measure it from. `Some(pos)` preserves it,
    ///   `None` drops it. The two gaps disagree on exactly this, so it is one value
    ///   rather than a policy flag plus a position that could contradict it
    ///   (see [`Self::push_block_to_keyword_gap`] / [`Self::push_header_to_body_gap`]).
    ///
    /// ⚠️ **The blank scan is raw `has_blank_line_between`, not the
    /// `blank_scan_start`/`blank_scan_end` helpers** that CLAUDE.md §Comment Handling
    /// prescribes for in-source scans — a raw scan cannot tell a comment's own newlines
    /// from an author's blank line. It is sound here *only* because **no comment in
    /// these gaps can be owned**: both `owned_by_node = true` sites live in
    /// `parser/expression.rs` and bind to expression starts, and neither a
    /// `}`→continuation-keyword gap nor a header→body gap contains one — so the
    /// to-emit set and the in-source set coincide and the scan can never straddle a
    /// comment this caller didn't emit. Verified 2026-07-20. If ownership ever extends
    /// to a token these gaps can hold, this must move to the helpers.
    fn build_comments_between_parts(
        &self,
        parts: &mut DocBuf,
        inline_prev: &[&Comment],
        own_line: &[&Comment],
        blank_seed: Option<u32>,
    ) {
        let d = self.d();
        // Trailing comments stay on same line
        for comment in inline_prev {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        // `None` until a comment has been emitted, so the *first* comment's blank is
        // checked only when the gap seeds one; every later blank is always checked.
        let mut prev_end = blank_seed;
        for comment in own_line {
            self.push_gap_blank_before(parts, prev_end, comment.span.start);
            parts.push(d.hardline());
            parts.push(self.build_comment_doc(comment));
            prev_end = Some(comment.span.end);
        }
    }

    /// Preserve an authored blank line before an own-line comment in a control-flow gap:
    /// a `literalline` (an empty, unindented line) ahead of the `hardline` that starts
    /// the comment's own line.
    ///
    /// `prev_end` is `None` for the first comment of a run whose gap drops a leading
    /// blank — every header→body gap, block body or not, so a body never sits below a
    /// blank. Every *subsequent* comment passes the previous comment's end, because a
    /// blank *between* two comments separates two distinct remarks and is always kept
    /// (`conformance_prettier.md` §"No blank above a body block's `{`").
    ///
    /// This is the rule for gap emitters that build their own comment run. A run emitted
    /// through the generic builder (`build_comments_between`) gets the same treatment
    /// there, at its own `hardline` seams — between them the rule is written twice, in
    /// the only two places a control-flow comment run is ever assembled.
    ///
    /// Each emitter still owns its comment **separators**; those legitimately differ, and
    /// one of them (for-in/of keeping a comment trailing `)`) is a sanctioned divergence.
    /// Only the blank rule is shared — re-deriving it per emitter is how `if`/`while`/
    /// for-in/of came to drop the blank while the C-style `for` kept it via a hand-rolled
    /// positional test.
    fn push_gap_blank_before(&self, parts: &mut DocBuf, prev_end: Option<u32>, next_start: u32) {
        if let Some(end) = prev_end
            && self.has_blank_line_between(end, next_start)
        {
            parts.push(self.d().literalline());
        }
    }

    /// Append `)` + comments + `;` for empty statement bodies.
    ///
    /// Handles comments between `)` and `;`:
    /// - Block comments: `if (a) /* comment */ ;`
    /// - Line comments: `if (a) // comment\n;`
    /// - No comments: `if (a);`
    fn append_close_paren_empty_stmt_with_comments(
        &self,
        parts: &mut DocBuf,
        paren_end: u32,
        empty_start: u32,
    ) {
        let d = self.d();
        parts.push(d.text(")"));
        if self.has_comments_to_emit_between(paren_end, empty_start) {
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

    /// Push `)` and the gap that follows it — the `)` anchor for
    /// [`Self::push_header_to_body_gap`], which owns the gap's comment rules. The caller
    /// pushes what comes after (a block body, or `switch`'s `{`).
    ///
    /// Used wherever a `)` is followed by a **block**: `if` / `while` / for-in/for-of
    /// bodies and the `switch` body brace. A **non-block** for-in/for-of body takes
    /// `append_close_paren_with_non_block_body` instead, which also indents.
    fn append_close_paren_with_comments(
        &self,
        parts: &mut DocBuf,
        paren_end: u32,
        body_start: u32,
    ) {
        let d = self.d();
        if self.has_comments_to_emit_between(paren_end, body_start) {
            parts.push(d.text(")"));
            self.push_header_to_body_gap(parts, paren_end, body_start);
        } else {
            parts.push(d.text(") "));
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
        if self.has_comments_to_emit_between(paren_end, body_start) {
            let mut parts: DocBuf = SmallVec::from_slice(head_parts);
            parts.push(d.text(")"));
            if self.header_to_body_gap_breaks(paren_end, body_start) {
                // A `//` (which would swallow the body) or any own-line comment forces
                // the break: stmt (cond)\n\t// comment\n\tfn();
                self.push_indented_header_to_body_gap(&mut parts, paren_end, body_start, body_doc);
                d.concat(&parts)
            } else {
                let comment_doc =
                    self.build_inline_comments_between_doc_no_leading_space(paren_end, body_start);
                // Block comment stays with statement: stmt (cond) /* c */ fn();
                // When broken: stmt (cond)\n\t/* c */ fn();
                parts.push(d.indent(d.concat(&[d.line(), comment_doc, d.text(" "), body_doc])));
                d.group(d.concat(&parts))
            }
        } else {
            let mut parts: DocBuf = SmallVec::from_slice(head_parts);
            parts.push(d.text(")"));
            parts.push(d.indent_line(body_doc));
            d.group(d.concat(&parts))
        }
    }

    /// Prettier's `shouldInlineCondition` (miscellaneous.js): a `!` / `!!`-negated
    /// parenthesized logical condition (`if (!(a || b))`, `while (!!(a && b))`) hugs
    /// the `(` instead of breaking onto its own line, so the whole statement reads
    /// `if (!(` … `)) {` rather than `if (⏎ !(…) ⏎) {`.
    ///
    /// True iff the test is `!X` or `!!X` (but not `!!!X`) where `X` is a *logical*
    /// binary expression. This matches only the `printIfOrWhileConditionOrWithStatementObject`
    /// callers (`if` / `while` / `do-while`), never `switch`. Comments on the condition
    /// disable inlining upstream — the caller only reaches the bare-doc path when the
    /// condition parens hold no comments.
    fn condition_should_inline_negation(&self, test: &Expression<'_>) -> bool {
        let Expression::UnaryExpression(outer) = test else {
            return false;
        };
        if outer.operator != UnaryOperator::Bang {
            return false;
        }
        // Peel one optional inner `!` (so `!` and `!!` qualify; a third `!` leaves a
        // UnaryExpression here and fails the logical-binary check below).
        let inner = match outer.argument {
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::Bang => u.argument,
            other => other,
        };
        matches!(inner, Expression::BinaryExpression(b) if b.operator.is_logical())
    }

    /// Build the condition doc for `if` / `while`, honoring the negation-inline rule.
    ///
    /// Mirrors Prettier's `printIfOrWhileConditionOrWithStatementObject`: when
    /// `condition_should_inline_negation` holds (and the parens carry no comments) the
    /// test doc is emitted bare so `!(…)` hugs `(`; otherwise the standard condition
    /// group wraps it. `switch` and the do-while comment-preservation path build their
    /// condition group directly and are deliberately excluded.
    fn build_statement_condition_doc(
        &self,
        test: &Expression<'_>,
        open_paren: Option<u32>,
        close_paren: Option<u32>,
    ) -> DocId {
        if self.condition_should_inline_negation(test) {
            let no_comments = match (open_paren, close_paren) {
                (Some(open), Some(close)) => !self.has_comments_to_emit_between(open + 1, close),
                _ => true,
            };
            if no_comments {
                return self.build_condition_doc(test);
            }
        }
        match (open_paren, close_paren) {
            (Some(open), Some(close)) => {
                self.build_condition_group_with_comments(test, open, close)
            }
            _ => self.build_condition_group(test),
        }
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
    fn build_condition_group(&self, test_expr: &Expression<'_>) -> DocId {
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
        test_expr: &Expression<'_>,
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
        test_expr: &Expression<'_>,
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
        test_expr: &Expression<'_>,
        open_paren_pos: u32,
        close_paren_pos: u32,
        preserve_inline: bool,
    ) -> DocId {
        let d = self.d();
        let test_start = test_expr.span().start;
        let test_end = test_expr.span().end;

        // Check for comments before and after the condition
        let has_leading = self.has_comments_to_emit_between(open_paren_pos + 1, test_start);
        let has_trailing = self.has_comments_to_emit_between(test_end, close_paren_pos);

        if !has_leading && !has_trailing {
            // No comments - use the standard condition group
            return self.build_condition_group(test_expr);
        }

        // Build with comments
        let test_doc = self.build_condition_doc(test_expr);
        let mut inner_parts = DocBuf::new();

        // Collect leading comments
        // Classification based on position relative to open paren AND condition:
        // - "inline with open paren" = comment STARTS on same line as open paren
        // - "own line" = comment does NOT start on same line as open paren
        let leading_comments: CommentVec<'_> = if has_leading {
            comments_to_emit_in_range(self.comments, open_paren_pos + 1, test_start).collect()
        } else {
            SmallVec::new()
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

            // Own-line comments: exactly one separator *before* each (plus any authored
            // blank), then one before the condition after the last.
            //
            // ⚠️ This used to push a break both after each comment and before the next,
            // which **fabricated a blank line between every pair** — `// c1⏎// c2` came
            // out `// c1⏎⏎// c2`, a blank the author never wrote. It was a stable fixed
            // point, so idempotency never caught it, and prettier agrees with the
            // adjacent form (it is no oracle for a comment after `(` here, but it is for
            // this shape). Pinned by `do_while/condition_own_line_comment_run`.
            let mut prev_end: Option<u32> = None;
            for comment in &leading_comments {
                if self.is_same_line(open_paren_pos, comment.span.start) {
                    continue;
                }
                // The first own-line comment needs no break of its own when an inline
                // comment already ended the line above it.
                if prev_end.is_some() || !has_inline_comment_followed_by_newline {
                    self.push_gap_blank_before(&mut inner_parts, prev_end, comment.span.start);
                    inner_parts.push(d.hardline());
                }
                inner_parts.push(self.build_comment_doc(comment));
                prev_end = Some(comment.span.end);
            }
            if let Some(end) = prev_end {
                if self.is_same_line(end, test_start) {
                    inner_parts.push(d.text(" "));
                } else {
                    inner_parts.push(d.hardline());
                }
            }

            if !has_inline_comment_followed_by_newline && !has_own_line_leading {
                inner_parts.push(d.softline());
            }
        } else {
            // Normalize comments based on their position:
            // - Comments on own line (not same line as open paren): force break with hardline
            // - Comments inline with open paren: allow collapsing with softline
            // `None` for the first comment: a blank directly under `(` is dropped (both
            // formatters), while a blank *between* two comments separates two distinct
            // remarks and survives — the same split the gap emitters make.
            let mut prev_end: Option<u32> = None;
            for comment in &leading_comments {
                if self.is_same_line(open_paren_pos, comment.span.start) {
                    // Comment is inline with open paren - use softline to allow collapse
                    inner_parts.push(d.softline());
                } else {
                    // Comment is on its own line - force break
                    self.push_gap_blank_before(&mut inner_parts, prev_end, comment.span.start);
                    inner_parts.push(d.hardline());
                }
                prev_end = Some(comment.span.end);
                inner_parts.push(self.build_comment_doc(comment));

                // A comment sharing the condition's line is separated from it by a
                // space. Emitted per comment rather than once after the loop because it
                // doubles as the separator between two comments on that same line; the
                // break case needs nothing here, since the next iteration (or the
                // closing separator below) supplies it.
                if self.is_same_line(comment.span.end, test_start) {
                    inner_parts.push(d.text(" "));
                }
            }

            // The separator before the condition. With no comments the group is free to
            // collapse; otherwise the last comment decides — one sharing the condition's
            // line already pushed its space above.
            match leading_comments.last() {
                None => inner_parts.push(d.softline()),
                Some(last) if !self.is_same_line(last.span.end, test_start) => {
                    inner_parts.push(d.hardline());
                }
                Some(_) => {}
            }
        }

        // The condition itself
        inner_parts.push(test_doc);

        // Trailing comments use partition_comments_by_line since the classification matches:
        // inline = starts on same line as test_end (goes to inline_prev)
        // own line = doesn't start on same line as test_end
        let (trailing_inline, trailing_own_line, _) =
            self.partition_comments_by_line(test_end, close_paren_pos);

        // Trailing comments — the shared run builder, which owns the blank rule. An
        // authored blank survives both above the first own-line comment (`blank_seed` =
        // `Some(test_end)`; there is no body `{` here for one to sit below, only the
        // `)`) and between subsequent ones. Prettier keeps both, and tsv's own call-arg
        // and array builders already did; this gap re-derived the run by hand and
        // dropped every blank.
        self.build_comments_between_parts(
            &mut inner_parts,
            &trailing_inline,
            &trailing_own_line,
            Some(test_end),
        );

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

    /// Build a doc for a condition expression (if/while/for test)
    ///
    /// For binary expressions, uses ungrouped version so parent group controls breaking.
    /// Logical operators (`&&`, `||`, `??`) break with the parent condition group.
    /// Non-logical operators (`<`, `===`, etc.) keep a sub-group for independent evaluation
    /// (e.g., `for (i = 0; i < len; i++)` — the `i < len` stays flat).
    /// Assignment expressions get double-parens for clarity: `while ((x = y))`
    fn build_condition_doc(&self, expr: &Expression<'_>) -> DocId {
        let inner = match expr {
            Expression::BinaryExpression(binary) => {
                self.build_binary_chain_doc_ungrouped_condition(binary)
            }
            _ => self.build_expression_doc(expr),
        };
        if self.needs_parens(expr, super::ParenContext::StatementTest) {
            let d = self.d();
            d.parens(inner)
        } else {
            inner
        }
    }
}
