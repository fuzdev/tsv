// Restricted productions: `return` / `throw` / `yield` and the ASI-safe parens
// their argument needs.
//
// `ReturnStatement : return [no LineTerminator here] Expression ;` (and the same for
// `throw` / `yield`) means a break between the keyword and its argument is not a layout
// choice — ASI would terminate the statement and change the program. So an argument that
// must start on its own line (an own-line comment ahead of it, a binaryish operand that
// breaks) is wrapped in parens that HOLD it to the keyword's line, and prettier's
// `printReturnOrThrowArgument` is the shape mirrored here.
//
// `yield` reaches the same helpers from `expressions/operators.rs` — it is the same
// production asking the same question, and answering it twice would let the two drift.

use super::Printer;
use crate::ast::internal::{self, Expression};
use crate::printer::expressions::operators::SeqLayout;
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{
    find_char_skipping_comments, has_newline_after_position, rfind_char_skipping_comments,
};

/// What a leading-comment gap opens at — the axis that decides whether a comment inside it
/// can be a *trailing* comment of something else instead of leading the node at the far end.
/// Naming it keeps the two cases from being one `u32` a caller can pass the wrong way:
/// reading `Keyword` where a node ends (or vice versa) flips
/// [`Printer::has_leading_own_line_comment_in_range`], and for `return`/`throw` that is an
/// ASI bug, not a layout nit.
#[derive(Clone, Copy)]
enum GapStart {
    /// A keyword, not a node (`return` / `throw`). Nothing here can own a trailing comment,
    /// so every comment in the gap leads the node at the far end.
    Keyword(u32),
    /// A node ends here. A comment sharing its line trails *it*, not the node at the far end.
    Node(u32),
}

impl GapStart {
    /// Where the gap begins — the same position either way; only the reading differs.
    const fn position(self) -> u32 {
        match self {
            Self::Keyword(p) | Self::Node(p) => p,
        }
    }
}

