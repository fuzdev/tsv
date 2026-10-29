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
use crate::ast::internal::{self, Expression, ExpressionKind};
use crate::printer::chain::{chain_mid_chain_paren_gaps, find_bracket_position};
use crate::printer::comments::left_spine_shell_is_comment_free;
use crate::printer::expressions::operators::SeqLayout;
use crate::printer::statements::TerminatorGap;
use smallvec::{SmallVec, smallvec};
use tsv_lang::doc::arena::DocId;
use tsv_lang::doc::{DocBuf, GapCommentLine, LeadingBreak};
use tsv_lang::source_scan::{
    find_char_skipping_comments, has_newline_after_position, has_newline_before_position,
    rfind_char_skipping_comments,
};
use tsv_lang::{Span, range_too_narrow_for_a_comment};

/// What a leading-comment gap opens at — the axis that decides whether a comment inside it
/// can be a *trailing* comment of something else instead of leading the node at the far end.
/// Naming it keeps the two cases from being one `u32` a caller can pass the wrong way:
/// reading `Keyword` where a node ends (or vice versa) flips
/// [`Printer::has_leading_own_line_comment_in_range`], and for `return`/`throw` that is an
/// ASI bug, not a layout nit.
#[derive(Clone, Copy)]
enum GapStart {
    /// A keyword, not a node (`return` / `throw` / `yield`). Nothing here can own a trailing
    /// comment, so every comment in the gap leads the node at the far end. This is also the
    /// gap the restricted production's `[no LineTerminator here]` covers, so it is the one
    /// reading that counts a comment's *own* line terminator — see
    /// [`Printer::has_leading_own_line_comment_in_range`].
    Keyword(u32),
    /// A node ends here. A comment sharing its line trails *it*, not the node at the far end
    /// — and so does one after a `)` that opens its line ([`tsv_lang::GapTrail::AfterParen`]).
    Node(u32),
    /// A node's own span start, ahead of its left-side child — the gap holds only the
    /// grouping `(`s the parser stripped from that child (`(⏎// c⏎a as any).b` opens the
    /// member at the `(`, the `as` at `a`). Like [`Self::Keyword`], nothing here can own a
    /// trailing comment: a comment sharing the `(`'s line still leads the child.
    ///
    /// `pair_emitted` is whether the printer RE-EMITS a pair in that gap
    /// ([`Printer::left_shell_paren_is_emitted`]). It does not change which comments LEAD
    /// the child — every one of them does — only whether a comment's **own** interior line
    /// terminator can reach the keyword: a run the gap emits is hoisted out in front of the
    /// pair, but a comment the child OWNS is printed from inside it and never travels.
    Shell { position: u32, pair_emitted: bool },
}

impl GapStart {
    /// Where the gap begins — the same position either way; only the reading differs.
    const fn position(self) -> u32 {
        match self {
            Self::Keyword(p) | Self::Node(p) | Self::Shell { position: p, .. } => p,
        }
    }

    /// Whether a comment's **own** line terminator counts as one in this gap — at
    /// [`Self::Keyword`], the gap a restricted production's `[no LineTerminator here]`
    /// covers, and at a [`Self::Shell`] whose pair the printer does not re-emit, where the
    /// strip puts the run in exactly that gap. The reading, and why [`Self::Node`] keeps
    /// the narrow term, is [`Printer::has_leading_own_line_comment_in_range`].
    const fn counts_interior_line_terminator(self) -> bool {
        matches!(
            self,
            Self::Keyword(_)
                | Self::Shell {
                    pair_emitted: false,
                    ..
                }
        )
    }
}

/// What a restricted production's argument asks of the hanging pair, from the source
/// ([`Printer::argument_hang`]).
pub(in crate::printer) enum ArgumentHang {
    /// No own-line comment the pair answers for.
    Bare,
    /// A comment that ends a line ahead of the argument in every arm — the pair.
    Always,
    /// Own-line comments only in plain member gaps (each named by its property's token
    /// start), which the chain may print glued: the pair only when the arm's doc prints one
    /// of them on a line of its own ([`Printer::argument_gap_hangs`]).
    IfGapPrintsLine(SmallVec<[u32; 2]>),
}

