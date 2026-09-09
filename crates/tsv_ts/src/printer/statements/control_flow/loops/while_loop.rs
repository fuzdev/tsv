// while, with and do-while statement printing
//
// Condition-group layout and body handling for while/with/do-while, including the
// do-while comment-preservation divergence from Prettier.

use super::super::{HeadChainGrouping, OpenParenLineBlockComment};
use crate::ast::internal::{self, Statement};
use crate::printer::Printer;
use crate::printer::statements::StatementContext;
use smallvec::smallvec;
use tsv_lang::doc::arena::DocId;

/// What parts `with` from `while` in the layout they otherwise share: the keyword, whether
/// an empty block body collapses (prettier's parent allowlist in `print/block.js`), and how
/// a logical head chain groups (prettier's `isInsideParenthesis`, [`HeadChainGrouping`]).
#[derive(Clone, Copy)]
struct ParenHeadKind {
    keyword: &'static str,
    collapses_empty_block: bool,
    grouping: HeadChainGrouping,
}

impl ParenHeadKind {
    const WHILE: Self = Self {
        keyword: "while",
        collapses_empty_block: true,
        grouping: HeadChainGrouping::ParenGroupDrives,
    };
    const WITH: Self = Self {
        keyword: "with",
        collapses_empty_block: false,
        grouping: HeadChainGrouping::ChainGroupsItself,
    };
}

impl<'a> Printer<'a> {
    /// Build a doc for a while statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the condition wraps to multiple lines
    /// when the `while (condition)` line exceeds print width.
    pub(in crate::printer::statements) fn build_while_statement_doc(
        &self,
        stmt: &internal::WhileStatement<'_>,
        ctx: StatementContext,
    ) -> DocId {
        self.build_paren_head_statement_doc(
            ParenHeadKind::WHILE,
            stmt.span.start,
            stmt.test,
            stmt.body,
            ctx,
        )
    }

    /// Build a doc for a `with` statement — `while`'s layout, which is also how prettier
    /// prints it: one printer serves both, keying the keyword on the node type
    /// (`print/while-statement.js`).
    ///
    /// Two axes part them, both carried by [`ParenHeadKind`]:
    ///
    /// - the EMPTY BLOCK body — prettier's block printer collapses `{}` only for a listed
    ///   set of parents, and `WithStatement` is not in it, so `with (o) {}` prints
    ///   `with (o) {⏎}` where `while (c) {}` stays collapsed (`print/block.js`);
    /// - the HEAD CHAIN grouping — `WithStatement` is not on prettier's
    ///   `isInsideParenthesis` parent list, so a logical head chain groups itself instead
    ///   of handing its breaking to the head's paren group ([`HeadChainGrouping`],
    ///   `print/binaryish.js`).
    pub(in crate::printer::statements) fn build_with_statement_doc(
        &self,
        stmt: &internal::WithStatement<'_>,
        ctx: StatementContext,
    ) -> DocId {
        self.build_paren_head_statement_doc(
            ParenHeadKind::WITH,
            stmt.span.start,
            stmt.object,
            stmt.body,
            ctx,
        )
    }

