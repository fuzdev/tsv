// Statement printing for TypeScript
//
// Handles printing of different statement types:
// - Expression statements (expression followed by semicolon)
// - Variable declarations (const, let, var)
// - Type-related statements (type alias, return)
// - Function and class declarations
// - Import/export statements
// - Control flow (if, for, while, switch, try, etc.)

mod class;
mod control_flow;
mod function;
mod modules;
mod type_declarations;
mod variable;

// Re-export for submodules to use `super::Printer` instead of `super::super::Printer`
pub(super) use super::{Printer, build_entity_name_doc, is_effectively_empty_body};

use super::ParenContext;
use super::class_expr_has_decorators;
use super::expressions::literals::format_directive;
use super::is_string_literal;
use super::needs_parens::leftmost_no_lookahead;
use crate::ast::internal::{self, Expression, Statement};
use crate::printer::analysis::has_newline_after_position;
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

/// Strip only `as`/`satisfies` casts from the head of a statement expression,
/// returning the innermost operand — but only if at least one cast was peeled.
/// Mirrors prettier's `ancestorNeitherAsNorSatisfies` walk
/// (parentheses/identifier.js): unlike `leftmost_no_lookahead` it does NOT descend
/// through member/call heads (`type.foo` is unambiguous), so it fires only for a
/// bare-identifier operand of a cast chain.
fn strip_statement_casts<'a>(expr: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let mut cur = expr;
    let mut stripped = false;
    loop {
        cur = match cur {
            Expression::TSAsExpression(e) => e.expression,
            Expression::TSSatisfiesExpression(e) => e.expression,
            _ => break,
        };
        stripped = true;
    }
    stripped.then_some(cur)
}

