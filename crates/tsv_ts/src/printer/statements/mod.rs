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
mod variable;

// Re-export for submodules to use `super::Printer` instead of `super::super::Printer`
pub(super) use super::{Printer, build_entity_name_doc, is_effectively_empty_body};

use super::LeadingGlue;
use super::ParenContext;
use super::class_expr_has_decorators;
use super::expressions::literals::format_directive;
use crate::ast::internal::{self, Expression, Statement};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a statement.
    ///
    /// `in_program_or_block` is Prettier's own-terms grandparent check for the
    /// "avoid becoming a directive" rule (see
    /// [`Printer::needs_avoid_directive_parens`]) — `true` when `statement`'s
    /// immediate container is a `Program` or `BlockStatement` (plain blocks,
    /// control-flow bodies, function/catch bodies), `false` for the containers
    /// that use a different AST node (`SwitchCase`, `StaticBlock`,
    /// `TSModuleBlock`). Only the `ExpressionStatement` arm consults it.
    pub(super) fn build_statement_doc(
        &self,
        statement: &Statement<'_>,
        in_program_or_block: bool,
    ) -> DocId {
        let d = self.d();
        match statement {
            Statement::ExpressionStatement(stmt) => {
                self.build_expression_statement_doc(stmt, in_program_or_block)
            }
            Statement::VariableDeclaration(decl) => self.build_variable_declaration_doc(decl, true),
            Statement::TSTypeAliasDeclaration(decl) => self.build_type_alias_declaration_doc(decl),
            Statement::ReturnStatement(ret) => self.build_return_statement_doc(ret),
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
            Statement::IfStatement(stmt) => self.build_if_statement_doc(stmt),
            Statement::ForStatement(stmt) => self.build_for_statement_doc(stmt),
            Statement::ForInStatement(stmt) => self.build_for_in_statement_doc(stmt),
            Statement::ForOfStatement(stmt) => self.build_for_of_statement_doc(stmt),
            Statement::WhileStatement(stmt) => self.build_while_statement_doc(stmt),
            Statement::DoWhileStatement(stmt) => self.build_do_while_statement_doc(stmt),
            Statement::SwitchStatement(stmt) => self.build_switch_statement_doc(stmt),
            Statement::TryStatement(stmt) => self.build_try_statement_doc(stmt),
            Statement::ThrowStatement(stmt) => self.build_throw_statement_doc(stmt),
            Statement::BreakStatement(stmt) => self.build_break_statement_doc(stmt),
            Statement::ContinueStatement(stmt) => self.build_continue_statement_doc(stmt),
            Statement::LabeledStatement(stmt) => self.build_labeled_statement_doc(stmt),
            Statement::EmptyStatement(_) => d.text(";"),
            Statement::DebuggerStatement(stmt) => {
                self.build_bare_keyword_terminator_doc("debugger", stmt.span)
            }
            Statement::TSInterfaceDeclaration(decl) => self.build_interface_declaration_doc(decl),
            Statement::TSDeclareFunction(decl) => self.build_declare_function_doc(decl),
            Statement::TSEnumDeclaration(decl) => self.build_enum_declaration_doc(decl),
            Statement::TSModuleDeclaration(decl) => self.build_module_declaration_doc(decl),
        }
    }

    /// Build a Doc for an expression statement: the value
    /// ([`Self::build_expression_statement_value_doc`], or the verbatim text of a
    /// directive-prologue entry) plus the `;` and the comments bound to it.
    fn build_expression_statement_doc(
        &self,
        stmt: &internal::ExpressionStatement<'_>,
        in_program_or_block: bool,
    ) -> DocId {
        let d = self.d();

        let mut parts: DocBuf = if stmt.is_directive {
            // Directives are exact code-unit sequences; `format_directive` mirrors
            // Prettier's `printDirective` (swap the outer quote to single only when
            // the content has no quote, else verbatim). Never parenthesized.
            let raw = stmt.expression.span().extract(self.source);
            smallvec![d.text_pooled(&format_directive(raw))]
        } else {
            smallvec![self.build_expression_statement_value_doc(stmt, in_program_or_block)]
        };

        // Comments between the expression and the `;`, with the `;` bound to the
        // statement: a same-line block trails *after* it (`fn() /* c */;` → `fn(); /* c */`,
        // prettier 3.9), a same-line line trails after it via `line_suffix`
        // (`fn() // c` → `fn(); // c`), an own-line comment drops to its own line after it
        // (emitting a line comment before the `;` would swallow it). See
        // `split_separator_gap_comments`.
        let expr_end = stmt.expression.span().end;
        let semicolon_pos = stmt.span.end.saturating_sub(1);
        self.push_semicolon_with_gap_comments(&mut parts, expr_end, semicolon_pos, true);
        d.concat(&parts)
    }

    /// The non-directive expression statement's VALUE — the expression plus whatever parens
    /// this print gives it (required, directive-avoiding, or a source pair it keeps) and the
    /// comments only those parens can emit. The statement's `;` and the comments bound to it
    /// stay with the caller.
    ///
    /// `in_program_or_block` is threaded from [`Printer::build_statement_doc`] for the
    /// "avoid becoming a directive" rule (see [`Printer::needs_avoid_directive_parens`]).
    fn build_expression_statement_value_doc(
        &self,
        stmt: &internal::ExpressionStatement<'_>,
        in_program_or_block: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
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
            .needs_parens(&stmt.expression, ParenContext::ExpressionStatement)
            || self.needs_avoid_directive_parens(stmt, in_program_or_block);

        // When the whole expression isn't wrapped, a nested leftmost
        // object/function/class still needs parens around itself
        // (`(class {}).foo`, `({}).foo`, `(class {}) + 1`).
        let nested_paren = if needs_parens {
            None
        } else {
            self.expr_stmt_nested_paren_target(&stmt.expression)
        };
        // A frozen slice is verbatim, so the printer has no interior left to wrap:
        // the nested target's parens go around the WHOLE slice instead. Without the
        // widening `({ a: 1 }.b)` would print as `{ a: 1 }.b;`, which reparses as a
        // block — prettier's own output there does exactly that.
        needs_parens |= frozen.is_some() && nested_paren.is_some();

        // The `(`→expression gap, resolved in one place so the gate below and the two
        // emitters that can claim it cannot read different ranges.
        let paren_gap =
            || comments_to_emit_in_range(self.comments, stmt.span.start + 1, expr_start);
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
            Some(span) => self.build_frozen_expression_doc(&stmt.expression, span),
            None => {
                self.expr_stmt_paren_target.set(nested_paren);
                self.is_expression_statement.set(true);
                self.in_top_level_assignment.set(true);
                let doc = self.build_expression_doc(&stmt.expression);
                self.in_top_level_assignment.set(false);
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
                d.empty(),
            );
            inner.push(expr_doc);
            parts.push(d.text("("));
            parts.push(d.indent(d.concat(&inner)));
            parts.push(d.hardline());
            parts.push(d.text(")"));
        } else if decorated_class_expr {
            parts.push(self.build_break_open_parens(expr_doc));
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
                    d.empty(),
                );
            }
            parts.push(expr_doc);
            if needs_parens {
                parts.push(d.text(")"));
            }
        }
        d.concat(&parts)
    }

    /// Build a Doc for a return statement.
    fn build_return_statement_doc(&self, ret: &internal::ReturnStatement<'_>) -> DocId {
        let Some(arg) = &ret.argument else {
            // No argument: a bare keyword closed by `;` (interior comments handled
            // there) — `return; /* c */` etc.
            return self.build_bare_keyword_terminator_doc("return", ret.span);
        };

        self.build_keyword_argument_doc("return", ret.span.start, ret.span.end, arg)
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
    /// `split_separator_gap_comments`: a same-line block trails after `;`
    /// (`debugger; /* c */`), a same-line line floats after `;` via `line_suffix`, an
    /// own-line comment drops to its own line (preceding blank line preserved). `span`
    /// is the full statement span — its end is the `;`, or the keyword end under ASI
    /// when there is no explicit `;` (then the interior range is empty).
    pub(in crate::printer::statements) fn build_bare_keyword_terminator_doc(
        &self,
        keyword: &'static str,
        span: Span,
    ) -> DocId {
        let d = self.d();
        let keyword_end = span.start + keyword.len() as u32;
        let semicolon_pos = span.end.saturating_sub(1);
        let mut parts: DocBuf = smallvec![d.text(keyword)];
        self.push_semicolon_with_gap_comments(&mut parts, keyword_end, semicolon_pos, true);
        d.concat(&parts)
    }
}