impl<'a> Printer<'a> {
    /// Shared dispatch for return/throw argument formatting.
    ///
    /// Matches Prettier's `printReturnOrThrowArgument` (`print/return-statement.js`):
    /// 1. Assignment expressions → unconditional parens: `return (a = b);`
    /// 2. Own-line comments in chain → unconditional parens
    /// 3. Binaryish arguments → conditional parens (ifBreak)
    /// 4. Otherwise → plain `keyword expr;`
    ///
    /// The source predicate ([`Self::argument_hang`]) chooses the arm; the
    /// arm's own doc is then the authority on ASI ([`Self::argument_leading_break`]): a
    /// line it would end ahead of the argument's first token in every layout takes the
    /// hanging pair, and one it ends only when a group breaks takes a pair that prints
    /// with that break — the binary arm's own, or [`Self::build_asi_soft_pair`]. It is also
    /// the authority on a member gap the predicate only named: the pair is taken when the
    /// doc prints a comment there on a line of its own ([`Self::argument_gap_hangs`]).
    ///
    /// `gap` is the statement container's deferral fact
    /// (`StatementContext::gap`), threaded into every arm's terminator split —
    /// in clause position the gap's own-line run renders dedented to the flushing
    /// construct's level (see [`Printer::split_terminator_gap_comments`]).
    pub(in crate::printer::statements) fn build_keyword_argument_doc(
        &self,
        keyword: &'static str,
        keyword_start: u32,
        span_end: u32,
        arg: &Expression<'_>,
        gap: TerminatorGap,
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
        // and sequence arms keep such a comment inside the parens they print themselves;
        // asking here is what makes every operand answer alike.
        let hang = self.argument_hang(keyword_start, arg);
        if matches!(hang, ArgumentHang::Always)
            || (!self.operand_arm_keeps_line_comment_inside(keyword, arg)
                && self.operand_parens_hold_line_comment(arg.span().end, span_end))
        {
            return self.build_comment_paren_doc(keyword, keyword_end, arg, span_end, gap);
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
        // Whether the arm's doc can print a comment ahead of the argument's first token at
        // all — asked ON PAGE, since a comment the argument owns is invisible to the gap's
        // emit axis and still prints there: one in the gap itself (glued inside the
        // statement's own stripped `(`, `return (/* c */ x)`), or one in the argument's
        // left-spine shell.
        let edge_gate = self.has_comments_on_page_between(keyword_end, arg.span().start)
            || !left_spine_shell_is_comment_free(self.source, arg.span().start);
        // A member gap the source shows an own-line comment in asks for the hanging pair only
        // when the arm's doc prints that comment on a line of its own (`ArgumentHang`).
        let leading_break = |doc: DocId| {
            if self.argument_gap_hangs(&hang, doc) {
                LeadingBreak::Hard
            } else if edge_gate {
                self.argument_leading_break(doc)
            } else {
                LeadingBreak::None
            }
        };

        // Assignment expressions need parentheses for clarity: return (a = b);
        // Comments go BEFORE the parens: return /* comment */ (a = b);
        // Matches Prettier's behavior for both return and throw.
        // Note: own-line comment check above takes priority — when there's a line
        // comment, the whole thing wraps in outer parens with build_comment_paren_doc
        // (which adds inner assignment parens separately).
        if matches!(arg.kind, ExpressionKind::AssignmentExpression(_)) {
            if let Some(comments_doc) = inline_comments
                && leading_break(comments_doc) != LeadingBreak::None
            {
                return self.build_comment_paren_doc(keyword, keyword_end, arg, span_end, gap);
            }
            let expr_doc = self.build_expression_doc(arg);
            if self.argument_gap_hangs(&hang, expr_doc) {
                return self.build_comment_paren_doc(keyword, keyword_end, arg, span_end, gap);
            }
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
                    keyword == "return",
                    gap,
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
            && let ExpressionKind::SequenceExpression(seq) = &arg.kind
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
            let seq_doc =
                self.build_sequence_doc_value(seq, arg.span, grouping_close, SeqLayout::Hanging);
            let rhs_doc = self.prepend_opt(inline_comments, seq_doc);
            let rhs_doc = match leading_break(rhs_doc) {
                LeadingBreak::Hard => {
                    return self.build_comment_paren_doc(keyword, keyword_end, arg, span_end, gap);
                }
                LeadingBreak::Soft => self.build_asi_soft_pair(rhs_doc),
                LeadingBreak::None => rhs_doc,
            };
            self.debug_assert_argument_edge_holds(rhs_doc);
            let mut parts: DocBuf = smallvec![d.text(keyword), d.text(" "), rhs_doc];
            // Any comment AFTER the grouping `)` (before the `;`) trails after the `;`;
            // the in-paren comment is already inside `seq_doc`.
            let after_start = grouping_close.saturating_add(1).min(span_end);
            let after = if self.has_comments_to_emit_between(after_start, span_end) {
                self.split_terminator_gap_comments(&mut parts, after_start, span_end, true, gap)
            } else {
                DocBuf::new()
            };
            parts.push(d.text(";"));
            parts.extend(after);
            return d.concat(&parts);
        }

        if let ExpressionKind::BinaryExpression(binary) = &arg.kind {
            // The arm's own conditional pair holds a line its group may break; only one
            // ended in every layout needs the hanging form.
            let expr_doc = self.prepend_opt(
                inline_comments,
                self.build_binary_chain_doc_ungrouped(binary, arg.span),
            );
            let expr_doc = match leading_break(expr_doc) {
                LeadingBreak::Hard => {
                    return self.build_comment_paren_doc(keyword, keyword_end, arg, span_end, gap);
                }
                // A line the chain's group may break is glued to a space, as the keyword
                // gap glues the same block written there: left in, it sits inside the broken
                // pair's `group(expr_doc)` and breaks with the chain, a second fixed point one
                // pass away near the width. Where the glue is refused the line stays, still
                // held by the pair, which prints whenever the group breaks.
                LeadingBreak::Soft => d
                    .glue_leading_comment_lines(expr_doc, self.source)
                    .unwrap_or(expr_doc),
                LeadingBreak::None => expr_doc,
            };
            return self.build_binary_paren_doc(keyword, expr_doc, arg.span, span_end, gap);
        }

        // Ternary in return/throw: binary test expressions need continuation indent.
        // Matches Prettier's `shouldNotIndent` (`print/binaryish.js`) — when the binary's
        // grandparent is ReturnStatement/ThrowStatement, shouldNotIndent = false.
        let expr_doc = if let ExpressionKind::ConditionalExpression(cond) = &arg.kind {
            self.build_conditional_doc_with_binary_test_indent(cond, arg.span)
        } else if let ExpressionKind::SequenceExpression(seq) = &arg.kind {
            // `throw` — the `return` arm above claimed its own sequence. Prettier's
            // `shouldIndentSequenceExpression` covers both keywords, so the operands hang
            // inside the parens; only the trailing-comment side differs, and `throw` floats
            // it out, which is the default paren mode.
            self.build_sequence_doc(seq, arg.span, SeqLayout::Hanging)
        } else {
            self.build_expression_doc(arg)
        };
        let rhs_doc = self.prepend_opt(inline_comments, expr_doc);
        let rhs_doc = match leading_break(rhs_doc) {
            LeadingBreak::Hard => {
                return self.build_comment_paren_doc(keyword, keyword_end, arg, span_end, gap);
            }
            LeadingBreak::Soft => self.build_asi_soft_pair(rhs_doc),
            LeadingBreak::None => rhs_doc,
        };
        self.debug_assert_argument_edge_holds(rhs_doc);

        let mut result_parts = smallvec![d.text(keyword), d.text(" "), rhs_doc];
        if has_trailing_comments {
            let after = self.split_terminator_gap_comments(
                &mut result_parts,
                argument_end,
                span_end,
                false,
                gap,
            );
            result_parts.push(d.text(";"));
            result_parts.extend(after);
        } else {
            result_parts.push(d.text(";"));
        }
        d.concat(&result_parts)
    }

    /// What a restricted production's argument doc prints ahead of its first token, read
    /// for ASI ([`tsv_lang::doc::LeadingEdge::asi`]): a line a comment ends there, a line
    /// the doc opens with, or a multi-line block's interior.
    ///
    /// [`LeadingBreak::Hard`] — a line ended in every layout — can only be printed inside
    /// the hanging pair ([`Self::build_restricted_production_paren_doc`]).
    /// [`LeadingBreak::Soft`] — a line ended only when its group breaks, or one in no
    /// group, which takes the enclosing mode — is held by a pair printed with that break
    /// ([`Self::build_asi_soft_pair`], or the binary arm's own).
    pub(in crate::printer) fn argument_leading_break(&self, doc: DocId) -> LeadingBreak {
        self.d().leading_comment_break(doc, self.source).asi()
    }

    /// The conditional pair a soft leading edge takes: `(` and `)` print only when the
    /// pair's group breaks, and a line inside it can break only then, so a line ahead of
    /// the argument's first token is always inside the parens (`return (⏎\t/* d */⏎\ta⏎)`)
    /// or flat (`return /* d */ a`). A line in no group of `doc` is this group's.
    pub(in crate::printer) fn build_asi_soft_pair(&self, doc: DocId) -> DocId {
        let d = self.d();
        d.group(d.concat(&[
            d.if_break(d.text("("), d.empty()),
            d.indent(d.concat(&[d.softline(), doc])),
            d.softline(),
            d.if_break(d.text(")"), d.empty()),
        ]))
    }

    /// The restricted-production invariant, re-read off the doc a BARE path prints after
    /// its keyword: no line terminator ahead of the argument's first token, outside a pair
    /// the path prints itself (`return [no LineTerminator here] Expression`). A debug
    /// build's check (`fuzz:audit:debug` and the corpus-debug runs reach it), so a path the
    /// predicate and the edge gate both miss is a caught panic rather than an ASI break.
    pub(in crate::printer) fn debug_assert_argument_edge_holds(&self, argument: DocId) {
        debug_assert_eq!(
            self.argument_leading_break(argument),
            LeadingBreak::None,
            "a restricted production's bare argument opens with a line terminator"
        );
    }

    /// What a return/throw argument's own-line comments ask of the hanging pair: always,
    /// never, or only if the arm's doc prints a named member gap's comment on a line of its
    /// own ([`ArgumentHang`]).
    ///
    /// Matches Prettier's `returnArgumentHasLeadingComment`
    /// (`utilities/return-statement-has-leading-comment.js`).
    ///
    /// Shared with `build_yield_doc`: `yield`/`yield*` are restricted productions
    /// like `return`/`throw`, so they ask the same question and must not answer it
    /// differently — one question, one predicate.
    pub(in crate::printer) fn argument_hang(
        &self,
        keyword_start: u32,
        arg: &Expression<'_>,
    ) -> ArgumentHang {
        // Own-line comment before the argument itself (`return (\n// c\nexpr)`).
        if self.has_leading_own_line_comment_in_range(
            GapStart::Keyword(keyword_start),
            arg.span().start,
        ) {
            return ArgumentHang::Always;
        }
        // A sequence operand prints its own `(`, which takes in a comment the keyword gap
        // holds inside the statement's stripped `(` (`throw (/* c */ a, b)` prints the
        // comment after the sequence's `(`), so the built doc never shows that block ahead
        // of the argument once one pass has moved it there. The arm is this predicate's,
        // then, for a block whose interior holds ANY line terminator — a `<LS>` / `<PS>`
        // too, which the comment's `\n`-only `multiline` flag misses — exactly as the `\n`
        // spelling takes it (`throw (/* a<LS>b */ (a, zz))` hangs on both passes). Only the
        // sequence: every other arm prints the block ahead of its first token, where the
        // doc edge reads the terminator ([`Self::argument_leading_break`]).
        if matches!(arg.kind, ExpressionKind::SequenceExpression(_))
            && self
                .comments_in_source_between(keyword_start, arg.span().start)
                .any(|c| {
                    c.is_block
                        && tsv_lang::printing::has_newline_between(
                            self.source,
                            c.span.start,
                            c.span.end,
                        )
                })
        {
            return ArgumentHang::Always;
        }

        // Walk the left side of chainable expressions checking for own-line comments
        let mut gaps = SmallVec::new();
        if self.chain_has_own_line_comment(arg, &mut gaps) {
            ArgumentHang::Always
        } else if gaps.is_empty() {
            ArgumentHang::Bare
        } else {
            ArgumentHang::IfGapPrintsLine(gaps)
        }
    }

    /// Whether `doc`, an arm built for a `hang` the chooser left to the doc, prints a comment
    /// on a line of its own in one of the member gaps it named — the reparse's own-line
    /// reading of that gap, so the argument hangs. A gap the read cannot find keeps the pair.
    pub(in crate::printer) fn argument_gap_hangs(&self, hang: &ArgumentHang, doc: DocId) -> bool {
        let ArgumentHang::IfGapPrintsLine(gaps) = hang else {
            return false;
        };
        gaps.iter().any(|&token_start| {
            self.d().gap_comment_line(doc, token_start, self.source) != GapCommentLine::Inline
        })
    }

    /// Walk the left side of the argument looking for leading own-line comments.
    ///
    /// Mirrors Prettier's `hasNakedLeftSide` + `getLeftSide` walk with
    /// `hasLeadingOwnLineComment` asked of each node on the way down. Two gaps are read
    /// per step. The gap between a node's own start and its left child's — the grouping
    /// `(`s the parser stripped from that child, so a comment there LEADS the child
    /// (`(⏎// c⏎a as any) ? b : c` opens the conditional at the `(`, and the comment is
    /// the test's leading comment, exactly where prettier attaches it). And, for a
    /// member, the object→property gap, where an own-line comment leads the property.
    /// Only comments on their own line count (not trailing comments on the same line as
    /// the preceding expression).
    ///
    /// The walk is what keeps a comment glued to the argument's leftmost leaf from
    /// escaping the keyword's line: every printer below strips that leaf's redundant
    /// shell and hoists the run in front of the whole argument, which is a break after
    /// `return` / `throw` / `yield` — ASI — unless the hanging parens hold it. Which
    /// nodes have a hoisted left side — and which pairs RETAIN the run instead, where a
    /// hanging pair would only double the one that survives — is
    /// [`Printer::hoisted_left_side_child`]'s, stated once for this walk and the assignment
    /// layout's.
    ///
    /// A shell the member chain prints MID-CHAIN ([`Printer::hoisted_left_spine`]'s stop)
    /// prints its comments in the member gap after the chain's head, so it is read the
    /// way the chain prints that gap ([`Self::mid_chain_shell_gap_hangs`]) rather than
    /// as a hoisted run; the walk goes on below it, since a shell deeper on the spine
    /// still leads the chain's head.
    fn chain_has_own_line_comment(
        &self,
        expr: &Expression<'_>,
        gaps: &mut SmallVec<[u32; 2]>,
    ) -> bool {
        self.chain_has_own_line_comment_below(expr, None, None, gaps)
    }

    /// [`Self::chain_has_own_line_comment`] at `expr`, inside the member / call run rooted
    /// at `chain_root` (`None` outside one), whose mid-chain shells it skips, and below
    /// `group`, the topmost binary above it, whose group the shells from its leftmost
    /// operand down print inside ([`Printer::left_spine_group_start`]).
    fn chain_has_own_line_comment_below<'e>(
        &self,
        mut expr: &'e Expression<'e>,
        mut chain_root: Option<&'e Expression<'e>>,
        mut group: Option<&'e Expression<'e>>,
        gaps: &mut SmallVec<[u32; 2]>,
    ) -> bool {
        // A loop down the left spine, not a recursion: a chain is as long as the spine limit
        // lets it be, and a frame per link is what the stack contract prices
        loop {
            if let ExpressionKind::MemberExpression(member) = &expr.kind {
                // Leading own-line comment between object and property — the gap as the
                // chain prints it, opening at the head's printed end, so a comment a stripped
                // shell there holds ends the `)`'s line ahead of the rest of the gap
                // ([`Printer::member_gap_start`]).
                //
                // A computed member's gap stops at its `[`: a comment inside the brackets
                // breaks the brackets, not the line after the keyword — the lookup keeps its
                // `[` on the object's line (`return x[⏎// c⏎y.p⏎];`), so no pair is owed.
                // The whole gap is asked first, so only a hit pays the `[` scan.
                let obj_end = self.member_gap_start(member.object);
                let prop_start = member.property.span().start;
                if self.has_leading_own_line_comment_in_range(GapStart::Node(obj_end), prop_start) {
                    if !member.computed {
                        // A plain member's gap is printed by the chain, which glues an
                        // own-line block in a flat member run: whether it keeps its line is
                        // the arm's doc to say.
                        gaps.push(self.member_gap_token(member));
                    } else if self.has_leading_own_line_comment_in_range(
                        GapStart::Node(obj_end),
                        find_bracket_position(self.source, obj_end, prop_start),
                    ) {
                        return true;
                    }
                }
            }
            if let ExpressionKind::CallExpression(call) = &expr.kind
                && let ExpressionKind::MemberExpression(member) = &call.callee.kind
                && !range_too_narrow_for_a_comment(
                    member.object.span().end,
                    member.property.span().start,
                )
                && self.callee_gap_ends_line_after_multiline_block(
                    self.member_gap_start(member.object),
                    member.property.span().start,
                )
            {
                // Whether the chain does break before the call's member is the arm's doc to
                // say, as for the own-line reading above.
                gaps.push(self.member_gap_token(member));
            }
            // No descent through a cast's or a postfix update's bare operand: whether a run
            // hoisted out of it ends a line ahead of the argument is the built doc's to say
            // ([`Self::argument_leading_break`]), and this walk only chooses the arm.
            if matches!(
                expr.kind,
                ExpressionKind::TSAsExpression(_)
                    | ExpressionKind::TSSatisfiesExpression(_)
                    | ExpressionKind::UpdateExpression(_)
            ) {
                return false;
            }
            let Some(left) = self.hoisted_left_side_child(expr) else {
                return false;
            };
            chain_root = match &expr.kind {
                ExpressionKind::MemberExpression(_) | ExpressionKind::CallExpression(_) => {
                    Some(chain_root.unwrap_or(expr))
                }
                _ => None,
            };
            // The stripped-shell gap ahead of the left child: empty when the child was
            // written bare (the two starts coincide). Inside a group a step above opened, a
            // glued block's soft `line` stays flat, so only a comment that breaks its line
            // there hangs ([`Printer::comment_takes_line_in_group`]); the binary arm's own
            // `ifBreak` pair holds the run if the group breaks after all.
            let shell_start = expr.span().start;
            let left_start = left.span().start;
            let in_group = || {
                group
                    .and_then(|group| self.left_spine_group_start(group))
                    .is_some_and(|start| shell_start >= start)
            };
            if shell_start < left_start
                && self.has_leading_own_line_comment_in_range_where(
                    GapStart::Shell {
                        position: shell_start,
                        pair_emitted: self.left_shell_paren_is_emitted(expr, left),
                    },
                    left_start,
                    |c| self.comment_takes_line_in_group(c) || !in_group(),
                )
                && (!chain_root.is_some_and(|root| {
                    chain_mid_chain_paren_gaps(root, self.linearize_input())
                        .contains(&(shell_start, left_start))
                }) || self.mid_chain_shell_gap_hangs(shell_start, left_start))
            {
                return true;
            }
            group = group.or_else(|| {
                matches!(expr.kind, ExpressionKind::BinaryExpression(_)).then_some(expr)
            });
            expr = left;
        }
    }

