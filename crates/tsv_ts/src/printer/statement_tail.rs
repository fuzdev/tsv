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

/// A statement whose terminator gap may take over its value's trailing region
/// ([`Printer::with_statement_tail`]).
#[derive(Clone, Copy)]
pub(in crate::printer) struct StatementTail {
    /// Where the value ends — the position a trailing region must close at to be the
    /// statement's.
    end: u32,
    /// Where the region handed to the terminator so far begins; `end` while nothing is.
    handed: u32,
}

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
    /// the two is outside it. Whitespace after a line comment's text is the comment's own, so
    /// the content then ends with the comment.
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
        let untrimmed = span.start + content.len() as u32;
        let content = content.trim_end_matches(|c| is_es_whitespace(c) || is_es_line_terminator(c));
        let trimmed = span.start + content.len() as u32;
        if trimmed == untrimmed {
            return trimmed;
        }
        // Whitespace after a line comment's text is the comment's own (its span runs to the
        // line terminator), so the trim can end inside it; the content ends with the comment
        // ([`Self::comment_ending_within`]).
        self.comment_ending_within(trimmed, untrimmed)
            .map_or(trimmed, |c| c.span.end)
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
    ///   end under ASI — that binary's too, where no `;` is found — and it never holds a `;`
    ///   (one right there is a terminator the parser would have taken), so the gate answers
    ///   it only through `end == floor`.
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
            let untrimmed = pos;
            let before = Span::new(floor, pos).extract(self.source);
            let trimmed =
                before.trim_end_matches(|c| is_es_whitespace(c) || is_es_line_terminator(c));
            pos = floor + trimmed.len() as u32;
            // A line comment whose text ends in whitespace straddles the trimmed `pos`
            // ([`Self::comment_ending_within`]), so the lookup reaches to `untrimmed`.
            match self
                .comment_ending_within(pos, untrimmed)
                .filter(|c| c.span.start >= floor)
            {
                Some(comment) => pos = comment.span.start,
                None => return pos,
            }
        }
    }

    /// Build a statement's VALUE with its trailing region open to the statement's terminator
    /// gap, returning the value's doc and where that gap now opens.
    ///
    /// The statement-terminator twin of [`Printer::member_gap_start`] (`docs/comments.md`
    /// §The right-spine shell run): a comment written last inside a grouping pair whose `)`
    /// ends the value (`x + (y /* c */);`, `v = (b /* c */)`, `x = a + (q + b /* c */);`)
    /// sits inside the value's span, yet nothing but `)`s and trivia stands between it and
    /// the terminator, and the reparse reads it from the terminator gap. So the gap opens
    /// where the value's PRINTED content ends: the region from there is claimed
    /// ([`Printer::claimed_right_spine_run`]) — the binary chain's trailing emitter declines
    /// it and its layout gate does not count it, so the chain lays out as its comment-free
    /// twin, which is what the reparse gives it — and the terminator's one emitter prints
    /// the whole gap in source order. That is also what keeps two comments in order: a block
    /// the value deferred out of its shell would flush behind one the terminator printed
    /// inline after the `;`.
    ///
    /// The region is opened two ways, one per emitter that would otherwise print it: here,
    /// down the value's right spine ([`Self::statement_tail_region`]) — a value no pair of the
    /// statement's encloses — and at a value SHELL whose `)` closes at the value's end
    /// ([`Self::shell_statement_tail_region`]), which alone knows whether its position keeps
    /// a pair. The value's end is the same for the `;` spelling and the ASI one, so the two
    /// format alike; `None` arms no tail, and shadows an enclosing statement's. Returns where
    /// the gap now opens, `None` with no tail armed.
    ///
    /// An expression statement whose value the statement does not wrap calls this; a
    /// variable declaration that prints its own terminator arms its tail around the
    /// declarator loop instead ([`Self::arm_statement_tail`]), the shells alone opening it.
    #[inline]
    pub(in crate::printer) fn with_statement_tail<T>(
        &self,
        value: Option<&internal::Expression<'_>>,
        build: impl FnOnce() -> T,
    ) -> (T, Option<u32>) {
        let outer = self.arm_statement_tail(value.map(|value| value.span().end));
        // Asked only of a tail that armed, so a region is never claimed for a terminator gap
        // that would not take it over.
        let region = value
            .zip(self.statement_tail.get())
            .and_then(|(value, tail)| self.statement_tail_region(value, tail.end));
        // One call site for `build`, so the value's whole build inlines once.
        let outer_claim = region.map(|region| self.open_statement_tail_claim(region));
        let built = build();
        if let Some(outer_claim) = outer_claim {
            self.claimed_right_spine_run.set(outer_claim);
        }
        (built, self.disarm_statement_tail(outer))
    }

    /// The arming half of [`Self::with_statement_tail`] with no walk, for a builder that
    /// cannot wrap its value in a closure (a declaration's declarator loop). Returns the
    /// enclosing tail, to hand back to [`Self::disarm_statement_tail`].
    #[inline]
    pub(in crate::printer) fn arm_statement_tail(&self, end: Option<u32>) -> Option<StatementTail> {
        let end = end.filter(|_| self.document_has_paren_closing_block());
        self.statement_tail
            .replace(end.map(|end| StatementTail { end, handed: end }))
    }

    /// [`Printer::has_paren_closing_block`], computed on first ask: a region a tail takes over
    /// ends in a same-line block followed by a shell's `)`, so a document holding no block
    /// followed by a `)` has none, and its statements arm no tail.
    #[inline]
    fn document_has_paren_closing_block(&self) -> bool {
        if let Some(answer) = self.has_paren_closing_block.get() {
            return answer;
        }
        let bytes = self.source.as_bytes();
        let answer = self.comments.iter().any(|c| {
            c.is_block
                && bytes[c.span.end as usize..]
                    .iter()
                    .find(|&&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
                    == Some(&b')')
        });
        self.has_paren_closing_block.set(Some(answer));
        answer
    }

    /// Restore the enclosing tail and report where this one's terminator gap opens
    /// (`None` when none was armed).
    #[inline]
    pub(in crate::printer) fn disarm_statement_tail(
        &self,
        outer: Option<StatementTail>,
    ) -> Option<u32> {
        self.statement_tail.replace(outer).map(|tail| tail.handed)
    }

    /// The trailing region of `expr` that a statement tail closing at `end` takes over:
    /// from where `expr`'s printed content ends ([`Printer::right_spine_printed_end_with`],
    /// stopping at a pair the printer adds) to `end`, when it holds a comment to emit.
    ///
    /// `None` when the region holds anything but same-line blocks, and when the value holds
    /// an honored format-ignore directive: a frozen slice prints its source verbatim, the
    /// region included, so claiming it too would print it twice. Asked only past the comment
    /// gate, so the scans cost nothing in the common case.
    #[inline]
    fn statement_tail_region(&self, expr: &internal::Expression<'_>, end: u32) -> Option<Span> {
        if !self.statement_tail_bytes_admit(end) {
            return None;
        }
        self.statement_tail_region_walk(expr, end)
    }

    /// The byte gate of [`Self::statement_tail_region`]: the region holds only `)`s,
    /// whitespace and same-line blocks, so one ends it — walking back from `end` over `)`s and
    /// whitespace reaches a `*/`, which almost no value's end does. A node's span ends at a
    /// token, so the region's last byte is a shell's `)` — the one byte almost every value
    /// fails on.
    #[inline]
    pub(in crate::printer) fn statement_tail_bytes_admit(&self, end: u32) -> bool {
        let bytes = self.source.as_bytes();
        let mut i = end as usize;
        if i == 0 || bytes[i - 1] != b')' {
            return false;
        }
        while i > 0 && matches!(bytes[i - 1], b')' | b' ' | b'\t' | b'\n' | b'\r') {
            i -= 1;
        }
        i >= 2 && bytes[i - 1] == b'/' && bytes[i - 2] == b'*'
    }

    /// [`Self::statement_tail_region`] past its byte gate, out of line.
    #[cold]
    #[inline(never)]
    fn statement_tail_region_walk(
        &self,
        expr: &internal::Expression<'_>,
        end: u32,
    ) -> Option<Span> {
        let start = self.right_spine_printed_end_with(expr, true);
        if start >= end || !self.has_comments_to_emit_between(start, end) {
            return None;
        }
        // Only a run of blocks on the value's own last line: a `//` or an own-line comment
        // keeps the deferral its own emitter gives it, which already lands past the `;` —
        // handed to the terminator it would open a line that a `//` the gap's next owner
        // defers behind it welds onto.
        let mut prev_end = start;
        let same_line_blocks = self.comments_to_emit_between(start, end).all(|c| {
            let glued = c.is_block && !self.comment_has_newline_between(prev_end, c.span.start);
            prev_end = c.span.end;
            glued
        });
        if !same_line_blocks {
            return None;
        }
        if self
            .comments_in_source_between(expr.span().start, end)
            .any(|c| self.is_honored_directive(c))
        {
            return None;
        }
        Some(Span::new(start, end))
    }

    /// Run `build` with `region` claimed for the statement tail: the value's emitters
    /// decline it, and the terminator gap opens no later than its start.
    pub(in crate::printer) fn with_statement_tail_claim<T>(
        &self,
        region: Span,
        build: impl FnOnce() -> T,
    ) -> T {
        let outer = self.open_statement_tail_claim(region);
        let built = build();
        self.claimed_right_spine_run.set(outer);
        built
    }

    /// Claim `region` for the statement tail ([`Self::with_statement_tail_claim`]) and return
    /// the claim it displaces, for the caller to restore.
    fn open_statement_tail_claim(&self, region: Span) -> Option<Span> {
        if let Some(mut tail) = self.statement_tail.get() {
            tail.handed = tail.handed.min(region.start);
            self.statement_tail.set(Some(tail));
        }
        self.claimed_right_spine_run.replace(Some(region))
    }

    /// Whether a value shell closing at `boundary_end` may hand its region to the enclosing
    /// statement's terminator gap ([`Self::with_statement_tail`]) — the cheap half of the
    /// question, inline: a tail ends there and the bytes admit a region. The rest is asked out
    /// of line by the shell's hand-over ([`Self::shell_statement_tail_region`]).
    #[inline]
    pub(in crate::printer) fn statement_tail_may_take(&self, boundary_end: u32) -> bool {
        self.statement_tail_ends_at(boundary_end) && self.statement_tail_bytes_admit(boundary_end)
    }

    /// The region a stripped value shell closing at `boundary_end` hands to the enclosing
    /// statement's terminator gap, past [`Self::statement_tail_may_take`] — to be claimed for
    /// the value's build ([`Self::with_statement_tail_claim`]): the shell does not keep its
    /// pair (`retains_parens` — a comment that needs the pair, or a pair the position prints
    /// anyway, holds the comment inside it), and the region from the value's printed end
    /// holds a run of same-line blocks.
    pub(in crate::printer) fn shell_statement_tail_region(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        retains_parens: bool,
    ) -> Option<Span> {
        if retains_parens {
            return None;
        }
        self.statement_tail_region_walk(expr, boundary_end)
    }

    /// Run `build` with no statement tail armed: a value the position wraps in a pair of its
    /// own keeps its trailing region inside that pair, out of the enclosing statement's
    /// terminator gap.
    #[inline]
    pub(in crate::printer) fn without_statement_tail<T>(&self, build: impl FnOnce() -> T) -> T {
        let outer = self.arm_statement_tail(None);
        let built = build();
        self.disarm_statement_tail(outer);
        built
    }

    /// Whether a statement tail is armed and ends at `end` — the one position a shell inside
    /// a node ending there could hand over to ([`Self::shell_statement_tail_region`]).
    #[inline]
    pub(in crate::printer) fn statement_tail_ends_at(&self, end: u32) -> bool {
        self.statement_tail
            .get()
            .is_some_and(|tail| tail.end == end)
    }
}