impl<'a> Printer<'a> {
    /// Shared dispatch for return/throw argument formatting.
    ///
    /// Matches Prettier's `printReturnOrThrowArgument` (function.js:231-277):
    /// 1. Assignment expressions → unconditional parens: `return (a = b);`
    /// 2. Own-line comments in chain → unconditional parens
    /// 3. Binaryish arguments → conditional parens (ifBreak)
    /// 4. Otherwise → plain `keyword expr;`
    pub(in crate::printer::statements) fn build_keyword_argument_doc(
        &self,
        keyword: &'static str,
        keyword_start: u32,
        span_end: u32,
        arg: &Expression<'_>,
    ) -> DocId {
        let d = self.d();

        let keyword_end = keyword_start + keyword.len() as u32;

        // `return` / `throw` argument — both are `ancestorNameMap` value positions.
        self.mark_ternary_extra_indent(arg);

        // A comment that must break takes the parenthesized form, which is what makes the
        // break legal; there the comment keeps the line the author gave it. That layout
        // owns its own trailing gap (the parens are retained, so the `)` — not the span
        // end — divides inside from outside), so it returns before the shared scan below.
        //
        // A `//` the author wrote inside the operand's own grouping parens forces the same
        // form, with nothing leading it: the terminator gap below would carry it past the
        // `)` and the `;`, onto a line that may already hold a `//` — and two `//`s sharing
        // an output line MERGE into one (the second delimiter becomes text). The binaryish
        // and sequence arms already keep such a comment inside their parens
        // (`keep_operand_line_inline`); asking here is what makes every operand answer alike.
        if self.argument_has_own_line_comment(keyword_start, arg)
            || (!self.operand_arm_keeps_line_comment_inside(keyword, arg)
                && self.operand_parens_hold_line_comment(arg.span().end, span_end))
        {
            return self.build_comment_paren_doc(keyword, keyword_end, arg, span_end);
        }

        // Trailing comments from stripped grouping parens: `return (x /* c */)` → `return x /* c */;`
        let argument_end = arg.span().end;
        let has_trailing_comments = self.has_comments_to_emit_between(argument_end, span_end);

        // Every remaining comment is glued to the keyword with the value after it on some
        // line, so the value is pulled up onto the comment's line (`return /* c */⏎(v)` →
        // `return /* c */ v`) rather than keeping the author's break: a break between
        // `return`/`throw` and its argument is ASI, not layout — these are restricted
        // productions. (The bare `keyword /* c */⏎value` cannot reach here at all: ASI
        // splits it at parse, so there would be no argument.) The parenthesized branches
        // below may still break, because they emit the parens that survive it.
        let inline_comments = self.build_rhs_comments_glued_opt(keyword_end, arg.span().start);

        // Assignment expressions need parentheses for clarity: return (a = b);
        // Comments go BEFORE the parens: return /* comment */ (a = b);
        // Matches Prettier's behavior for both return and throw.
        // Note: own-line comment check above takes priority — when there's a line
        // comment, the whole thing wraps in outer parens with build_comment_paren_doc
        // (which adds inner assignment parens separately).
        if matches!(arg, Expression::AssignmentExpression(_)) {
            let expr_doc = self.build_expression_doc(arg);
            let mut parts: DocBuf = if let Some(comments_doc) = inline_comments {
                smallvec![
                    d.text(keyword),
                    d.text(" "),
                    comments_doc,
                    d.text("("),
                    expr_doc,
                ]
            } else {
                smallvec![d.text(keyword), d.text(" ("), expr_doc]
            };
            // Trailing comments in the operand→`;` gap were previously DROPPED here.
            // A line comment trails after the `;` in both keywords (`(a = b); // c`).
            // A same-line block comment differs (prettier is inconsistent between the
            // two): `return` keeps it INSIDE the parens (`return (a = b /* c */);`,
            // #19263 — operand-attached), `throw` sends it past the `;`
            // (`throw (a = b); /* c */`) exactly as its own bare operand does
            // (`throw a; /* c */`).
            //
            // ⚠️ Both arms take the terminator split, and the keyword is exactly its
            // `operand_parens_printed` argument — "does the pair printed just below
            // ENCLOSE this comment?", which decides whether the split leaves the comment
            // inside `parts` or hands it back to follow the `;`. Floating `throw`'s
            // comment out to just after the `)` instead gave the statement a
            // SECOND fixed point: `throw (a = b) /* c */;` reproduced itself, while the
            // already-normalized `throw (a = b); /* c */` reproduced *itself*, so one
            // source had two answers and F1 saw neither as wrong. That form was prettier's
            // pass 1, not its fixed point.
            let after = if has_trailing_comments {
                self.split_terminator_gap_comments(
                    &mut parts,
                    argument_end,
                    span_end,
                    false,
                    keyword == "return",
                )
            } else {
                DocBuf::new()
            };
            parts.push(d.text(")"));
            parts.push(d.text(";"));
            parts.extend(after);
            return d.concat(&parts);
        }

        // Sequence operand: `return (a, b)`. In `return` (a value position) a trailing
        // comment stays INSIDE the parens (`return (a, b /* c */);`, prettier #19263),
        // built via the value-position sequence printer. `throw` floats it out, so it
        // falls through to the generic path (which uses the default `build_sequence_doc`).
        if keyword == "return"
            && let Expression::SequenceExpression(seq) = arg
        {
            // The grouping `)` sits outside `seq.span` (the parens aren't part of the
            // node); a trailing comment before it stays inside the parens. Scan to the
            // OUTERMOST `)`, since every redundant shell collapses into the single pair
            // `seq_doc` emits ([`Printer::collapsed_grouping_close`]) — stopping at the
            // first sent a doubly-shelled argument's comment out to the terminator gap
            // (`return ((a, b) /* c */);` → `return (a, b) /* c */;`, then past the `;`
            // on the next pass), where the single-shell form keeps it inside.
            let grouping_close = self
                .collapsed_grouping_close(argument_end, span_end)
                .unwrap_or(argument_end);
            let seq_doc = self.build_sequence_doc_value(seq, grouping_close, SeqLayout::Hanging);
            let mut parts: DocBuf = if let Some(comments_doc) = inline_comments {
                smallvec![d.text(keyword), d.text(" "), comments_doc, seq_doc]
            } else {
                smallvec![d.text(keyword), d.text(" "), seq_doc]
            };
            // Any comment AFTER the grouping `)` (before the `;`) trails after the `;`;
            // the in-paren comment is already inside `seq_doc`.
            let after_start = grouping_close.saturating_add(1).min(span_end);
            let after = if self.has_comments_to_emit_between(after_start, span_end) {
                self.split_terminator_gap_comments(&mut parts, after_start, span_end, false, true)
            } else {
                DocBuf::new()
            };
            parts.push(d.text(";"));
            parts.extend(after);
            return d.concat(&parts);
        }

        if let Expression::BinaryExpression(binary) = arg {
            return self.build_binary_paren_doc(keyword, binary, span_end, inline_comments);
        }

        // Ternary in return/throw: binary test expressions need continuation indent.
        // Matches Prettier's shouldNotIndent (binaryish.js:109-113) — when the binary's
        // grandparent is ReturnStatement/ThrowStatement, shouldNotIndent = false.
        let expr_doc = if let Expression::ConditionalExpression(cond) = arg {
            self.build_conditional_doc_with_binary_test_indent(cond)
        } else if let Expression::SequenceExpression(seq) = arg {
            // `throw` — the `return` arm above claimed its own sequence. Prettier's
            // `shouldIndentSequenceExpression` covers both keywords, so the operands hang
            // inside the parens; only the trailing-comment side differs, and `throw` floats
            // it out, which is the default paren mode.
            self.build_sequence_doc(seq, SeqLayout::Hanging)
        } else {
            self.build_expression_doc(arg)
        };
        let rhs_doc = if let Some(comments_doc) = inline_comments {
            d.concat(&[comments_doc, expr_doc])
        } else {
            expr_doc
        };

        let mut result_parts = smallvec![d.text(keyword), d.text(" "), rhs_doc];
        let after = if has_trailing_comments {
            self.split_terminator_gap_comments(
                &mut result_parts,
                argument_end,
                span_end,
                false,
                false,
            )
        } else {
            DocBuf::new()
        };
        result_parts.push(d.text(";"));
        result_parts.extend(after);
        d.concat(&result_parts)
    }