    /// The token a member gap ends at, for [`Self::argument_gap_hangs`]: the property's
    /// first printed byte — past a private name's `#`, which prints with the `.`.
    fn member_gap_token(&self, member: &internal::MemberExpression<'_>) -> u32 {
        let prop_start = member.property.span().start;
        prop_start + u32::from(self.source.as_bytes().get(prop_start as usize) == Some(&b'#'))
    }

    /// Whether a called member's object→property gap holds a comment on the closing line
    /// of a multi-line block before it (`a.b.c /*⏎*/ /* e */.m()`): the block forces the
    /// chain open, the broken chain starts the call's member on a line of its own, and the
    /// comment then ends its line — the own-line comment the reparse's object→property
    /// reading hangs ([`Self::has_leading_own_line_comment_in_range`]). The source has no
    /// newline after it yet, so the first pass names the gap itself, and the arm's doc says
    /// whether the chain does break there ([`Self::argument_gap_hangs`]).
    fn callee_gap_ends_line_after_multiline_block(&self, obj_end: u32, prop_start: u32) -> bool {
        let mut block_end = None;
        // The gap emitter's own run, which also prints a comment after a `)` that opens
        // its line on the closing line of a trailing block before it
        // (`(a /** b⏎ */⏎) /* e */.m()` prints `a /** b⏎ */ /* e */`).
        let mut scan = self.chain_gap_trail_scan(obj_end);
        self.comments_in_source_between(obj_end, prop_start)
            .any(|c| {
                let after_paren = matches!(
                    self.chain_gap_trail(&mut scan, c),
                    tsv_lang::GapTrail::AfterParen
                );
                let on_block_line = block_end.is_some_and(|end| {
                    after_paren
                        || !tsv_lang::printing::has_newline_between(self.source, end, c.span.start)
                });
                block_end = (c.is_block && c.multiline).then_some(c.span.end);
                on_block_line
            })
    }

