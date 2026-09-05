// Statement printing for TypeScript — the per-kind DISPATCH plus the expression
// statement, which has no module of its own because its whole job is the value it
// wraps.
//
// The statement kinds with a shape of their own live in the submodules below:
// declarations in `variable` / `function` / `class` / `type_declarations`, modules in
// `modules`, control flow in `control_flow`. Two cross-cutting questions were split out
// because each is asked from several of those and answering it twice would let the
// answers drift: `head_parens` (which parens a statement's HEAD must keep or
// synthesize) and `restricted_production` (`return` / `throw` / `yield` and the
// ASI-safe parens their argument needs).

mod class;
mod control_flow;
pub(in crate::printer) mod function;
mod head_parens;
mod modules;
mod restricted_production;
mod type_declarations;
pub(in crate::printer) mod variable;

// Re-export for submodules to use `super::Printer` instead of `super::super::Printer`
pub(super) use super::{Printer, build_entity_name_doc, is_effectively_empty_body};

use super::LeadingGlue;
use super::ParenContext;
use super::RunLeadingBlank;
use super::class_expr_has_decorators;
use super::expressions::literals::format_directive;
use super::expressions::operators::SeqLayout;
use crate::ast::internal::{self, Expression, Statement};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Where a statement sits — the two container facts its builders read.
///
/// `in_program_or_block` is Prettier's own-terms grandparent check for the
/// "avoid becoming a directive" rule ([`Printer::needs_avoid_directive_parens`]) —
/// `true` when the statement's immediate container is a `Program` or
/// `BlockStatement`, `false` for the containers that use a different AST node
/// (`SwitchCase`, `StaticBlock`, `TSModuleBlock`) and for every clause body.
///
/// `clause_tail_dedent` is the `;`-terminator gap's deferral axis. In a statement
/// LIST the joiner's own break immediately follows the statement doc, so the gap's
/// own-line run can take real breaks — the emission is closed on its own line before
/// anything else can queue a `line_suffix` (`None`). A non-block CLAUSE body's last
/// line instead stays open to the enclosing construct — an `else`, a do-while
/// `while`, or just the construct's end — where a later gap's deferred `//` would
/// flush onto a real-text `//` and weld irreversibly (`x; // c1 // c2` reparses as
/// ONE comment). There the run defers through the suffix machinery
/// ([`Printer::push_semicolon_with_gap_comments`]), and the value is how many indent
/// levels sit between the statement's doc position and the break that flushes its
/// tail: each body site adds one per `indent` wrap it emits, and a construct that
/// CONTINUES on the tail's flush line (`else`, `while`) restarts the count at its own
/// gap ([`StatementContext::clause_body`]).
#[derive(Clone, Copy)]
pub(in crate::printer) struct StatementContext {
    pub(in crate::printer) in_program_or_block: bool,
    clause_tail_dedent: Option<u8>,
}

impl StatementContext {
    /// A statement joined into a `Program` or `BlockStatement` list.
    pub(in crate::printer) const PROGRAM_OR_BLOCK: Self = Self {
        in_program_or_block: true,
        clause_tail_dedent: None,
    };
    /// A statement list whose container is not one of those AST nodes
    /// (`SwitchCase`, `StaticBlock`, `TSModuleBlock`).
    pub(in crate::printer) const OTHER_LIST: Self = Self {
        in_program_or_block: false,
        clause_tail_dedent: None,
    };

    /// The context for a non-block clause BODY built from `self`'s position.
    ///
    /// `continues` — the construct emits more on the line the body's tail flushes at
    /// (an `if` with an `else`, a do-while's `while`), so the flush is that
    /// construct's own gap break; otherwise the tail falls through to whatever
    /// flushes `self`'s. `indented` — this body site wraps the body in one `indent`
    /// (prettier's `adjustClause`); an inline body (an `else`'s inline arm) adds
    /// none. The dedent must mirror the wraps exactly: a collapsed clause CHAIN
    /// stacks its indents even though no break renders them, and the freed comment
    /// settles at the flushing construct's level, not the innermost body's.
    pub(in crate::printer) fn clause_body(self, continues: bool, indented: bool) -> Self {
        let base = if continues {
            0
        } else {
            self.clause_tail_dedent.unwrap_or(0)
        };
        Self {
            in_program_or_block: false,
            clause_tail_dedent: Some(base.saturating_add(u8::from(indented))),
        }
    }