    /// Check if a return/throw argument has own-line comments that require
    /// unconditional paren wrapping.
    ///
    /// Matches Prettier's `returnArgumentHasLeadingComment` (function.js:290-318).
    ///
    /// Shared with `build_yield_doc`: `yield`/`yield*` are restricted productions
    /// like `return`/`throw`, so they ask the same question and must not answer it
    /// differently — one question, one predicate.
    pub(in crate::printer) fn argument_has_own_line_comment(
        &self,
        keyword_start: u32,
        arg: &Expression<'_>,
    ) -> bool {
        // Own-line comment before the argument itself (`return (\n// c\nexpr)`).
        if self.has_leading_own_line_comment_in_range(
            GapStart::Keyword(keyword_start),
            arg.span().start,
        ) {
            return true;
        }

        // Walk the left side of chainable expressions checking for own-line comments
        self.chain_has_own_line_comment(arg)
    }

    /// Walk the left side of a chain looking for leading own-line comments.
    ///
    /// Mirrors Prettier's `hasNakedLeftSide` + `getLeftSide` walk with
    /// `hasLeadingOwnLineComment` check at each node. Only counts comments
    /// that are on their own line (not trailing comments on the same line
    /// as the preceding expression).
    fn chain_has_own_line_comment(&self, expr: &Expression<'_>) -> bool {
        match expr {
            Expression::CallExpression(call) => self.chain_has_own_line_comment(call.callee),
            Expression::MemberExpression(member) => {
                // Leading own-line comment between object and property.
                let obj_end = member.object.span().end;
                let prop_start = member.property.span().start;
                if self.has_leading_own_line_comment_in_range(GapStart::Node(obj_end), prop_start) {
                    return true;
                }
                self.chain_has_own_line_comment(member.object)
            }
            Expression::TSNonNullExpression(non_null) => {
                self.chain_has_own_line_comment(non_null.expression)
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.chain_has_own_line_comment(tagged.tag)
            }
            _ => false,
        }
    }