    /// Whether a shell gap the member chain prints MID-CHAIN (`[shell_start, left_start)`,
    /// one of [`chain_mid_chain_paren_gaps`]) asks for the hanging pair.
    ///
    /// The gap's comments split two ways. One the head's leaf OWNS
    /// ([`tsv_lang::Comment::owned_by_node`]) prints from inside the leaf, ahead of the
    /// chain's head and so on the keyword's line: a line-spanning block there is a line
    /// terminator ahead of the argument (`return (/* a⏎b */ a.b).m(1)` would print
    /// `return /* a⏎b */ a.b.m(1)`, which reparses as `return;`). The rest print as ONE
    /// run in the member gap after the head, read the way the reparse reads that gap: a
    /// comment there on a line of its own with a line terminator after it is what the
    /// reparse's object→property reading hangs (`return (⏎// c⏎a.b).m(1)` prints
    /// `a⏎// c⏎.b.m(1)`), so pass 1 must hang it too. A comment opens a line of its own
    /// when the source puts a newline before it — in a run holding a `//`, a newline
    /// anywhere since the previous comment ended or the shell opened, the `(`s between
    /// included (`(/* d */⏎ (// e⏎ a.b)).m()` prints `// e` under `a /* d */`); a
    /// block-only run keeps only the newline right before the comment. The opened line
    /// hangs once a `//`, or a newline after a comment, ends it — the nested shells
    /// `(// d⏎ (// e⏎ a.b)).m()` print `// e` under `// d`. Glued to its `(` and first in
    /// the run, a comment trails the head on the keyword's line instead
    /// (`return ((// d⏎(a).b()).c.m())` prints `return a // d⏎\t.b()…`) and is skipped.
    //
    // The run's line structure is re-derived from the source here rather than read off
    // the chain's emitter, whose gap classification (`classify_chain_gap`) is not the
    // printed layout: a block it buckets as leading still prints inline in a flat member
    // run (`(/* d */⏎ (/* d */⏎ a.b)).m()` prints `a /* d */ /* d */.b`).
    fn mid_chain_shell_gap_hangs(&self, shell_start: u32, left_start: u32) -> bool {
        let has_line = self
            .comments_in_source_between(shell_start, left_start)
            .any(|c| !c.owned_by_node && !c.is_block);
        let mut prev_end = shell_start;
        let mut opened = false;
        let mut after_multiline_block = false;
        for c in self.comments_in_source_between(shell_start, left_start) {
            if c.owned_by_node {
                if c.multiline {
                    return true;
                }
                prev_end = c.span.end;
                after_multiline_block = false;
                continue;
            }
            // A comment on the closing line of a multi-line block before it has a line
            // terminator between itself and the head, inside that block — the reparse's
            // object→property reading counts it (`(/*⏎*/ // e⏎a.b).m()` prints
            // `a /*⏎*/ // e⏎.b.m()`, whose `// e` the next pass hangs).
            let starts_line = after_multiline_block
                || if has_line {
                    tsv_lang::printing::has_newline_between(self.source, prev_end, c.span.start)
                } else {
                    has_newline_before_position(self.source, c.span.start)
                };
            after_multiline_block = c.is_block
                && tsv_lang::printing::has_newline_between(self.source, c.span.start, c.span.end);
            opened |= starts_line;
            let ends_line = !c.is_block || has_newline_after_position(self.source, c.span.end);
            if opened && ends_line {
                return true;
            }
            if ends_line {
                opened = false;
            }
            prev_end = c.span.end;
        }
        false
    }