    /// The context for a labeled statement's BODY: a label adds no indent and ends
    /// with its body, so the body's tail is the label's tail — only the
    /// directive-prologue fact changes (a labeled string is never a directive).
    pub(in crate::printer) fn labeled_body(self) -> Self {
        Self {
            in_program_or_block: false,
            ..self
        }
    }

    /// The clause-tail deferral: `Some(dedent)` when the `;`-terminator gap's
    /// own-line run must ride in `line_suffix` docs, dedented this many levels.
    pub(in crate::printer) fn clause_tail(self) -> Option<u8> {
        self.clause_tail_dedent
    }
}

impl<'a> Printer<'a> {
    /// Build a Doc for a statement. `ctx` is the container's two facts
    /// ([`StatementContext`]); only the arms whose statement ends in a
    /// `;`-terminator gap (and the clause-bearing control flow that must thread it)
    /// consult it.
    pub(super) fn build_statement_doc(
        &self,
        statement: &Statement<'_>,
        ctx: StatementContext,
    ) -> DocId {
        let d = self.d();
        match statement {
            Statement::ExpressionStatement(stmt) => self.build_expression_statement_doc(stmt, ctx),
            Statement::VariableDeclaration(decl) => {
                self.build_variable_declaration_doc(decl, true, ctx.clause_tail())
            }
            Statement::TSTypeAliasDeclaration(decl) => {
                self.build_type_alias_declaration_doc(decl, ctx.clause_tail())
            }
            Statement::ReturnStatement(ret) => {
                self.build_return_statement_doc(ret, ctx.clause_tail())
            }
            // A statement-position block (bare `{ }`, a labeled block's body, or a
            // block nested directly in another block) expands its empty form to `{\n}`,
            // matching prettier. Only control-flow *bodies* (while/for/do/catch) and
            // function/class bodies collapse to `{}`, and those are built by their own
            // parents — never through this dispatch.
            Statement::BlockStatement(block) => self.build_block_statement_expand_empty_doc(block),
            Statement::FunctionDeclaration(decl) => self.build_function_declaration_doc(decl),
            Statement::ClassDeclaration(decl) => self.build_class_declaration_doc(decl),
            Statement::ExportNamedDeclaration(decl) => {
                self.build_export_named_declaration_doc(decl)
            }
            Statement::ExportDefaultDeclaration(decl) => {
                self.build_export_default_declaration_doc(decl)
            }
            Statement::ExportAllDeclaration(decl) => self.build_export_all_declaration_doc(decl),
            Statement::TSExportAssignment(decl) => self.build_export_assignment_doc(decl),
            Statement::TSNamespaceExportDeclaration(decl) => {
                self.build_namespace_export_declaration_doc(decl)
            }
            Statement::ImportDeclaration(decl) => self.build_import_declaration_doc(decl),
            Statement::TSImportEqualsDeclaration(decl) => {
                self.build_import_equals_declaration_doc(decl)
            }
            // Control flow statements - use simple doc building
            Statement::IfStatement(stmt) => self.build_if_statement_doc(stmt, ctx),
            Statement::ForStatement(stmt) => self.build_for_statement_doc(stmt, ctx),
            Statement::ForInStatement(stmt) => self.build_for_in_statement_doc(stmt, ctx),
            Statement::ForOfStatement(stmt) => self.build_for_of_statement_doc(stmt, ctx),
            Statement::WhileStatement(stmt) => self.build_while_statement_doc(stmt, ctx),
            Statement::DoWhileStatement(stmt) => self.build_do_while_statement_doc(stmt, ctx),
            Statement::SwitchStatement(stmt) => self.build_switch_statement_doc(stmt),
            Statement::TryStatement(stmt) => self.build_try_statement_doc(stmt),
            Statement::ThrowStatement(stmt) => {
                self.build_throw_statement_doc(stmt, ctx.clause_tail())
            }
            Statement::BreakStatement(stmt) => {
                self.build_break_statement_doc(stmt, ctx.clause_tail())
            }
            Statement::ContinueStatement(stmt) => {
                self.build_continue_statement_doc(stmt, ctx.clause_tail())
            }
            Statement::LabeledStatement(stmt) => self.build_labeled_statement_doc(stmt, ctx),
            Statement::EmptyStatement(_) => d.text(";"),
            Statement::DebuggerStatement(stmt) => {
                self.build_bare_keyword_terminator_doc("debugger", stmt.span, ctx.clause_tail())
            }
            Statement::TSInterfaceDeclaration(decl) => self.build_interface_declaration_doc(decl),
            Statement::TSDeclareFunction(decl) => {
                self.build_declare_function_doc(decl, ctx.clause_tail())
            }
            Statement::TSEnumDeclaration(decl) => self.build_enum_declaration_doc(decl),
            Statement::TSModuleDeclaration(decl) => {
                self.build_module_declaration_doc(decl, ctx.clause_tail())
            }
        }
    }