    /// Whether a comment in the gap *leads* the node at `end` and is followed by a newline —
    /// Prettier's `hasLeadingOwnLineComment` (`utils/index.js`). Two terms, and both are
    /// load-bearing:
    ///
    /// - **Leads.** Decided by `gap_start`: a comment sharing a preceding *node*'s line
    ///   trails that node rather than leading the next one (Prettier attaches it as a
    ///   trailing comment), so it never counts — `return foo() // c` + `.bar` keeps the
    ///   chain bare.
    /// - **Followed by a newline.** This is what makes a break unavoidable: the node cannot
    ///   share the comment's line, so the caller must emit the form that survives one. A
    ///   block comment with code after it on the same line (`return /* c */ (x)`) fails this
    ///   term and stays inline.
    ///
    /// For `return`/`throw` the second term is an ASI guard, not cosmetics. Both are
    /// restricted productions (`return [no LineTerminator here] Expression`), so putting the
    /// argument on a later line without parens *changes the program*: `return` silently
    /// becomes `return;` plus an unreachable statement, and `throw` becomes a syntax error.
    fn has_leading_own_line_comment_in_range(&self, gap_start: GapStart, end: u32) -> bool {
        self.comments_in_source_between(gap_start.position(), end)
            .any(|c| {
                let leads = match gap_start {
                    GapStart::Keyword(_) => true,
                    GapStart::Node(prev_end) => !self.is_same_line(prev_end, c.span.start),
                };
                leads && has_newline_after_position(self.source, c.span.end)
            })
    }

    /// The shared paren-wrapped operand layout for the three **restricted productions**
    /// (`return` / `throw` / `yield` / `yield*`): `kw (⏎ body⏎)`, emitted when a comment
    /// forces the operand's break — the break is legal only inside the parens (each is
    /// `kw [no LineTerminator here] operand`, so a bare newline is ASI, which would end
    /// the production and silently drop the operand).
    ///
    /// A `//` LINE comment authored on the SAME line as the grouping `(` stays trailing
    /// the `(` (`kw ( // c⏎ …`) rather than dropping to its own line — the comment-position
    /// philosophy (a same-line `//` is a deliberate authoring choice), where prettier
    /// relocates it. A block comment, or any comment on its own line below `(`, keeps the
    /// own-line placement (matching prettier). The operand renders BARE — a sequence must
    /// not self-parenthesize (`build_sequence_doc` would give `(a, b)`) and an assignment
    /// needs no inner `(a = b)`; the hanging parens ARE the grouping, so an inner pair
    /// would double it. A comment before the `)` stays inside the parens where it was
    /// written.
    ///
    /// Returns the hanging doc and the grouping `)` boundary. A statement caller
    /// ([`Self::build_comment_paren_doc`]) uses the boundary to place a comment authored
    /// between the `)` and the `;`; the `yield` expression discards it (its `span_end` is
    /// the `)`, so nothing follows).
    pub(in crate::printer) fn build_restricted_production_paren_doc(
        &self,
        keyword: &'static str,
        keyword_end: u32,
        arg: &Expression<'_>,
        span_end: u32,
    ) -> (DocId, u32) {
        let d = self.d();
        let arg_start = arg.span().start;

        // The opening-delimiter rule at the hang's `(`: a `//` the author glued to it keeps
        // that line ([`Printer::split_located_paren_glued_run`]), as at every other opening
        // delimiter; the rest of the run leads the argument below. The doc carries its own
        // leading space.
        let open_paren = find_char_skipping_comments(
            self.source.as_bytes(),
            keyword_end as usize,
            arg_start as usize,
            b'(',
        )
        .map(|p| p as u32);
        let (paren_trailing, leading_start) =
            self.split_located_paren_glued_run(keyword_end, open_paren, arg_start);
        let inline_comments = self.build_rhs_comments_opt(leading_start, arg_start);

        // Rule: an own-line directive in the grouping `(`→operand gap freezes the operand
        // WHOLE. The slice is the operand's node span, so the grouping `)` this layout
        // supplies stays parent-owned — and it is emitted BARE for the same reason the
        // ordinary path renders a sequence bare here: the hanging parens ARE the grouping,
        // so re-synthesizing the sequence's own pair would double it.
        let paren_gap = open_paren.map_or(keyword_end, |p| p + 1);
        let expr_doc = match self.value_head_frozen_span(paren_gap, arg.span()) {
            Some(frozen) => self.build_frozen_node_doc(frozen),
            None => match arg {
                Expression::SequenceExpression(seq) => self.build_sequence_doc_bare(seq),
                _ => self.build_expression_doc(arg),
            },
        };
        let mut body = DocBuf::new();
        if let Some(comments_doc) = inline_comments {
            body.push(comments_doc);
        }
        body.push(expr_doc);

        // The grouping `)` — not the statement/expression end — bounds what is *inside* the
        // parens. A comment before the `)` stays inside them, where it was written.
        let argument_end = arg.span().end;
        let boundary = self
            .retained_grouping_close(argument_end, span_end)
            .unwrap_or(argument_end);
        if self.has_comments_to_emit_between(argument_end, boundary) {
            self.append_trailing_paren_comments(&mut body, argument_end, boundary);
        }

        (
            self.build_hanging_paren_doc(keyword, d.concat(&body), paren_trailing),
            boundary,
        )
    }