    /// Whether a comment in the gap *leads* the node at `end` and puts a line terminator
    /// between itself and that node — Prettier's `hasLeadingOwnLineComment`
    /// (`utilities/has-leading-own-line-comment.js`), plus, at the keyword gap, the second
    /// disjunct of its `returnArgumentHasLeadingComment`. Two terms, and both are
    /// load-bearing:
    ///
    /// - **Leads.** Decided by `gap_start`: a comment sharing a preceding *node*'s line
    ///   trails that node rather than leading the next one (Prettier attaches it as a
    ///   trailing comment), so it never counts — `return foo() // c` + `.bar` keeps the
    ///   chain bare.
    /// - **Carries a line terminator.** This is what makes a break unavoidable: the node
    ///   cannot share the comment's last line, so the caller must emit the form that
    ///   survives one. A block comment with code after it on the same line
    ///   (`return /* c */ (x)`) fails this term and stays inline.
    ///
    /// For `return`/`throw` the second term is an ASI guard, not cosmetics. Both are
    /// restricted productions (`return [no LineTerminator here] Expression`), so putting the
    /// argument on a later line without parens *changes the program*: `return` silently
    /// becomes `return;` plus an unreachable statement, and `throw` becomes a syntax error.
    ///
    /// Which is why the term is a disjunction at [`GapStart::Keyword`]: a comment can carry
    /// the terminator *inside* itself as well as after itself. A `MultiLineComment` holding
    /// a `LineTerminator` **is** one for ASI (ecma262 sec-comments: it "is considered to be
    /// a *LineTerminator* for purposes of parsing by the syntactic grammar"), so
    /// `throw (/* a⏎b */ x)` is exactly as unprintable bare as `throw (/* c */⏎x)` —
    /// `has_newline_after_position` alone reads the first as inline and emits a dead
    /// document. Prettier says the same thing with its own second disjunct
    /// (`hasNewlineInRange(locStart(comment), locEnd(comment))`, asked of the argument's
    /// leading comments in `utilities/return-statement-has-leading-comment.js`).
    ///
    /// A [`GapStart::Shell`] whose pair the printer does not re-emit counts the interior
    /// terminator too, since its run is hoisted into exactly the keyword's gap
    /// (`throw (/* a⏎b */a).b` takes the hanging pair). [`GapStart::Node`] keeps the narrow
    /// term: a comment there follows the object, so the argument has already begun on the
    /// keyword's line and a line inside the comment ends nothing.
    //
    // TODO: two left-side spellings of the multi-line block still take two passes, layout
    // only (each output reparses as authored): a sequence head at `return` / `throw`,
    // `return (/* a⏎b */a), b` (pass 1 breaks `a,⏎b`, pass 2 joins it), and a binary under
    // another one at `yield` / `yield*`, `yield ((/* a⏎b */a) + b) * c` (pass 1 keeps the
    // hanging pair, pass 2 drops it).
    fn has_leading_own_line_comment_in_range(&self, gap_start: GapStart, end: u32) -> bool {
        self.has_leading_own_line_comment_in_range_where(gap_start, end, |_| true)
    }

