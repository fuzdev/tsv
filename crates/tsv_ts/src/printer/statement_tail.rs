// The statement-tail table: which terminator each statement kind owns (prettier's
// `shouldIgnoredNodePrintSemicolon`), and the statement-tail questions read off it —
// where a statement's content ends, whether its terminator gap is handed off to the
// enclosing list, where its comment claim ends, and where a trivia run starts. Both the
// format-ignore freeze (`ignore.rs`) and the statement-list terminator-gap seams read it.

use super::Printer;
use super::comments::ends_no_trivia;
use crate::ast::internal;
use crate::lexer::{is_es_line_terminator, is_es_whitespace};
use tsv_lang::Span;

/// Which terminator a statement kind owns — prettier's `shouldIgnoredNodePrintSemicolon`,
/// read as the three answers it gives. It is therefore also what a freeze over the
/// statement must NOT copy.
///
/// The distinction that matters is `Never` vs. the other two: a `;` a kind does not own is
/// the statement's own content (an empty-statement body's `;`), so it stays inside the
/// frozen slice; a `;` a kind owns is the printer's and is re-emitted rather than frozen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::printer) enum StatementTerminator {
    /// The kind ends in a `;` however it was authored — ASI supplies one where the author
    /// did not. `VariableDeclaration`, `break` / `continue` / `debugger`.
    Always,
    /// The kind ends in a `;` only where the author wrote one; ASI spellings get none.
    /// `ExpressionStatement` (a directive prologue entry included), `return` / `throw` /
    /// do-while and the module declarations.
    IfAuthored,
    /// The kind owns no terminator, so a trailing `;` is content and freezes with it.
    Never,
}

impl<'a> Printer<'a> {
    /// Which terminator rule a statement's kind follows — prettier's
    /// `shouldIgnoredNodePrintSemicolon`. A header kind resolves through its TAIL statement
    /// the way prettier does: a statement's terminator is its tail's.
    pub(in crate::printer) fn statement_terminator(
        mut stmt: &internal::Statement<'_>,
    ) -> StatementTerminator {
        loop {
            match Self::own_statement_terminator(stmt) {
                Ok(terminator) => return terminator,
                Err(tail) => stmt = tail,
            }
        }
    }

    /// The statement `stmt` DELEGATES to — the inner statement that prints its text, its
    /// `;` included — or `None` when `stmt` prints its own.
    ///
    /// A delegating wrapper adds a prefix and hands the rest to
    /// [`Printer::build_statement_doc`] under a context it *inherits*: an `export` with a
    /// declaration (`build_export_declaration_doc`) and a label
    /// (`StatementContext::labeled_body`, which changes only the directive-prologue fact).
    /// So the wrapper and the inner statement must answer
    /// [`Self::statement_hands_off_terminator_gap`] alike, and the inner one is the
    /// authority — it is the one holding the emitter.
    ///
    /// ⚠️ **A clause-BEARING header is not this**, though
    /// [`Self::statement_hands_off_terminator_gap`] now walks through one as well and the
    /// freeze table always did ([`Self::own_statement_terminator`]). The two steps stay
    /// separate because they answer to different context: a delegating wrapper's inner
    /// statement INHERITS the wrapper's context, so the two are interchangeable, while
    /// `if` / `for` / `while` / `with` build their body under `StatementContext::clause_body`,
    /// which decides for itself whether the gap is the list's — it is only when the body's
    /// tail is also the header's (`StatementContext::tail_reaches_list`), and a body whose
    /// tail stays OPEN to a continuing construct (an `if`'s consequent before its `else`, a
    /// do-while's body) keeps the node-owned split. So the clause step is sound here for the
    /// reason its own context states, not because the two walks are the same walk.
    /// Conflating them cost one DOUBLE-PRINT (every `export type A = B;`) and one
    /// DROP (every `l: a();`), each found by the gap-injection audit rather than by a
    /// fixture — the authoring is one no formatted document contains.
    fn delegated_statement<'s, 'arena>(
        stmt: &'s internal::Statement<'arena>,
    ) -> Option<&'s internal::Statement<'arena>> {
        match &stmt.kind {
            internal::StatementKind::ExportNamedDeclaration(decl) => decl.declaration,
            internal::StatementKind::LabeledStatement(stmt) => Some(stmt.body),
            _ => None,
        }
    }