    /// Build unconditional paren-wrapped doc for a return/throw operand with own-line
    /// comments, terminated by `;`. Wraps the shared
    /// [`Self::build_restricted_production_paren_doc`] layout and appends the statement
    /// `;`, placing a comment authored in the `)`→`;` gap between the two.
    fn build_comment_paren_doc(
        &self,
        keyword: &'static str,
        keyword_end: u32,
        arg: &Expression<'_>,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let (hanging, boundary) =
            self.build_restricted_production_paren_doc(keyword, keyword_end, arg, span_end);
        let mut parts: DocBuf = smallvec![hanging];
        let after = if self.has_comments_to_emit_between(boundary, span_end) {
            self.split_terminator_gap_comments(&mut parts, boundary, span_end, false, true)
        } else {
            DocBuf::new()
        };
        parts.push(d.text(";"));
        parts.extend(after);
        d.concat(&parts)
    }

    /// Where a *retained* grouping paren around a statement's operand closes, if the
    /// source has one at all — the boundary between what prints inside those parens and
    /// what trails the `;`.
    ///
    /// It is the **last** `)` before the `;`, not the first. Everything between the
    /// operand's end and the `;` is closing parens, comments and whitespace, so the
    /// outermost wrapper is the one that closes last. Taking the first would misread a
    /// paren *this printer itself adds*: the assignment clarity parens put a `)` before
    /// the comment on the second pass (`return (⏎(x = y) /* t */⏎);`), the comment would
    /// then read as outside the group, and it would float one line further out on every
    /// pass. The scan skips comments because a `)` may sit inside one.
    ///
    /// `None` means no paren was authored — an own-line comment inside a chain forces the
    /// break with none present — so there is no inside to speak of, and the caller uses the
    /// operand's end as the boundary instead.
    ///
    /// The returned position doubles as both bounds: the `)` byte itself can't start a
    /// comment, so the in-paren scan (end-inclusive) and the past-paren scan
    /// (start-inclusive) split the gap at it without overlapping.
    /// Whether the operand's own arm below already keeps a `//` inside the parens it
    /// prints, so the hanging form must not preempt it.
    ///
    /// Two do. The **binaryish** arm renders its operand inside `ifBreak` parens and sets
    /// `keep_operand_line_inline`; the **`return` sequence** arm builds through the
    /// value-position sequence printer, which takes the grouping `)` and emits the comment
    /// inside it — and breaks per operand, a layout the hanging form does not reproduce.
    /// `throw`'s sequence is not one of them: it falls through to the generic arm, whose
    /// `build_sequence_doc` pair the terminator gap floats the comment past.
    ///
    // TODO: the two sequence layouts disagree — per operand here, one line inside the
    // hanging parens (every keyword, including `return` once a *leading* own-line comment
    // forces that form). One construct, one question, two answers; worth a verdict.
    fn operand_arm_keeps_line_comment_inside(
        &self,
        keyword: &'static str,
        arg: &Expression<'_>,
    ) -> bool {
        match arg {
            Expression::BinaryExpression(_) => true,
            Expression::SequenceExpression(_) => keyword == "return",
            _ => false,
        }
    }