/// Contextual-keyword identifier names whose **bare** `<kw> as T` / `<kw> satisfies T`
/// at statement position tsv's parser *rejects* — it commits to a declaration reading
/// (`type <name> = …` alias, `module <name> { … }` namespace) and errors when no
/// `=`/`{` follows. Dropping the source parens on `(type) as T` would make the output
/// unreparseable, so the parens are kept.
///
/// This is deliberately tsv's reject-set, NOT prettier's full identifier list
/// (parentheses/identifier.js also lists `await`/`interface`/`yield`/`let`/`component`/
/// `hook`, which tsv's parser rejects even bare-as-an-expression, so they never reach
/// the formatter). `using` is excluded on purpose: tsv **accepts** bare `using as T`
/// (a cast, per its acorn oracle) and keeps it bare — a deliberate divergence pinned by
/// `typescript_specific/using/cast_prettier_divergence`. Wrapping it would break that.
fn is_statement_ambiguous_keyword(name: &str) -> bool {
    matches!(name, "type" | "module")
}

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

    /// Build a Doc for an expression statement
    ///
    /// Handles parentheses for object patterns and the "avoid becoming a
    /// directive" rule for bare string literals (see
    /// [`Printer::needs_avoid_directive_parens`]); `in_program_or_block` is
    /// threaded from [`Printer::build_statement_doc`].
    fn build_expression_statement_doc(
        &self,
        stmt: &internal::ExpressionStatement<'_>,
        in_program_or_block: bool,
    ) -> DocId {
        let d = self.d();

        let mut parts = DocBuf::new();

        if stmt.is_directive {
            // Directives are exact code-unit sequences; `format_directive` mirrors
            // Prettier's `printDirective` (swap the outer quote to single only when
            // the content has no quote, else verbatim). Never parenthesized.
            let raw = stmt.expression.span().extract(self.source);
            parts.push(d.text_pooled(&format_directive(raw)));
        } else {
            // Parens required for correctness (object expressions, object pattern
            // assignments) OR to avoid a bare string statement being read as a
            // directive (recomputed fresh, not preserved from source).
            let needs_parens = self
                .needs_parens(&stmt.expression, ParenContext::ExpressionStatement)
                || self.needs_avoid_directive_parens(stmt, in_program_or_block);

            // An own-line comment between a source `(` and the expression
            // (`(// c⏎ expr)` / `(/* c */⏎ expr)` — e.g. a bare parenthesized
            // decorated class expression) is preserved inside the parens, breaking
            // them open; the flat `(`/`)` wrap below would drop it. Own-line = a line
            // comment (never inline) or a block comment with a newline before it; a
            // same-line block comment (`(/* c */ expr)`) is a separate, rarer case
            // (inline) left to the default flow. `stmt.span.start < expr_start` means
            // a real source `(` precedes the expression. prettier hoists the comment
            // before `(` — a divergence (`decorated_expr_open_paren_comment`).
            // TODO: a same-line block comment after `(` is still dropped here.
            let expr_start = stmt.expression.span().start;
            // Deliberately **to emit**, not on-page: this branch also *prints* the comments it
            // finds, and the non-owned path here already drops them (`(/* c */ fn());` loses the
            // comment — the ledger reports it). Moving it to the layout axis before that is
            // fixed would route an owned comment into a path that loses it.
            let paren_open_own_line_comment = needs_parens
                && stmt.span.start < expr_start
                && comments_to_emit_in_range(self.comments, stmt.span.start + 1, expr_start)
                    .any(|c| self.is_own_line_comment(c));

            // When the whole expression isn't wrapped, a nested leftmost
            // object/function/class still needs parens around itself
            // (`(class {}).foo`, `({}).foo`, `(class {}) + 1`). The matching node's
            // doc builder consumes this span-matched target and wraps itself.
            if !needs_parens {
                let leftmost = leftmost_no_lookahead(&stmt.expression);
                if matches!(
                    leftmost,
                    Expression::ObjectExpression(_)
                        | Expression::FunctionExpression(_)
                        | Expression::ClassExpression(_)
                ) {
                    self.expr_stmt_paren_target.set(Some(leftmost.span()));
                } else if let Some(Expression::Identifier(id)) =
                    strip_statement_casts(&stmt.expression)
                    && self.with_ident_name(id, is_statement_ambiguous_keyword)
                {
                    // `(type) as T;` / `(module) satisfies U;` — a contextual keyword
                    // heading an `as`/`satisfies` cast at statement level reparses as a
                    // `type`/`module`/… declaration without the parens. The identifier's
                    // doc builder consumes this span-matched target and wraps itself.
                    self.expr_stmt_paren_target.set(Some(id.span));
                }
            }

            // Build the expression once. Context flags for chain handling:
            // is_expression_statement allows short identifier names to merge with the
            // first call; in_top_level_assignment selects the regular assignment
            // layout (not chain formatting). Clear the (non-consuming, span-matched)
            // paren target afterward so it can't leak into a sibling statement.
            self.is_expression_statement.set(true);
            self.in_top_level_assignment.set(true);
            let expr_doc = self.build_expression_doc(&stmt.expression);
            self.in_top_level_assignment.set(false);
            self.is_expression_statement.set(false);
            self.expr_stmt_paren_target.set(None);

            // A parenthesized *decorated* class expression breaks its parens open and
            // indents the content (prettier): `(⏎\t@dec⏎\tclass {}⏎)`. The decorators
            // force the break; an undecorated `(class {})` / `(function () {})` stays
            // inline (flat `else` below).
            let decorated_class_expr = needs_parens
                && matches!(
                    &stmt.expression,
                    Expression::ClassExpression(c) if class_expr_has_decorators(c)
                );

            if paren_open_own_line_comment {
                let mut inner: DocBuf = smallvec![d.hardline()];
                for comment in
                    comments_to_emit_in_range(self.comments, stmt.span.start + 1, expr_start)
                {
                    inner.push(self.build_comment_doc(comment));
                    inner.push(d.hardline());
                }
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
                }
                parts.push(expr_doc);
                if needs_parens {
                    parts.push(d.text(")"));
                }
            }
        }

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

    /// Whether a bare string-literal expression statement needs synthetic parens
    /// to avoid being read as a directive-prologue entry.
    ///
    /// Mirrors Prettier's `needs-parentheses.js` `StringLiteral`/`Literal` case:
    /// recomputed fresh from AST structure, never preserved from source. A
    /// non-directive string statement gets parens exactly when its immediate
    /// container is a `Program` or `BlockStatement` (`in_program_or_block`) —
    /// plain blocks, `if`/`for`/`while`/`try`/`catch` bodies, and function/arrow/
    /// method bodies all qualify; `SwitchCase`, `StaticBlock`, and
    /// `TSModuleBlock` (namespace) bodies don't. Because this is recomputed
    /// rather than preserved, redundant source parens are stripped in an
    /// ineligible container (`static { ('x'); }` → `'x';`) and any number of
    /// source parens collapse to exactly one where they're needed.
    ///
    /// Only called from the `!stmt.is_directive` branch of
    /// `build_expression_statement_doc`, so a real directive never reaches here.
    fn needs_avoid_directive_parens(
        &self,
        stmt: &internal::ExpressionStatement<'_>,
        in_program_or_block: bool,
    ) -> bool {
        in_program_or_block && is_string_literal(&stmt.expression)
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

    /// Shared dispatch for return/throw argument formatting.
    ///
    /// Matches Prettier's `printReturnOrThrowArgument` (function.js:231-277):
    /// 1. Assignment expressions → unconditional parens: `return (a = b);`
    /// 2. Own-line comments in chain → unconditional parens
    /// 3. Binaryish arguments → conditional parens (ifBreak)
    /// 4. Otherwise → plain `keyword expr;`
    fn build_keyword_argument_doc(
        &self,
        keyword: &'static str,
        keyword_start: u32,
        span_end: u32,
        arg: &Expression<'_>,
    ) -> DocId {
        let d = self.d();

        let keyword_end = keyword_start + keyword.len() as u32;

        // Trailing comments from stripped grouping parens: `return (x /* c */)` → `return x /* c */;`
        let argument_end = arg.span().end;
        let has_trailing_comments = self.has_comments_to_emit_between(argument_end, span_end);

        // A comment that must break takes the parenthesized form, which is what makes the
        // break legal; there the comment keeps the line the author gave it.
        if self.argument_has_own_line_comment(keyword_start, arg) {
            let own_line_comments = self.build_rhs_comments_opt(keyword_end, arg.span().start);
            return self.build_comment_paren_doc(keyword, arg, span_end, own_line_comments);
        }

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
            // #19263 — operand-attached), `throw` floats it OUT after `)`
            // (`throw (a = b) /* c */;`).
            if keyword == "return" {
                let after = if has_trailing_comments {
                    self.split_terminator_gap_comments(&mut parts, argument_end, span_end, false)
                } else {
                    DocBuf::new()
                };
                parts.push(d.text(")"));
                parts.push(d.text(";"));
                parts.extend(after);
            } else {
                parts.push(d.text(")"));
                if has_trailing_comments {
                    self.append_trailing_paren_comments(&mut parts, argument_end, span_end);
                }
                parts.push(d.text(";"));
            }
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
            // node); a trailing comment before it stays inside the parens.
            let grouping_close = find_char_skipping_comments(
                self.source.as_bytes(),
                argument_end as usize,
                span_end as usize,
                b')',
            )
            .map_or(argument_end, |p| p as u32);
            let seq_doc = self.build_sequence_doc_value(seq, grouping_close);
            let mut parts: DocBuf = if let Some(comments_doc) = inline_comments {
                smallvec![d.text(keyword), d.text(" "), comments_doc, seq_doc]
            } else {
                smallvec![d.text(keyword), d.text(" "), seq_doc]
            };
            // Any comment AFTER the grouping `)` (before the `;`) trails after the `;`;
            // the in-paren comment is already inside `seq_doc`.
            let after_start = grouping_close.saturating_add(1).min(span_end);
            let after = if self.has_comments_to_emit_between(after_start, span_end) {
                self.split_terminator_gap_comments(&mut parts, after_start, span_end, false)
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
            self.split_terminator_gap_comments(&mut result_parts, argument_end, span_end, false)
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

    /// Build unconditional paren-wrapped doc for return/throw with own-line comments.
    ///
    /// Matches Prettier's `["(", indent([hardline, argumentDoc]), hardline, ")"]`
    /// (function.js:239). Unlike the binaryish case which uses `ifBreak`, this is
    /// unconditional because the comment placement makes the line break semantically necessary.
    fn build_comment_paren_doc(
        &self,
        keyword: &'static str,
        arg: &Expression<'_>,
        span_end: u32,
        inline_comments: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let raw_expr_doc = self.build_expression_doc(arg);
        // Assignment expressions need inner parens for clarity: return (\n  (a = b)\n);
        let expr_doc = if matches!(arg, Expression::AssignmentExpression(_)) {
            d.parens(raw_expr_doc)
        } else {
            raw_expr_doc
        };
        let mut body = DocBuf::new();
        if let Some(comments_doc) = inline_comments {
            body.push(comments_doc);
        }
        body.push(expr_doc);

        // The grouping `)` — not the statement end — bounds what is *inside* the parens.
        // These parens are not optional: they are what makes the comment-forced break
        // legal, so a comment before the `)` stays inside them, where it was written
        // (`build_yield_doc`'s hanging branch is the same rule for the third restricted
        // production; omitting it here DROPPED the comment outright). A comment *past* the
        // `)` is outside them and follows the ordinary terminator rule, trailing the `;`.
        let argument_end = arg.span().end;
        let (in_paren_end, after_start) = match self.retained_grouping_close(argument_end, span_end)
        {
            Some(close) => (close, close.saturating_add(1).min(span_end)),
            None => (argument_end, argument_end),
        };
        if self.has_comments_to_emit_between(argument_end, in_paren_end) {
            self.append_trailing_paren_comments(&mut body, argument_end, in_paren_end);
        }

        let mut parts: DocBuf = smallvec![self.build_hanging_paren_doc(keyword, d.concat(&body))];
        let after = if self.has_comments_to_emit_between(after_start, span_end) {
            self.split_terminator_gap_comments(&mut parts, after_start, span_end, false)
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
    /// break with none present — so there is no inside to speak of.
    fn retained_grouping_close(&self, argument_end: u32, span_end: u32) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let mut close = None;
        let mut scan = argument_end as usize;
        while let Some(found) = find_char_skipping_comments(bytes, scan, span_end as usize, b')') {
            close = Some(found as u32);
            scan = found + 1;
        }
        close
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
    pub(in crate::printer) fn build_hanging_paren_doc(
        &self,
        keyword: &'static str,
        body: DocId,
    ) -> DocId {
        let d = self.d();
        d.concat(&[
            d.text(keyword),
            d.text(" ("),
            d.indent(d.concat(&[d.hardline(), body])),
            d.hardline(),
            d.text(")"),
        ])
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
        let after_semi =
            self.split_terminator_gap_comments(&mut inline_trailing, expr_end, semicolon_pos, true);
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