    /// One step of the [`Self::statement_terminator`] table: this statement's OWN
    /// terminator rule, or the tail statement a header kind defers to.
    ///
    /// Two readings of one table rather than two tables. The recursive reading answers "does
    /// a `;` follow this statement's text", which is a fact about the whole construct; the
    /// **one-step** reading answers "is this statement's `;` its own", which is what
    /// [`Self::statement_hands_off_terminator_gap`] needs — a header kind's terminator
    /// belongs to a CLAUSE BODY, printed by a site that keeps the node-owned split, so
    /// lowering the enclosing list's cursor over it would claim a comment that body already
    /// prints (a DOUBLE-PRINT).
    fn own_statement_terminator<'s, 'arena>(
        stmt: &'s internal::Statement<'arena>,
    ) -> Result<StatementTerminator, &'s internal::Statement<'arena>> {
        Ok(match &stmt.kind {
            internal::StatementKind::VariableDeclaration(_)
            | internal::StatementKind::BreakStatement(_)
            | internal::StatementKind::ContinueStatement(_)
            | internal::StatementKind::DebuggerStatement(_) => StatementTerminator::Always,
            internal::StatementKind::ExpressionStatement(_)
            | internal::StatementKind::ReturnStatement(_)
            | internal::StatementKind::ThrowStatement(_)
            | internal::StatementKind::DoWhileStatement(_)
            | internal::StatementKind::ImportDeclaration(_)
            | internal::StatementKind::ExportNamedDeclaration(_)
            | internal::StatementKind::ExportDefaultDeclaration(_)
            | internal::StatementKind::ExportAllDeclaration(_) => StatementTerminator::IfAuthored,
            internal::StatementKind::IfStatement(s) => {
                return Err(s.alternate.unwrap_or(s.consequent));
            }
            internal::StatementKind::ForStatement(s) => return Err(s.body),
            internal::StatementKind::ForInStatement(s) => return Err(s.body),
            internal::StatementKind::ForOfStatement(s) => return Err(s.body),
            internal::StatementKind::WhileStatement(s) => return Err(s.body),
            internal::StatementKind::WithStatement(s) => return Err(s.body),
            internal::StatementKind::LabeledStatement(s) => return Err(s.body),
            _ => StatementTerminator::Never,
        })
    }

    /// Where a statement's own CONTENT ends — prettier's `locEnd` overrides
    /// (`src/language-js/location/overrides.js`), read as a position.
    ///
    /// It is the statement's full end for every kind that owns no terminator
    /// ([`StatementTerminator::Never`]) and for every ASI spelling; otherwise it is the end of
    /// the content the trailing `;` terminates, so the whitespace the author left between
    /// the two is outside it.
    ///
    /// **Two readers, one table.** [`Self::frozen_statement_slice`] asks it for the extent a
    /// freeze copies verbatim — a terminator belongs to the printer, not to the slice. The
    /// statement-list walk asks it for the second anchor prettier's `isNextLineEmpty`
    /// measures a blank line from ([`Printer::statement_content_tail_blank`]). Keeping them
    /// on one reading is the point: the kinds are a table transcribed from prettier, and a
    /// second hand-rolled copy is how the two would come to disagree about where `a()⏎⏎;`
    /// ends.
    ///
    /// The whitespace class is ECMAScript's `WhiteSpace ∪ LineTerminator` — what JS
    /// `trimEnd` trims, which is neither Rust's `char::is_whitespace` (omits `<ZWNBSP>`,
    /// admits `<NEL>`) nor ASCII.
    ///
    /// ⚠️ The trim is over WHITESPACE ONLY — **a comment is content** here, where prettier
    /// measures its comment-STRIPPED text. The two readers absorb that difference the same
    /// way and for the same reason, each stated at its own site.
    pub(in crate::printer) fn statement_content_end(&self, stmt: &internal::Statement<'_>) -> u32 {
        let span = stmt.span();
        if Self::statement_terminator(stmt) == StatementTerminator::Never {
            return span.end;
        }
        let Some(content) = span.extract(self.source).strip_suffix(';') else {
            // ASI supplied the terminator: there is no authored `;` for content to end before.
            return span.end;
        };
        let content = content.trim_end_matches(|c| is_es_whitespace(c) || is_es_line_terminator(c));
        span.start + content.len() as u32
    }