    /// Whether the operand's authored grouping parens hold a `//` — the trailing-edge
    /// question the terminator gap cannot answer.
    ///
    /// A line comment ends its output line, so carrying it past the `)` welds it to
    /// whatever already trails the statement; keeping it inside the parens is what makes
    /// the two survive as two. The paren must be **authored** — with no grouping shell
    /// there is nothing to keep the comment inside, and the terminator gap is its only
    /// home (`return x // c⏎;` → `return x; // c`).
    pub(in crate::printer) fn operand_parens_hold_line_comment(
        &self,
        argument_end: u32,
        span_end: u32,
    ) -> bool {
        self.retained_grouping_close(argument_end, span_end)
            .is_some_and(|close| self.has_line_comments_between(argument_end, close))
    }

    pub(in crate::printer) fn retained_grouping_close(
        &self,
        argument_end: u32,
        span_end: u32,
    ) -> Option<u32> {
        rfind_char_skipping_comments(
            self.source.as_bytes(),
            argument_end as usize,
            span_end as usize,
            b')',
        )
        .map(|close| close as u32)
    }

    /// The paren-wrapped layout a comment-forced break takes: `kw (⏎\tbody⏎close`.
    ///
    /// Shared by the three **restricted productions** — `return`/`throw` (via
    /// [`Self::build_comment_paren_doc`]) and `yield`/`yield*` (via
    /// `build_yield_doc`). All three are `kw [no LineTerminator here] operand`, so a
    /// break between the keyword and its operand is ASI, not layout; the parens are
    /// what make the author's break legal.
    ///
    /// The layout closes at the `)`. A statement's `;` is appended by its caller rather
    /// than folded in here, because a comment authored between the `)` and the `;` prints
    /// in that gap and so has to be emitted between the two.
    ///
    /// `body` is the already-assembled operand doc, including any leading comment
    /// run and any trailing comment that stays inside the parens.
    ///
    /// `paren_trailing` is a `//` line comment the author put on the `(` line — it trails
    /// the `(` (`kw ( // c⏎ …`) instead of leading `body` on its own line (the
    /// comment-position divergence; see [`Self::build_comment_paren_doc`]). `None` keeps
    /// the plain `kw (` open.
    pub(in crate::printer) fn build_hanging_paren_doc(
        &self,
        keyword: &'static str,
        body: DocId,
        paren_trailing: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let open = match paren_trailing {
            // `kw ( // c` — the line comment runs to end-of-line; the indent's hardline
            // supplies the break, so nothing is swallowed. The space is the split's.
            Some(comment) => d.concat(&[d.text(keyword), d.text(" ("), comment]),
            None => d.concat(&[d.text(keyword), d.text(" (")]),
        };
        d.concat(&[open, d.indent_hardline(body), d.hardline(), d.text(")")])
    }