    /// Build a Doc for an expression statement: the value
    /// ([`Self::build_expression_statement_value_doc`], or the verbatim text of a
    /// directive-prologue entry) plus the `;` and the comments bound to it.
    fn build_expression_statement_doc(
        &self,
        stmt: &internal::ExpressionStatement<'_>,
        ctx: StatementContext,
    ) -> DocId {
        let d = self.d();

        let expr_end = stmt.expression.span().end;
        // The value reports which grouping `)` — if any — it retained and emitted the gap
        // up to, because that region is then the VALUE's share rather than the
        // terminator's. Asking the source a second time here instead would DROP whatever
        // the value declined to claim: the two must be one answer.
        let (value_doc, consumed_close) = if stmt.is_directive {
            (self.build_directive_doc(stmt), None)
        } else {
            self.build_expression_statement_value_doc(stmt, ctx.in_program_or_block)
        };

        // Comments between the expression and the `;`, with the `;` bound to the
        // statement: a same-line block trails *after* it (`fn() /* c */;` → `fn(); /* c */`,
        // prettier 3.9), a same-line line trails after it via `line_suffix`
        // (`fn() // c` → `fn(); // c`), an own-line comment drops to its own line after it
        // (emitting a line comment before the `;` would swallow it). See
        // `push_semicolon_with_gap_comments`.
        let gap_start = consumed_close.map_or(expr_end, |close| {
            Self::past_grouping_close(close, stmt.span.end)
        });
        if self.semicolon_gap_is_bare(gap_start, stmt.span.end) {
            return d.concat(&[value_doc, d.text(";")]);
        }
        let mut parts: DocBuf = smallvec![value_doc];
        self.push_semicolon_with_gap_comments(
            &mut parts,
            gap_start,
            stmt.span.end,
            true,
            ctx.clause_tail(),
        );
        d.concat(&parts)
    }

    /// A directive-prologue entry's VALUE — the literal's own source bytes, plus the
    /// comment glued to them.
    ///
    /// Directives are exact code-unit sequences; `format_directive` mirrors Prettier's
    /// `printDirective` (swap the outer quote to single only when the content has no
    /// quote, else verbatim). Never parenthesized, and never routed through
    /// `build_expression_doc` — `format_string_literal` re-quotes and re-escapes, which
    /// is precisely what a directive must not do.
    ///
    /// Which makes this a **reassembly** in the sense of `docs/comments.md` hazard 1: the
    /// glued block the parser bound to the string token (`owned_by_node`) is skipped by
    /// every gap emitter, so nothing else can print it and the claim belongs here. A
    /// directive is a bare string literal by grammar, so the literal's span start is the
    /// statement's first printed byte — the left-edge fact
    /// [`Printer::prepend_owned_leading_comment_at`] asks each caller for.
    fn build_directive_doc(&self, stmt: &internal::ExpressionStatement<'_>) -> DocId {
        let d = self.d();
        let span = stmt.expression.span();
        let text = d.text_pooled(&format_directive(span.extract(self.source)));
        self.prepend_owned_leading_comment_at(span.start, text)
    }