    /// Whether a statement's terminator gap belongs to the enclosing statement LIST rather
    /// than to the statement itself — the kind half of prettier's `nodeTypesWithContentEnd`.
    ///
    /// The **recursive** reading of the [`Self::statement_terminator`] table
    /// ([`Self::own_statement_terminator`]): a HEADER kind's `;` belongs to its clause BODY,
    /// so the walk follows it there and answers for the emitter that actually prints it. The
    /// tail-most body is the one the table names (an `if`'s alternate before its consequent,
    /// a loop's body), which is exactly the body whose tail is the header's own — so the gap
    /// this claims sits between two LIST members like any other, and the list's seam places
    /// its run (`if (cond) expr⏎/* c */;⏎fn();` → `if (cond) expr;⏎/* c */ fn();`, the answer
    /// the plain statement list and prettier both give).
    ///
    /// ⚠️ **`StatementContext::terminator_gap` is the other end of this rule and must agree
    /// exactly** — both claiming is a DOUBLE-PRINT, neither a DROP. It reaches the same body
    /// by carrying `tail_reaches_list` down the clause chain, and both stop at the same
    /// place: a body whose tail stays OPEN to a continuing construct (an `if`'s consequent
    /// before its `else`, a do-while's body) keeps the node-owned split
    /// ([`Printer::push_statement_semicolon`]), because no list seam can reach a gap interior
    /// to the construct.
    ///
    /// A kind the table calls [`StatementTerminator::Never`] has no terminator to eject at all.
    pub(in crate::printer) fn statement_hands_off_terminator_gap(
        &self,
        mut stmt: &internal::Statement<'_>,
    ) -> bool {
        // Walk to the statement that actually prints the `;` — through every DELEGATING
        // wrapper, and through every clause-bearing HEADER to its tail-most clause body
        // ([`Self::own_statement_terminator`] picks that body: an `if`'s alternate before its
        // consequent, a loop's body). Both steps end at one emitter, and the answer is that
        // emitter's.
        loop {
            // `export default`'s value is held INLINE rather than as a statement, so its kind
            // cannot answer alone: an EXPRESSION routes through
            // [`Printer::split_terminator_gap_comments`] and hands over, while a DECLARATION
            // (`export default function main(): void;`) is printed by its own signature
            // printer, which keeps the gap. Reading it as one kind double-printed every
            // ambient default overload.
            if let internal::StatementKind::ExportDefaultDeclaration(decl) = &stmt.kind {
                return matches!(
                    decl.declaration,
                    internal::ExportDefaultValue::Expression(_)
                );
            }
            let inner = match Self::delegated_statement(stmt) {
                Some(inner) => inner,
                None => match Self::own_statement_terminator(stmt) {
                    Ok(terminator) => {
                        return matches!(
                            terminator,
                            StatementTerminator::Always | StatementTerminator::IfAuthored
                        );
                    }
                    Err(body) => body,
                },
            };
            // ⚠️ A directive INSIDE the step freezes only the inner statement
            // (`export⏎// prettier-ignore⏎const a = 1;`, `l:⏎// prettier-ignore⏎fn();`,
            // `if (a)⏎// prettier-ignore⏎fn(  b  );`), and a frozen slice already holds its
            // terminator-gap comment — the same reason the walk's own freeze verdict
            // suppresses the hand-off ([`Printer::statement_emitted_end`]). The walk cannot
            // see this one: it asks of the gap ABOVE the outermost statement, which is
            // directive-free here.
            if self.member_gap_frozen(stmt.span().start, inner.span().start) {
                return false;
            }
            stmt = inner;
        }
    }

    /// Where a statement's own COMMENT CLAIM ends — prettier's `__contentEnd`
    /// (`setContentEnd`, `src/language-js/parse/postprocess/index.js`), the third reader of
    /// the [`Self::statement_content_end`] table and the only one that measures the
    /// comment-STRIPPED text prettier itself measures.
    ///
    /// The whole point of the difference: the region between a statement's content and the
    /// `;` that terminates it is trivia, so a comment the author left there is **not inside
    /// the statement** for attachment purposes — it falls in the gap BETWEEN two statements,
    /// where the ordinary own-line / same-line split decides whether it trails the statement
    /// before it or leads the one after (`docs/comments.md` §The statement-gap seam). That is
    /// what makes `a()⏎/* c */;⏎b();` print as `a();⏎/* c */ b();`: the comment leads `b()`
    /// and, being glued to a pure separator, shares its line
    /// ([`Printer::comment_hugs_next`]).
    ///
    /// ⚠️ **Not a replacement for its sibling — a third reading, and the two must not be
    /// swapped.** [`Self::statement_content_end`] counts a comment as CONTENT, which is what
    /// keeps a freeze slice verbatim and what keeps
    /// [`Printer::statement_content_tail_blank`]'s scan from crossing a comment's own
    /// interior newlines (`docs/comments.md` §The five hazards, hazard 5). This reading is
    /// for the comment CURSOR alone; the blank cursor and the freeze slice keep theirs.
    ///
    /// The measure is [`Self::trivia_run_start`] back from the `;`, whose alternating walk is
    /// what takes a whole run (`a()⏎/* c1 */ /* c2 */;`) out, as prettier's `stripComments`
    /// blanks every one of them before its single `trimEnd`. Its floor is the span's start,
    /// but what stops it is the first byte, walking back, that is neither whitespace nor the
    /// last byte of a comment, so a comment INSIDE the statement's content (`fn(/* c */)`) is
    /// never reached — the `)` ends the walk first.
    pub(in crate::printer) fn statement_comment_claim_end(
        &self,
        stmt: &internal::Statement<'_>,
    ) -> u32 {
        let span = stmt.span();
        if !self.statement_hands_off_terminator_gap(stmt) {
            return span.end;
        }
        let Some(content) = span.extract(self.source).strip_suffix(';') else {
            // ASI supplied the terminator: no authored `;`, so no trivia region to eject.
            return span.end;
        };
        self.trivia_run_start(span.start, span.start + content.len() as u32)
    }