    /// The layout `while` and `with` share: a `keyword (head)` group, then the body in
    /// one of three arms (block / empty statement / `adjustClause`).
    ///
    /// [`ParenHeadKind`] carries the three facts the two constructs part on.
    fn build_paren_head_statement_doc(
        &self,
        kind: ParenHeadKind,
        stmt_start: u32,
        head: &internal::Expression<'_>,
        body: &Statement<'_>,
        ctx: StatementContext,
    ) -> DocId {
        let d = self.d();
        // The head every arm below shares, built once — the same one the `if` printer
        // takes, so the paren-headed statements cannot drift apart.
        let (mut parts, paren_end) =
            self.build_paren_condition_head(kind.keyword, stmt_start, head, kind.grouping);

        if let Statement::BlockStatement(block) = body {
            // Block body: while (cond) { ... }
            // Uses append_close_paren_with_comments for consistency with if/for-in/for-of:
            // block comments stay inline, line comments become trailing.
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_statement_head_doc(paren_end, body, || {
                if kind.collapses_empty_block {
                    self.build_block_statement_doc(block)
                } else {
                    self.build_block_statement_expand_empty_doc(block)
                }
            }));
            d.group(d.concat(&parts))
        } else if matches!(body, Statement::EmptyStatement(_)) {
            // Empty statement: `while (cond);` or `while (cond) /* comment */ ;`
            let empty_start = body.span().start;
            self.append_close_paren_empty_stmt_with_comments(&mut parts, paren_end, empty_start);
            d.group(d.concat(&parts))
        } else {
            // Non-block body: use adjustClause equivalent
            // - When flat: line becomes space -> `while (cond) a;`
            // - When broken: line becomes newline + indent -> `while (cond)\n\ta;`
            let body_start = body.span().start;
            let body_doc = self.build_statement_head_doc(paren_end, body, || {
                // `adjustClause` wraps the body in one indent in every arm; nothing
                // continues on the tail's line, so the tail falls through to whatever
                // flushes the statement's own position.
                self.build_statement_doc(body, ctx.clause_body(false, true))
            });
            self.build_adjust_clause_with_comments(&parts, paren_end, body_start, body_doc)
        }
    }

    pub(in crate::printer::statements) fn build_do_while_statement_doc(
        &self,
        stmt: &internal::DoWhileStatement<'_>,
        ctx: StatementContext,
    ) -> DocId {
        let d = self.d();
        let is_block = matches!(stmt.body, Statement::BlockStatement(_));

        // Check for comments between `do` keyword and body
        let do_end = stmt.span.start + "do".len() as u32;
        let body_start = stmt.body.span().start;
        // The body's container facts: the `while` CONTINUES on the tail's flush line,
        // and only the broken header→body arm below wraps the body in an indent — the
        // plain `do body` spelling is inline, so its tail dedents zero levels.
        let gap_has_comments = self.has_comments_to_emit_between(do_end, body_start);
        let gap_breaks_body =
            gap_has_comments && self.header_to_body_gap_breaks(do_end, body_start) && !is_block;
        let body_ctx = ctx.clause_body(true, gap_breaks_body);
        // A loop body collapses its empty block form (`do {} while (cond)`) — unless an
        // own-line directive in the `do`→body gap freezes it.
        let body_doc = self.build_statement_head_doc(do_end, stmt.body, || {
            self.build_collapsing_body_doc(stmt.body, body_ctx)
        });

        let mut parts = if gap_has_comments {
            let mut p = smallvec![d.text("do")];
            if gap_breaks_body {
                // Non-block body whose run breaks: the run shares the body's indent, with
                // a `//` normalized onto its own line — prettier does the same here, so
                // there is nothing to preserve.
                self.push_indented_header_to_body_gap(&mut p, do_end, body_start, body_doc);
            } else {
                // Everything else — the shared header→body gap, exactly as `try`/`catch`/
                // `finally` use it: a comment trailing `do` stays trailing, one on its own
                // line keeps it. Emitting the run inline after a bare space relocated an
                // own-line comment up onto the `do` line (`do // c⏎{`), the same defect
                // the `try` family had; prettier preserves here, so it is a clean oracle
                // rather than a stance.
                //
                // A non-breaking run (block comments trailing `do`) took a hand-rolled
                // third arm here until it was measured against this one: that arm's
                // `" "` + run + `" "` is exactly what this emitter emits when
                // `header_to_body_gap_breaks` is false, so it was a second spelling of
                // this gap's own no-break case rather than a case this one lacks.
                self.push_header_to_body_gap(&mut p, do_end, body_start);
                p.push(body_doc);
            }
            p
        } else if matches!(stmt.body, Statement::EmptyStatement(_)) {
            // Prettier's `adjustClause` returns `";"` directly for an empty body
            // → `do;`, not `do ;`.
            smallvec![d.text("do"), body_doc]
        } else {
            smallvec![d.text("do "), body_doc]
        };

        // Find the while keyword position for comment handling
        // Search forward from body end, skipping over comments to find the actual keyword
        let body_end = stmt.body.span().end;
        let test_start = stmt.test.span().start;
        let while_pos = self.find_keyword_in_range(body_end, test_start, "while");

        // The `}`→`while` gap: its comments and the separator before the keyword.
        // Emitted here, ahead of the paren bookkeeping below, which computes without
        // pushing; the `while` keyword itself follows it.
        if let Some(while_start) = while_pos {
            self.push_block_to_keyword_gap(
                &mut parts,
                body_end,
                while_start,
                is_block,
                self.terminator_defers_comment(stmt.body.span().start, body_end),
            );
        } else {
            parts.push(if is_block { d.text(" ") } else { d.hardline() });
        }

        // Find paren positions for comment handling
        let open_paren = while_pos.and_then(|p| self.find_open_paren_after(p));
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `while` keyword and `(` in place:
        //   do{}while/* c */(a); → do {} while /* c */ (a);
        let keyword_comments = while_pos.and_then(|wp| {
            self.build_keyword_paren_comments(wp + "while".len() as u32, open_paren)
        });

        // The keyword is pushed here rather than by `push_keyword_open_paren`: the
        // `}`→`while` gap above already owns everything ahead of it.
        parts.push(d.text("while"));
        self.push_open_paren_after_keyword(&mut parts, keyword_comments);

        // The same entry point `if` / `while` use, as in prettier (whose
        // `printDoWhileStatementCondition` is `printIfStatementCondition` under another
        // name): the width-driven condition group, the `!(…)` negation hug, and the
        // clarity parens an assignment takes (`do {} while ((x = y))`). The one axis
        // that parts do-while from the rest is the `(`-line comment policy — a comment
        // after `while (` stays in place, where prettier relocates it outside the parens
        // (`OpenParenLineBlockComment::Preserve`).
        parts.push(self.build_statement_condition_doc(
            stmt.test,
            open_paren,
            close_paren,
            OpenParenLineBlockComment::Preserve,
            HeadChainGrouping::ParenGroupDrives,
        ));

        // Comments between the condition's `)` and the do-while's terminating `;`,
        // with the `;` bound to the statement: a same-line block trails *after* it
        // (`} while (x) /* c */;` → `} while (x); /* c */`, prettier 3.9), a same-line
        // line via `line_suffix`, an own-line comment on its own line after. (Unlike
        // an empty *body* `;` — `if (a) /* c */ ;` — which keeps the comment inline;
        // the do-while `;` is the statement terminator.) See
        // `push_semicolon_with_gap_comments`.
        if let Some(close) = close_paren {
            parts.push(d.text(")"));
            self.push_semicolon_with_gap_comments(
                &mut parts,
                close + 1,
                stmt.span.end,
                true,
                ctx.clause_tail(),
            );
        } else {
            parts.push(d.text(");"));
        }
        d.concat(&parts)
    }
}