    /// The non-directive expression statement's VALUE — the expression plus whatever parens
    /// this print gives it (required, directive-avoiding, or a source pair it keeps) and the
    /// comments only those parens can emit. The statement's `;` and the comments bound to it
    /// stay with the caller.
    ///
    /// `in_program_or_block` is threaded from [`Printer::build_statement_doc`] for the
    /// "avoid becoming a directive" rule (see [`Printer::needs_avoid_directive_parens`]).
    ///
    /// Returns the doc and the grouping `)` this print RETAINED and emitted the
    /// expression→`)` gap inside, if any — the caller's terminator scan resumes past it.
    /// Reported rather than re-derived, because only the branch that ran knows whether it
    /// claimed that region; re-asking the source would drop what it declined to claim.
    fn build_expression_statement_value_doc(
        &self,
        stmt: &internal::ExpressionStatement<'_>,
        in_program_or_block: bool,
    ) -> (DocId, Option<u32>) {
        let d = self.d();
        // A `//` the author wrote inside the value's own grouping parens keeps those
        // parens: the terminator gap would defer it past the `)` and the `;`, onto a line
        // that may already hold a `//`, where the two MERGE into one comment. The
        // statement's other value positions (declarator initializer, assignment RHS,
        // ternary branch) already answer this way through the shared shell builder.
        let expr_end = stmt.expression.span().end;
        let shell_close = self.value_paren_line_comment_close(expr_end, stmt.span.end);
        // A comment between a source `(` and the expression (`(// c⏎ expr)` /
        // `(/* c */⏎ expr)` — e.g. a bare parenthesized decorated class
        // expression) is preserved inside the parens, breaking them open; the flat
        // `(`/`)` wrap below would drop it, since there is nowhere on one line for
        // a comment the expression doesn't own to go. prettier hoists the comment
        // before `(` — a divergence (`expression_statement_paren_kept_comment`,
        // `decorated_expr_open_paren_comment`).
        let expr_start = stmt.expression.span().start;
        let source_paren = self.statement_opens_with_paren(stmt.span, expr_start);
        // An own-line directive in that `(`→expression gap freezes the expression
        // (Rule A). Only a SOURCE paren opens the gap: without one the directive
        // leads the statement, where the statement list's own rule already claims it.
        let frozen = source_paren
            .then(|| self.value_head_frozen_span(stmt.span.start + 1, stmt.expression.span()))
            .flatten();

        // Parens required for correctness (object expressions, object pattern
        // assignments) OR to avoid a bare string statement being read as a
        // directive (recomputed fresh, not preserved from source).
        let mut needs_parens = self
            .needs_parens(stmt.expression, ParenContext::ExpressionStatement)
            || self.needs_avoid_directive_parens(stmt, in_program_or_block);

        // When the whole expression isn't wrapped, a nested leftmost
        // object/function/class still needs parens around itself
        // (`(class {}).foo`, `({}).foo`, `(class {}) + 1`).
        let nested_paren = if needs_parens {
            None
        } else {
            self.expr_stmt_nested_paren_target(stmt.expression)
        };
        // A frozen slice is verbatim, so the printer has no interior left to wrap:
        // the nested target's parens go around the WHOLE slice instead. Without the
        // widening `({ a: 1 }.b)` would print as `{ a: 1 }.b;`, which reparses as a
        // block — prettier's own output there does exactly that.
        needs_parens |= frozen.is_some() && nested_paren.is_some();

        // The `(`→expression gap, resolved in one place so the gate below and the two
        // emitters that can claim it cannot read different ranges.
        let paren_gap = || self.comments_to_emit_between(stmt.span.start + 1, expr_start);
        // Deliberately **to emit**, not on-page: this branch also *prints* the comments it
        // finds. A block glued to the *expression* is owned by it, rides inside its doc and
        // is skipped here — which is what keeps `(/* c */ expr)` flat.
        let paren_open_comments = needs_parens
            && source_paren
            && self.has_comments_to_emit_between(stmt.span.start + 1, expr_start);

        // Build the expression once — or not at all, when the freeze replaces its doc
        // with the verbatim slice. Context flags for chain handling:
        // is_expression_statement allows short identifier names to merge with the
        // first call; in_top_level_assignment selects the regular assignment
        // layout (not chain formatting). The matching node's doc builder consumes the
        // (non-consuming, span-matched) paren target and wraps itself; clear it
        // afterward so it can't leak into a sibling statement.
        let expr_doc = match frozen {
            Some(span) => self.build_frozen_expression_doc(stmt.expression, span),
            None => {
                self.expr_stmt_paren_target.set(nested_paren);
                self.is_expression_statement.set(true);
                // Saved rather than set: an expression statement nested inside this one's
                // expression (a function body) runs the same pair, and restoring the
                // constant `false` left the outer statement's remaining work in the wrong
                // context. Reached 16,391× over ~23k real files, neutral at every one — see
                // `build_variable_declaration_doc`, which carries the same pair.
                let prev_top_level_assignment = self.in_top_level_assignment.replace(true);
                let doc = match &stmt.expression {
                    // One of prettier's two `printSequenceExpression` parent arms (the `for`
                    // head is the other): the operands after the first take a continuation
                    // indent. Claimed here rather than in the expression dispatch because
                    // that is where the parent is known — a sequence nested deeper inside
                    // this statement is an ordinary operand and keeps the default layout.
                    Expression::SequenceExpression(seq) => {
                        self.build_sequence_doc(seq, SeqLayout::Indented)
                    }
                    // A continuation-indent position — the binaryish counterpart of the
                    // sequence arm above. A labeled statement's body IS an expression
                    // statement, so it rides here too.
                    expr => self.build_continuation_indent_expression_doc(expr),
                };
                self.in_top_level_assignment.set(prev_top_level_assignment);
                self.is_expression_statement.set(false);
                self.expr_stmt_paren_target.set(None);
                doc
            }
        };

        // A parenthesized *decorated* class expression breaks its parens open and
        // indents the content (prettier): `(⏎\t@dec⏎\tclass {}⏎)`. The decorators
        // force the break; an undecorated `(class {})` / `(function () {})` stays
        // inline (flat `else` below).
        let decorated_class_expr = needs_parens
            && matches!(
                &stmt.expression,
                Expression::ClassExpression(c) if class_expr_has_decorators(c)
            );

        // Which grouping `)` this print retains and emits the expression→`)` gap inside.
        // The two paren-KEEPING branches below claim it; the decorated one does not (its
        // layout has no seam for a trailing run), so there the terminator gap keeps it.
        // The plain statement — no required, directive-avoiding, or source paren — IS its
        // expression: the branch below would push it alone and `concat` would hand it
        // back, so the buffer is skipped outright (the common case by a wide margin).
        if !needs_parens && !source_paren && shell_close.is_none() {
            return (expr_doc, None);
        }
        let mut parts = DocBuf::new();
        let mut consumed_close = None;
        if paren_open_comments {
            // The parens break open around the run. Only the first `hardline` is
            // the site's own — the run's internal separators come from the shared
            // leading-comment emitter, so a run the author glued onto one line
            // stays glued and a blank line between two comments survives.
            let mut inner: DocBuf = smallvec![d.hardline()];
            self.push_leading_comment_run(
                &mut inner,
                paren_gap(),
                expr_start,
                LeadingGlue::Adjacent,
            );
            inner.push(expr_doc);
            // The parens are already open, so a `//` before the `)` stays inside them
            // rather than deferring past the `;` — the same claim the retained-shell
            // branch below makes, at the branch that got here for the leading run.
            consumed_close = self.push_retained_gap_trailing_run(&mut inner, expr_end, shell_close);
            parts.push(d.text("("));
            parts.push(d.indent(d.concat(&inner)));
            parts.push(d.hardline());
            parts.push(d.text(")"));
        } else if decorated_class_expr {
            parts.push(self.build_break_open_parens(expr_doc));
        } else if let Some(close) = shell_close {
            // The value's authored parens are RETAINED around a `//` that would otherwise
            // escape them. This pair is the position's pair — a shell whose `(` leads the
            // statement already discharges what `needs_parens` wraps for, so adding the
            // clarity pair too would double it. The leading run is the plain branch's,
            // unchanged: a dropped source `(` is what strands it, and here the `(` is
            // kept, so only the expression's own owned comments ride inside `expr_doc`.
            let mut inner = DocBuf::new();
            if !needs_parens && source_paren {
                self.push_leading_comment_run(
                    &mut inner,
                    paren_gap(),
                    expr_start,
                    LeadingGlue::Adjacent,
                );
            }
            inner.push(expr_doc);
            consumed_close = self.push_retained_gap_trailing_run(&mut inner, expr_end, Some(close));
            parts.push(d.text("("));
            parts.push(d.indent_hardline(d.concat(&inner)));
            parts.push(d.hardline());
            parts.push(d.text(")"));
        } else {
            if needs_parens {
                parts.push(d.text("("));
            } else if source_paren {
                // The statement had a source `(` that this print DROPS as
                // redundant. A comment inside it then has no emitter left: the
                // statement-list gap ends at the `(`, and the expression's own doc
                // starts past the comment — so it must be emitted here or it is
                // lost. It leads the statement, which is where prettier hoists it
                // too. (An owned/glued comment rides inside `expr_doc` and is
                // skipped by the emit iterator; the paren-KEPT cases are the two
                // branches above.)
                self.push_leading_comment_run(
                    &mut parts,
                    paren_gap(),
                    expr_start,
                    LeadingGlue::Adjacent,
                );
            }
            parts.push(expr_doc);
            if needs_parens {
                parts.push(d.text(")"));
            }
        }
        (d.concat(&parts), consumed_close)
    }