    /// Where the run of TRIVIA ending at `end` begins — walking back over ECMAScript
    /// whitespace and whole comments alternately, never below `floor`.
    ///
    /// Prettier's `stripComments` + `trimEnd` pair, as a position: blanking every comment
    /// and then trimming is the same walk, and the alternation is what a RUN needs
    /// (`a()⏎/* c1 */ /* c2 */;` trims, steps over `c2`, trims again, steps over `c1`).
    ///
    /// The stopping byte is what makes this the right question at a terminator gap: a `)`
    /// the printer keeps is neither, so a comment the author wrote inside a retained shell
    /// (`return (x /* c */);`) is never ejected — the shell closes *after* it.
    ///
    /// Nearly every gap asked about holds no trivia at all, and two bytes say so without
    /// the trim or the comment search ([`Self::trivia_run_is_empty`]); the walk
    /// ([`Self::trivia_run_start_walk`]) runs only when one could be there.
    #[inline]
    pub(in crate::printer) fn trivia_run_start(&self, floor: u32, end: u32) -> u32 {
        if self.trivia_run_is_empty(floor, end) {
            debug_assert_eq!(
                self.trivia_run_start_walk(floor, end),
                end,
                "a gap the byte gate calls trivia-free must walk to its own end"
            );
            return end;
        }
        self.trivia_run_start_walk(floor, end)
    }

    /// Whether no trivia can END at `end`, so the run [`Self::trivia_run_start`] walks is
    /// empty and its answer is `end` itself.
    ///
    /// Two shapes, between them almost every terminator gap a statement has:
    ///
    /// - **`end == floor`** — nothing to walk (`a();` as [`Self::node_terminator_claim_end`]
    ///   asks it: the gap from `a()` to its `;` is empty).
    /// - **`end` holds a `;` glued to a byte that ends no trivia** ([`ends_no_trivia`]: an
    ///   ASCII graphic character other than `/`). That byte is not the last byte of a
    ///   whitespace character, and not the `/` every block comment closes on. A line comment
    ///   cannot end there either, which is what the `;` is for: a line comment runs to a line
    ///   terminator or to the end of the text its lexer read, so one reaching `end` would have
    ///   swallowed the `;` behind it. Every `end` that holds a `;` holds one a caller located
    ///   as a token of the same text: [`Self::statement_comment_claim_end`]'s, a span that ends
    ///   in its `;` at [`Self::node_terminator_claim_end`], and the `;` position a caller
    ///   passes there as the span end itself (the parenthesized `return` / `throw` binary,
    ///   whose `;` a comment-skipping scan found). The one other `end` is a statement's own
    ///   end under ASI — or, at that binary's ASI fallback, its argument's end, which is the
    ///   floor itself — and it never holds a `;` (one right there is a terminator the parser
    ///   would have taken), so the gate answers it only through `end == floor`.
    ///
    /// Anything else — whitespace, a `/`, a non-ASCII byte, a gap with no `;` behind it —
    /// takes the walk, which is exact for every input; this gate only ever answers where the
    /// walk provably returns `end` (asserted in debug builds).
    #[inline]
    fn trivia_run_is_empty(&self, floor: u32, end: u32) -> bool {
        if end == floor {
            return true;
        }
        let bytes = self.source.as_bytes();
        let end = end as usize;
        end > floor as usize && bytes.get(end) == Some(&b';') && ends_no_trivia(bytes[end - 1])
    }

    /// The walk behind [`Self::trivia_run_start`], exact for every gap — kept out of line,
    /// since only a gap that may hold trivia reaches it: **228 of 367,392 calls** (0.06%) in
    /// a format pass over a 2,877-file TypeScript corpus, every other one answered by the
    /// byte gate ([`Self::trivia_run_is_empty`]).
    #[cold]
    #[inline(never)]
    fn trivia_run_start_walk(&self, floor: u32, end: u32) -> u32 {
        let mut pos = end;
        loop {
            let before = Span::new(floor, pos).extract(self.source);
            let trimmed =
                before.trim_end_matches(|c| is_es_whitespace(c) || is_es_line_terminator(c));
            pos = floor + trimmed.len() as u32;
            match self
                .comments_in_source_between(floor, pos)
                .last()
                .filter(|c| c.span.end == pos)
            {
                Some(comment) => pos = comment.span.start,
                None => return pos,
            }
        }
    }
}