    /// Shared logic for return/throw with binaryish arguments.
    ///
    /// Matches Prettier's `printReturnOrThrowArgument` (function.js:240-252):
    /// when the argument is `isBinaryish`, wraps in `ifBreak("(")...ifBreak(")")`.
    ///
    /// When the expression contains hardlines (multi-line callbacks, block bodies,
    /// object literals), the group is forced to break so `ifBreak` produces parens.
    /// This matches Prettier's `propagateBreaks` preprocessing which cascades
    /// `breakParent` (bundled with every `hardline`) up through all ancestor groups.
    /// Our renderer's `will_break` can't see through `IfBreak` nodes, so we detect
    /// hardlines in the expression doc and force the group to break explicitly.
    fn build_binary_paren_doc(
        &self,
        keyword: &'static str,
        binary: &internal::BinaryExpression<'_>,
        span_end: u32,
        inline_comments: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let raw_expr_doc = self.build_binary_chain_doc_ungrouped(binary);
        let expr_doc = if let Some(comments_doc) = inline_comments {
            d.concat(&[comments_doc, raw_expr_doc])
        } else {
            raw_expr_doc
        };

        // Find trailing comments between expression end and semicolon. The scan
        // skips comments so a `;` inside one (`a + b /* ; */ /* c */;`) isn't
        // mistaken for the statement's terminator, which would drop the comments
        // after it. Bounded by `span_end` (the statement's own end): under ASI
        // there is no `;` within the statement, so the scan must not wander past
        // it into the enclosing source and find a later terminator (the object
        // literal's `};`, the next statement's `;`) — that would pull the
        // statement's own trailing comment into this gap AND leave it for the
        // block's trailing-comment emitter too, printing it twice.
        let expr_end = binary.span.end;
        let semicolon_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            expr_end as usize,
            span_end as usize,
            b';',
        )
        .map_or(expr_end, |p| p as u32);

        // Split the trailing comments: an operand-attached block (inside stripped
        // parens, `return (a + b /* c */);`) stays inside the parens before the `;`,
        // while a statement-trailing comment trails *after* the `;` (prettier 3.9:
        // `return a + b; /* c */`). An operand-attached *line* comment
        // (`return (a && b // c\n);`) likewise stays inside the parens — it forces the
        // break so it never lands on the flat `expr // c;` path. See
        // `split_terminator_gap_comments`.
        // Axis-free: the rule looks only at LINE comments, and ownership binds only a block
        // comment (`owned ⇒ is_block`), so skipping and counting give the same answer.
        let has_operand_line_comment =
            comments_to_emit_in_range(self.comments, expr_end, semicolon_pos)
                .any(|c| !c.is_block && self.gap_has_close_paren(c.span.end, semicolon_pos));
        let mut inline_trailing = DocBuf::new();
        let after_semi = self.split_terminator_gap_comments(
            &mut inline_trailing,
            expr_end,
            semicolon_pos,
            true,
            true,
        );
        let trailing_comments_doc = d.concat(&inline_trailing);

        // When the expression contains hardlines (e.g., multi-line callback in a
        // chain), the group must break to produce parens. In Prettier, hardline
        // includes breakParent which propagateBreaks cascades up. Our will_break
        // can't see through IfBreak, so we check the expression doc directly. An
        // operand-attached line comment must also break (it sits inside the parens).
        let force_break = d.will_break(expr_doc) || has_operand_line_comment;

        // Broken: keyword (\n  expr\n);
        // Flat: keyword expr;
        // The trailing-comment doc is `empty()` when the terminator gap has no comment
        // (the common case) — omit it so neither `if_break` branch (both are materialized)
        // carries a wasted empty child. Byte-identical: an empty child renders to nothing.
        let (broken_doc, flat_doc) = if inline_trailing.is_empty() {
            (
                d.concat(&[
                    d.text(" ("),
                    d.indent(d.concat(&[d.softline(), d.group(expr_doc)])),
                    d.softline(),
                    d.text(")"),
                ]),
                d.concat(&[d.text(" "), expr_doc]),
            )
        } else {
            (
                d.concat(&[
                    d.text(" ("),
                    d.indent(d.concat(&[d.softline(), d.group(expr_doc), trailing_comments_doc])),
                    d.softline(),
                    d.text(")"),
                ]),
                d.concat(&[d.text(" "), expr_doc, trailing_comments_doc]),
            )
        };

        let mut inner_parts: DocBuf = smallvec![
            d.text(keyword),
            d.if_break(broken_doc, flat_doc),
            d.text(";"),
        ];
        inner_parts.extend(after_semi);
        let inner = d.concat(&inner_parts);

        if force_break {
            d.group_break(inner)
        } else {
            d.group(inner)
        }
    }
}
