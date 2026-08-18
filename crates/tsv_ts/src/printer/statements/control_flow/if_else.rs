// if/else statement printing
//
// Entry point (`build_if_statement_doc`) plus the wrapping and
// comment-handling variants, and else-clause layout helpers.

use crate::ast::internal::{self, Statement};
use crate::printer::Printer;
use crate::printer::statements::StatementContext;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::skip_comment;

/// Check if a statement can be printed inline after `if (cond)` without a newline.
///
/// Block, expression, break, continue, return, throw, and empty statements stay inline.
/// Other statements (if, for, while, etc.) go on a new line with indent.
fn is_inline_consequent(stmt: &Statement<'_>) -> bool {
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
fn is_inline_alternate(stmt: &Statement<'_>) -> bool {
    is_inline_consequent(stmt) || matches!(stmt, Statement::IfStatement(_))
}

impl<'a> Printer<'a> {
    /// Build a non-block, non-inline else body via Prettier's `adjustClause`
    /// (`group(indent([line, clause]))`): `else while (x) g();` stays inline when it fits
    /// and breaks to `else⏎↹while (x) g();` when it doesn't — the same soft-line layout the
    /// consequent uses (see `build_adjust_clause_with_comments`), never a bare hardline. The
    /// caller emits the bare `else` (no trailing space); the leading `line` supplies the
    /// separator when flat.
    fn build_else_adjust_clause(&self, alternate: &Statement<'_>, ctx: StatementContext) -> DocId {
        let d = self.d();
        // One indent wrap; nothing continues after the alternate, so its tail falls
        // through to whatever flushes the if's own position.
        d.group(d.indent_line(self.build_statement_doc(alternate, ctx.clause_body(false, true))))
    }

    /// The doc for one BRANCH of an `if` — a consequent or an alternate. A block body expands
    /// its empty form (`{⏎}` rather than `{}`, prettier's `block.js`); anything else prints as
    /// an ordinary statement. Both branch sides and both `if` printers share it, so the two
    /// can't drift on which block form they emit.
    ///
    /// Callers that introduce the branch across a gap want [`Self::build_branch_body_doc`],
    /// the freeze-aware wrapper; this bare form is for the gaps that provably hold no comment
    /// (and so no directive).
    fn build_branch_statement_doc(
        &self,
        body: &Statement<'_>,
        body_ctx: StatementContext,
    ) -> DocId {
        match body {
            Statement::BlockStatement(block) => self.build_block_statement_expand_empty_doc(block),
            _ => self.build_statement_doc(body, body_ctx),
        }
    }

    /// [`Self::build_branch_statement_doc`] with the introducing gap's format-ignore freeze
    /// applied: an own-line directive in `[gap_start, body.start)` freezes the branch whole,
    /// over its own span, so a block's braces ride inside the slice.
    fn build_branch_body_doc(
        &self,
        gap_start: u32,
        body: &Statement<'_>,
        body_ctx: StatementContext,
    ) -> DocId {
        self.build_statement_head_doc(gap_start, body.span(), || {
            self.build_branch_statement_doc(body, body_ctx)
        })
    }

    /// Append the `else` keyword, the comment run in its →body gap, and the alternate body —
    /// the comment-bearing else layout, shared by BOTH `if` printers. The two differ only in
    /// the `}`→`else` gap that selects them, so this gap must answer identically in each;
    /// `leading_space` prefixes ` else` (set when `else` abuts a preceding `}` on its line),
    /// mirroring [`Self::append_else_keyword_body`].
    ///
    /// The gap routes through the shared header→body emitters like `try`'s or `do`'s: a comment
    /// trailing `else` stays trailing, one on its own line keeps its own line, and a `//`
    /// forces the body down. Emitting the run inline relocated an own-line comment up onto the
    /// `else` line (`} else // c`) — the same defect the `try` family had. Prettier preserves
    /// here, so it is a clean oracle rather than a stance.
    ///
    /// Which layout the gap gets is prettier's, measured on all three alternate shapes:
    ///
    /// | alternate | first gap comment own-line | first gap comment trails `else` |
    /// | --- | --- | --- |
    /// | **block** | flush (`} else⏎// c⏎{`) | flush |
    /// | **else-if** | continuation indent (`} else⏎→// c⏎→if (b) {`) | flush |
    /// | **other non-block** | continuation indent | continuation indent |
    ///
    /// A block alternate's own `{` opens the body, so the gap's comment belongs at the head's
    /// level either way. An else-if is a nested statement and takes the indent — but only when
    /// the run owns its lines: a comment TRAILING `else` is a trailing comment of the keyword,
    /// printed before the clause and outside its indent, and prettier keeps the whole run flush
    /// there (pinned by `if/else_consecutive_comment`). `inline_prev` — the emitters' own
    /// partition — is that question.
    ///
    /// An own-line directive in the gap freezes the alternate whole
    /// ([`Self::build_statement_head_doc`]).
    fn append_else_gap_and_body(
        &self,
        parts: &mut DocBuf,
        alternate: &Statement<'_>,
        else_end: u32,
        leading_space: bool,
        ctx: StatementContext,
    ) {
        let d = self.d();
        let alt_start = alternate.span().start;
        let flush_with_else = match alternate {
            Statement::BlockStatement(_) => true,
            Statement::IfStatement(_) => {
                self.has_anchor_trailing_comment_between(else_end, alt_start)
            }
            _ => false,
        };
        parts.push(d.text(if leading_space { " else" } else { "else" }));
        // Only the indented arm below wraps the body; the flush arm prints it on the
        // `else`'s own line.
        let body_doc = self.build_branch_body_doc(
            else_end,
            alternate,
            ctx.clause_body(false, !flush_with_else),
        );
        if flush_with_else {
            self.push_header_to_body_gap(parts, else_end, alt_start);
            parts.push(body_doc);
        } else {
            self.push_indented_else_to_body_gap(parts, else_end, alt_start, body_doc);
        }
    }

    /// Append the `else` keyword and its alternate body, choosing the layout: an inline
    /// alternate (block / expression / else-if) prints `else <body>`; a non-block, non-inline
    /// alternate uses `adjustClause` (`else` + [`Self::build_else_adjust_clause`]) so it stays
    /// inline when it fits and breaks to `else⏎↹clause` otherwise. `leading_space` prefixes
    /// ` else` — set when `else` abuts a preceding `}` on the same line (`} else …`), cleared
    /// when it starts its own line after a `hardline`. (EmptyStatement and comment-bearing
    /// alternates are handled by the callers, not here.)
    fn append_else_keyword_body(
        &self,
        parts: &mut DocBuf,
        alternate: &Statement<'_>,
        leading_space: bool,
        ctx: StatementContext,
    ) {
        let d = self.d();
        if is_inline_alternate(alternate) {
            // Inline alternate: on the `else`'s line with no indent wrap.
            parts.push(d.text(if leading_space { " else " } else { "else " }));
            parts.push(self.build_branch_statement_doc(alternate, ctx.clause_body(false, false)));
        } else {
            parts.push(d.text(if leading_space { " else" } else { "else" }));
            parts.push(self.build_else_adjust_clause(alternate, ctx));
        }
    }

    /// Append the `else` clause: the keyword, its →body gap's comments, and the alternate
    /// — the four-way dispatch **both** `if` printers need.
    ///
    /// `leading_space` prefixes the keyword with a space. It is the *only* thing that
    /// differs between the two callers: the plain printer's `}` sits on the keyword's line
    /// (`} else /* c */ {`), while the comment-bearing one has already emitted the
    /// `}`→`else` gap, which supplies its own separator.
    ///
    /// `else_end` is [`Self::find_else_keyword_end_between`]'s answer, passed in rather
    /// than re-scanned: the caller needs the keyword's START for the `}`→`else` gap it
    /// emits first, so one scan serves both. `None` is the scan failing, which leaves no
    /// gap to emit.
    ///
    /// ⚠️ These were two copies of this dispatch, and the copy drifted: the empty-alternate
    /// arm here moved to the leading-run separators while the other kept an older
    /// `has_line_comments_between` tail, gluing a `/* c1 */⏎// c2` pair the run rule breaks.
    /// One dispatch, one bool.
    fn append_else_clause(
        &self,
        parts: &mut DocBuf,
        alternate: &Statement<'_>,
        else_end: Option<u32>,
        leading_space: bool,
        ctx: StatementContext,
    ) {
        let d = self.d();
        let alt_start = alternate.span().start;

        if matches!(alternate, Statement::EmptyStatement(_)) {
            // Empty alternate: `else;`, `else /* c */ ;`, or `else // c\n;` — the same gap
            // the `)`→`;` one is, so it shares that emitter.
            match else_end {
                Some(else_end) => {
                    parts.push(d.text(if leading_space { " else" } else { "else" }));
                    self.push_empty_statement_gap(parts, else_end, alt_start);
                }
                None => parts.push(d.text(if leading_space { " else;" } else { "else;" })),
            }
        } else if let Some(else_end) = else_end
            && self.has_comments_to_emit_between(else_end, alt_start)
        {
            self.append_else_gap_and_body(parts, alternate, else_end, leading_space, ctx);
        } else {
            self.append_else_keyword_body(parts, alternate, leading_space, ctx);
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
        while i + "else".len() <= end {
            if let Some(new_i) = skip_comment(bytes, i, end) {
                // Clamp in case a block comment was unterminated
                i = new_i.min(end);
                continue;
            }
            // `str::get` (not `source[i..i + 4]`): `bytes[i] == b'e'` proves `i` is a char
            // boundary, but `i + "else".len()` is not — an `e` followed within 3 bytes by a
            // multibyte char lands the slice end mid-codepoint and panics. `get` returns
            // `None` there, so a doomed scan over minted multibyte text simply finds no `else`.
            if bytes[i] == b'e' && self.source.get(i..i + "else".len()) == Some("else") {
                return Some((i + "else".len()) as u32);
            }
            i += 1;
        }
        None
    }

    /// The `if` head ([`Self::build_paren_condition_head`], shared with `while`) and its
    /// consequent, as one parts buffer — everything **both** `if` printers built
    /// identically, so neither can drift from the other again.
    ///
    /// ⚠️ It has drifted twice, and both times the comment-bearing printer was the one
    /// that re-decided: once with a weaker head→body gate that relocated an own-line block
    /// UP onto the `)` line, once with `is_inline_consequent` + a bare `") "` in place of
    /// `adjustClause`'s `indent([line, body])`, which had no way to drop the body at all
    /// and so broke the CONDITION PARENS open on overflow — a form neither prettier nor
    /// the other printer produces. The consequent must not be decided twice; the only real
    /// difference between the two printers is the `}`→`else` gap that SELECTS them.
    ///
    /// The head half of that drift is now unreachable from here at all — it is
    /// [`Self::build_paren_condition_head`], which `while` takes too. What remains here is
    /// the CONSEQUENT dispatch, which is `if`'s own (a `while` body has no `else` after it).
    ///
    /// The caller owns what follows: the `else` clause (which differs) and whether the
    /// whole statement is grouped.
    fn build_if_head_and_consequent(
        &self,
        stmt: &internal::IfStatement<'_>,
        ctx: StatementContext,
    ) -> DocBuf {
        let (mut parts, paren_end) =
            self.build_paren_condition_head("if", stmt.span.start, &stmt.test);
        // The consequent is always one `adjustClause` indent in, and an `else`
        // CONTINUES on the line its tail flushes at (a block consequent ignores this).
        let body_ctx = ctx.clause_body(stmt.alternate.is_some(), true);
        match stmt.consequent {
            // Block consequent: `if (` + condition + `) ` + block.
            Statement::BlockStatement(block) => {
                self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
                parts.push(self.build_branch_body_doc(paren_end, stmt.consequent, body_ctx));
            }
            // `if (cond);` or `if (cond) /* c */ ;`
            Statement::EmptyStatement(_) => {
                let empty_start = stmt.consequent.span().start;
                self.append_close_paren_empty_stmt_with_comments(
                    &mut parts,
                    paren_end,
                    empty_start,
                );
            }
            // Non-block consequent: prettier's `adjustClause` — `indent([line, clause])`,
            // so `line` is a space when flat (`if (cond) a;`) and a newline + indent when
            // broken. `parts` holds exactly the head at this point, which is what
            // adjustClause groups with the body.
            _ => {
                let body_start = stmt.consequent.span().start;
                let consequent_doc =
                    self.build_branch_body_doc(paren_end, stmt.consequent, body_ctx);
                let head = std::mem::take(&mut parts);
                parts.push(self.build_adjust_clause_with_comments(
                    &head,
                    paren_end,
                    body_start,
                    consequent_doc,
                ));
            }
        }
        parts
    }

    /// Build a doc for an if statement with proper line-width wrapping.
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
    ///
    /// **One printer.** There were two — selected on whether a comment sat in the
    /// consequent→alternate gap — and every part they had in common drifted at least once
    /// (see [`Self::build_if_head_and_consequent`] and [`Self::append_else_clause`]). The
    /// gap that selected them is not a separate layout at all: [`Self::push_block_to_keyword_gap`]
    /// already answers the no-comment case with exactly the separator the comment-free
    /// printer hand-wrote (`" "` after a `}`, a `hardline` otherwise), so the two paths are
    /// the same emission.
    ///
    /// The outer group is keyed on the CONSEQUENT alone (prettier's `group([…])` around the
    /// whole statement), never on whether the alternate gap held a comment — the split's
    /// last residue, since the comment-bearing path grouped nothing. It is inert for those
    /// two consequent shapes by construction: `parts` holds only text, hardlines and
    /// self-contained groups, so there is no `line` for a group to flatten. The adversarial
    /// shape is a non-block alternate wide enough to break
    /// (`if (a) {…} /* c */ else while (<overflowing>) g();`), and it does not see the
    /// group either — [`Self::build_else_adjust_clause`] carries its own.
    pub(in crate::printer::statements) fn build_if_statement_doc(
        &self,
        stmt: &internal::IfStatement<'_>,
        ctx: StatementContext,
    ) -> DocId {
        let d = self.d();
        let mut parts = self.build_if_head_and_consequent(stmt, ctx);

        if let Some(alternate) = &stmt.alternate {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alternate.span().start;
            // One keyword scan for both halves of the clause: the `}`→`else` gap needs the
            // keyword's START, the clause itself its END.
            let else_end = self.find_else_keyword_end_between(consequent_end, alternate_start);
            // The `}`→`else` gap. Its no-comment answer is the separator itself, so the
            // keyword takes no leading space of its own here.
            let before_else_end = else_end.map_or(alternate_start, |e| e - "else".len() as u32);
            self.push_block_to_keyword_gap(
                &mut parts,
                consequent_end,
                before_else_end,
                matches!(stmt.consequent, Statement::BlockStatement(_)),
                self.terminator_defers_comment(stmt.consequent.span().start, consequent_end),
            );
            self.append_else_clause(&mut parts, alternate, else_end, false, ctx);
        }

        let doc = d.concat(&parts);
        if matches!(
            stmt.consequent,
            Statement::BlockStatement(_) | Statement::EmptyStatement(_)
        ) {
            d.group(doc)
        } else {
            doc
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrinterInputs;
    use tsv_lang::EmbedContext;
    use tsv_lang::doc::arena::DocArena;

    /// A synthetic if/else region (as `tsv_svelte_compile` mints) can bracket arbitrary
    /// multibyte template text. Here an `e` byte is immediately followed by a multibyte
    /// char whose bytes straddle `i + "else".len()`, so an unchecked `source[i..i + 4]`
    /// slice would panic on a non-char-boundary. The `str::get` form must instead find no
    /// `else` and return `None` without panicking (prod WASM is `panic = "abort"`).
    #[test]
    fn find_else_keyword_end_between_multibyte_is_panic_free() {
        // bytes: `e`, `x`, then the 3-byte em-dash `—` at 2,3,4 — so index 4 (the slice
        // end for an `e` at index 0) falls inside the em-dash.
        let source = "ex—";
        let arena = DocArena::new();
        let inputs = PrinterInputs {
            source,
            comments: &[],
            line_breaks: &[],
            has_owned_comments: false,
            has_format_ignore: false,
        };
        let printer = Printer::with_context(&arena, &inputs, EmbedContext::default(), 0);
        assert_eq!(
            printer.find_else_keyword_end_between(0, source.len() as u32),
            None
        );
    }
}