    /// Emit the expression→`)` gap's trailing run inside a retained grouping pair, and
    /// report the `)` claimed — `None` when there is nothing to retain, which leaves the
    /// gap to the caller's terminator scan.
    ///
    /// One seam so the two paren-keeping branches of
    /// [`Self::build_expression_statement_value_doc`] cannot claim different ranges: a
    /// region claimed here and re-read there is a DOUBLE-PRINT, and one claimed at
    /// neither is a DROP.
    fn push_retained_gap_trailing_run(
        &self,
        inner: &mut DocBuf,
        expr_end: u32,
        close: Option<u32>,
    ) -> Option<u32> {
        let close = close?;
        self.push_anchored_trailing_run(inner, expr_end, close, RunLeadingBlank::Keep);
        Some(close)
    }

    /// Build a Doc for a return statement. `clause_tail` reaches both terminator
    /// gaps — the bare form's keyword→`;` gap and the argument form's operand→`;`
    /// gap (threaded through the restricted production's own emitters).
    fn build_return_statement_doc(
        &self,
        ret: &internal::ReturnStatement<'_>,
        clause_tail: Option<u8>,
    ) -> DocId {
        let Some(arg) = &ret.argument else {
            // No argument: a bare keyword closed by `;` (interior comments handled
            // there) — `return; /* c */` etc.
            return self.build_bare_keyword_terminator_doc("return", ret.span, clause_tail);
        };

        self.build_keyword_argument_doc("return", ret.span.start, ret.span.end, arg, clause_tail)
    }