    /// [`Self::has_leading_own_line_comment_in_range`] over the comments `counts` admits.
    fn has_leading_own_line_comment_in_range_where(
        &self,
        gap_start: GapStart,
        end: u32,
        counts: impl Fn(&internal::Comment) -> bool,
    ) -> bool {
        // At a node, a comment leads exactly when the gap's trail scan does not take it
        // (`tsv_lang::GapTrailScan`, the chain gap emitter's reading), so a comment the
        // emitter prints right after a kept pair's `)` never reads as own-line here. The
        // scan takes the next-pass reading: a comment on a multi-line block's closing line
        // is one the NEXT pass's object→property reading hangs
        // (`callee_gap_ends_line_after_multiline_block`).
        let mut scan = self.chain_gap_trail_scan_fixed(gap_start.position());
        self.comments_in_source_between(gap_start.position(), end)
            .map(|c| {
                let leads = match gap_start {
                    GapStart::Keyword(_) | GapStart::Shell { .. } => true,
                    GapStart::Node(_) => !self.chain_gap_trail(&mut scan, c).trails(),
                };
                (c, leads)
            })
            .filter(|(c, _)| counts(c))
            .any(|(c, leads)| {
                let carries_line_terminator = (gap_start.counts_interior_line_terminator()
                    && c.multiline)
                    || has_newline_after_position(self.source, c.span.end);
                leads && carries_line_terminator
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
        // Where the argument's own docs begin — the far end of the leading gap this layout
        // emits. A sequence renders BARE below (the hanging parens ARE its grouping pair),
        // and the bare builder emits nothing ahead of its first operand, so the sequence's
        // leading edge — the `(` the parser stripped from that operand, and any comment
        // inside it (`return (⏎// c⏎a), b`) — is this gap's to print too; the sequence's
        // own span opens at that `(`, where the bare builder's scans do not reach.
        //
        // The bare sequence's head prints inside the sequence's group, so a run hoisted
        // from a shell on the head's own left spine would land there, flat behind a glued
        // block's soft `line` (`return (⏎/* d */ a.b, z⏎)`), where the reparse reads it as
        // this gap's and prints the sequence's own pair instead. The gap claims that
        // region ([`Printer::left_spine_run_claim`]) and prints it here, outside the group,
        // where the run keeps the line the hang gave it.
        let (arg_start, claim) = match &arg.kind {
            ExpressionKind::SequenceExpression(seq) => {
                let head = seq.expressions[0];
                // Keyed on the empty span at the keyword's end, which is no node's.
                let claim = self.left_spine_run_claim(
                    Span::new(keyword_end, keyword_end),
                    head.span().start,
                    head,
                    || true,
                );
                (
                    claim.map_or_else(|| head.span().start, |claim| claim.region.end),
                    claim,
                )
            }
            _ => (arg.span().start, None),
        };

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
        let frozen = self.value_head_frozen_span(paren_gap, arg.span());
        let expr_doc = self.with_left_spine_claim(claim, || match frozen {
            Some(frozen) => self.build_frozen_node_doc(frozen),
            None => match &arg.kind {
                ExpressionKind::SequenceExpression(seq) => {
                    self.build_sequence_doc_bare(seq, arg.span, arg.span.end)
                }
                // A return/throw argument is `shouldNotIndent` (`print/binaryish.js`) whichever
                // form it takes: these hanging parens supply the one level, exactly as the
                // `if_break` pair does on the ordinary path
                // (`Self::build_binary_paren_doc`, which reaches the same layout through
                // the ungrouped builder because there the group is the parent's).
                //
                // A MULTI-LINE block the operand OWNS is claimed HERE rather than by the
                // innermost node its token begins
                // ([`Printer::build_value_with_outermost_owned_comment`]): the hanging pair
                // is a required pair like every other on that seam's list, and left inside
                // the operand's own doc the comment's reprinted body force-breaks a group
                // prettier keeps flat (`return (⏎// c⏎/* a⏎b */ cond ? a : b⏎)` exploded the
                // ternary). The position does not move — this is where the pair already
                // prints the gap's un-owned run — only the group the comment sits in. The
                // SEQUENCE arm above takes the same claim inside its own printer, keyed on
                // the run rather than on an operand node, so it must not be wrapped twice.
                //
                // The claim is reached past every grouping shell the parser erased from the
                // argument's LEFT SPINE ([`Printer::build_value_with_left_spine_owned_comment`]):
                // a stripped shell prints no bytes, so the leaf behind it is what the hanging
                // pair prints first, and keying on the argument's own span start declined
                // there — `return (/* a⏎b */ a) ? b : c;` exploded the ternary across three
                // lines where both formatters keep it flat.
                _ => self.build_value_with_left_spine_owned_comment(arg, || {
                    self.build_flat_chain_expression_doc(arg)
                }),
            },
        });
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
        // The gap opens where the argument's doc stops printing, which for a BARE sequence
        // is its last operand — the trailing-side twin of the `arg_start` above: the shell
        // the parser erased from that operand closes inside these parens, so its comments
        // are this gap's, and anchored at the node's end they reach no emitter at all and
        // are DROPPED. One rule with the shell builder's
        // ([`Printer::shell_trailing_gap_start`], which also carries the FROZEN answer);
        // the hanging parens are the printer's own and always emitted, so this caller
        // states that rather than scanning the gap for an author's `)`.
        let trailing_start = self.shell_trailing_gap_start(arg, boundary, frozen, true);
        if self.has_comments_to_emit_between(trailing_start, boundary) {
            self.append_trailing_paren_comments(&mut body, trailing_start, boundary);
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
        gap: TerminatorGap,
    ) -> DocId {
        let d = self.d();
        let (hanging, boundary) =
            self.build_restricted_production_paren_doc(keyword, keyword_end, arg, span_end);
        let mut parts: DocBuf = smallvec![hanging];
        let after = if self.has_comments_to_emit_between(boundary, span_end) {
            self.split_terminator_gap_comments(&mut parts, boundary, span_end, true, gap)
        } else {
            DocBuf::new()
        };
        parts.push(d.text(";"));
        parts.extend(after);
        d.concat(&parts)
    }

    /// Whether the operand's own arm below already keeps a `//` inside the parens it
    /// prints, so the hanging form must not preempt it.
    ///
    /// Two do. The **binaryish** arm prints a pair of its own around a `//` written before
    /// the `)` ([`Self::build_binary_paren_doc`]); the **`return` sequence** arm builds
    /// through the value-position sequence printer, which takes the grouping `)` and emits
    /// the comment inside it — and breaks per operand, a layout the hanging form does not
    /// reproduce. `throw`'s sequence is not one of them: it falls through to the generic
    /// arm, whose `build_sequence_doc` pair the terminator gap floats the comment past.
    ///
    // TODO: the two sequence layouts disagree — per operand here, one line inside the
    // hanging parens (every keyword, including `return` once a *leading* own-line comment
    // forces that form). One construct, one question, two answers; worth a verdict.
    fn operand_arm_keeps_line_comment_inside(
        &self,
        keyword: &'static str,
        arg: &Expression<'_>,
    ) -> bool {
        match &arg.kind {
            ExpressionKind::BinaryExpression(_) => true,
            ExpressionKind::SequenceExpression(_) => keyword == "return",
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
    /// Matches Prettier's `printReturnOrThrowArgument` (`print/return-statement.js`):
    /// when the argument is `isBinaryish`, wraps in `ifBreak("(")...ifBreak(")")`.
    ///
    /// When the expression contains hardlines (multi-line callbacks, block bodies,
    /// object literals), the group is forced to break so `ifBreak` produces parens.
    /// This matches Prettier's `propagateBreaks` preprocessing which cascades
    /// `breakParent` (bundled with every `hardline`) up through all ancestor groups.
    /// Our renderer's `will_break` can't see through `IfBreak` nodes, so we detect
    /// hardlines in the expression doc and force the group to break explicitly.
    ///
    /// A comment the author wrote between the operand and the `)` of grouping parens
    /// around it stays inside the pair this prints, and holds it open, unless it is a
    /// single-line block trailing the operand's line — the same whether the statement
    /// ends in a `;` or ASI ends it at that `)`.
    ///
    /// `expr_doc` is the ungrouped chain with the keyword gap's run ahead of it, built by
    /// the caller so it can read the doc's leading edge first: a line the run ends there
    /// only when a group breaks is this pair's to hold, since the flat arm prints with the
    /// group flat.
    fn build_binary_paren_doc(
        &self,
        keyword: &'static str,
        expr_doc: DocId,
        span: Span,
        span_end: u32,
        gap: TerminatorGap,
    ) -> DocId {
        let d = self.d();

        // The terminator gap runs from the operand's end to the statement's `;` — or, where
        // ASI ended the statement, to the statement's own end, which is then just past the
        // author's closing `)`. The scan skips comments so a `;` inside one
        // (`a + b /* ; */ /* c */;`) isn't mistaken for the statement's terminator, which
        // would drop the comments after it. It is bounded by `span_end` (the statement's
        // own end): under ASI there is no `;` within the statement, so the scan must not
        // wander past it into the enclosing source and find a later terminator (the object
        // literal's `};`, the next statement's `;`) — that would pull the statement's own
        // trailing comment into this gap AND leave it for the block's trailing-comment
        // emitter too, printing it twice.
        let expr_end = span.end;
        let gap_end = find_char_skipping_comments(
            self.source.as_bytes(),
            expr_end as usize,
            span_end as usize,
            b';',
        )
        .map_or(span_end, |p| p as u32);

        // The author's grouping `)` splits the gap: what sits before it was written inside
        // the pair, what sits past it trails the statement. A run inside the pair that
        // cannot print on the flat path — a `//`, a comment on a line of its own, or a
        // block whose own text spans a line — HOLDS the pair: it is printed open and the
        // run prints inside it, where it was written, through the emitter the hanging
        // form uses for the same region ([`Self::build_restricted_production_paren_doc`]).
        // Only the region past the `)` is then the terminator split's.
        //
        // ⚠️ "The run defers" and "the pair is held" are one reading — the emitter's own
        // report plus the multi-line test. A deferred doc on the flat arm would carry the
        // comment past the `;`, out of the pair it was written in.
        let mut inline_trailing = DocBuf::new();
        let mut held_close = None;
        if let Some(close) = self.retained_grouping_close(expr_end, gap_end) {
            let deferred =
                self.append_trailing_paren_comments(&mut inline_trailing, expr_end, close);
            if deferred || self.has_multiline_block_comments_on_page_between(expr_end, close) {
                held_close = Some(close);
            } else {
                // The run is single-line blocks on the operand's line alone, which the
                // split below places by its own rule. Nothing built for it is on the page
                // or counted: a comment doc is recorded where it renders, and only a run
                // that defers is counted.
                inline_trailing.clear();
            }
        }
        let after_semi = if let Some(close) = held_close {
            // Nothing past the last `)` is enclosed by it, so the split holds nothing back
            // ahead of the `;`.
            let mut before_semi = DocBuf::new();
            let after =
                self.split_terminator_gap_comments(&mut before_semi, close, gap_end, false, gap);
            debug_assert!(before_semi.is_empty());
            after
        } else {
            // No pair, or a pair whose run is same-line single-line blocks alone. Such a
            // block does not hold the pair open: it stays before the `)` when the operand
            // breaks (`return (⏎\ta +⏎\tb /* c */⏎);`) and before the `;` when it fits,
            // while a comment written past the `)` trails *after* the `;`
            // (`return a + b; /* c */`). See `split_terminator_gap_comments`.
            self.split_terminator_gap_comments(&mut inline_trailing, expr_end, gap_end, true, gap)
        };
        let trailing_comments_doc = d.concat(&inline_trailing);

        // A held pair is printed with break POINTS — the hanging form's shape
        // ([`Self::build_hanging_paren_doc`]) — rather than as a group told to break: a
        // host that flattens soft breaks (a Svelte block head inside a
        // whitespace-sensitive element) keeps hard lines alone, and under it the
        // conditional pair below takes its flat arm and carries the run out of the pair.
        // Nothing trails the `(` here: a `//` there leads the operand on a line of its
        // own, which takes the hanging form before this arm is reached.
        if held_close.is_some() {
            let body = d.concat(&[d.group(expr_doc), trailing_comments_doc]);
            let mut held_parts: DocBuf = smallvec![
                self.build_hanging_paren_doc(keyword, body, None),
                d.text(";"),
            ];
            held_parts.extend(after_semi);
            return d.concat(&held_parts);
        }

        // When the expression contains hardlines (e.g., multi-line callback in a
        // chain), the group must break to produce parens. In Prettier, hardline
        // includes breakParent which propagateBreaks cascades up. Our will_break
        // can't see through IfBreak, so we check the expression doc directly.
        let force_break = d.will_break(expr_doc);

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

        // The invariant, read off the pair as the keyword's group prints it.
        if cfg!(debug_assertions) {
            let pair = d.if_break(broken_doc, flat_doc);
            self.debug_assert_argument_edge_holds(if force_break {
                d.group_break(pair)
            } else {
                d.group(pair)
            });
        }
        if force_break {
            d.group_break(inner)
        } else {
            d.group(inner)
        }
    }
}