    /// Build a Doc for a "bare" keyword-terminator statement — a keyword that takes
    /// no operand and is closed by `;`: `debugger`, the no-arg `return`, and a
    /// label-less `break`/`continue`.
    ///
    /// None has a `[no LineTerminator]` issue at this point (the operand/label is
    /// absent), so when an explicit `;` follows on a later line the parser scans
    /// forward to it and the `;` becomes the statement's terminator — any comment
    /// between the keyword and that `;` sits *inside* the statement span (e.g.
    /// `debugger\n\n// c\n;` → span swallows `// c` and the `;`). Emitting just
    /// `keyword;` would drop them. Route the interior gap through
    /// `push_semicolon_with_gap_comments`: a same-line block trails after `;`
    /// (`debugger; /* c */`), a same-line line floats after `;` via `line_suffix`, an
    /// own-line comment drops to its own line (preceding blank line preserved). `span`
    /// is the full statement span — its end is the `;`, or the keyword end under ASI
    /// when there is no explicit `;` (then the interior range is empty).
    pub(in crate::printer::statements) fn build_bare_keyword_terminator_doc(
        &self,
        keyword: &'static str,
        span: Span,
        clause_tail: Option<u8>,
    ) -> DocId {
        let d = self.d();
        let keyword_end = span.start + keyword.len() as u32;
        let mut parts: DocBuf = smallvec![d.text(keyword)];
        self.push_semicolon_with_gap_comments(&mut parts, keyword_end, span.end, true, clause_tail);
        d.concat(&parts)
    }
}
