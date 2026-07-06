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
pub(super) use super::{Printer, build_entity_name_doc, should_hug_union_type};

use super::ParenContext;
use super::expressions::literals::format_directive;
use super::needs_parens::leftmost_no_lookahead;
use crate::ast::internal::{self, Expression, LiteralValue, Statement};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    /// Build a Doc for a statement
    pub(super) fn build_statement_doc(&self, statement: &Statement<'_>) -> DocId {
        let d = self.d();
        match statement {
            Statement::ExpressionStatement(stmt) => self.build_expression_statement_doc(stmt),
            Statement::VariableDeclaration(decl) => self.build_variable_declaration_doc(decl, true),
            Statement::TSTypeAliasDeclaration(decl) => self.build_type_alias_declaration_doc(decl),
            Statement::ReturnStatement(ret) => self.build_return_statement_doc(ret),
            Statement::BlockStatement(block) => self.build_block_statement_doc(block),
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
    /// Handles parentheses for object patterns and comments before semicolon.
    /// Preserves source parens around string literals: `('hello');` stays parenthesized.
    fn build_expression_statement_doc(&self, stmt: &internal::ExpressionStatement<'_>) -> DocId {
        let d = self.d();

        let mut parts = DocBuf::new();

        if stmt.is_directive {
            // Directives are exact code-unit sequences; `format_directive` mirrors
            // Prettier's `printDirective` (swap the outer quote to single only when
            // the content has no quote, else verbatim). Never parenthesized.
            let raw = stmt.expression.span().extract(self.source);
            parts.push(d.text_pooled(&format_directive(raw)));
        } else {
            // Parens required for correctness (object expressions, object pattern assignments)
            // OR preserved from source for string literals (matches Prettier behavior)
            let needs_parens = self
                .needs_parens(&stmt.expression, ParenContext::ExpressionStatement)
                || self.has_expression_statement_source_parens(stmt);

            // An own-line comment between a source `(` and the expression
            // (`(// c⏎ expr)` / `(/* c */⏎ expr)` — e.g. a bare parenthesized
            // decorated class expression) is preserved inside the parens, breaking
            // them open; the flat `(`/`)` wrap below would drop it. Own-line = a line
            // comment (never inline) or a block comment with a newline before it; a
            // same-line block comment (`(/* c */ expr)`) is a separate, rarer case
            // (inline) left to the default flow. `stmt.span.start < expr_start` means
            // a real source `(` precedes the expression (see
            // `has_expression_statement_source_parens`). prettier hoists the comment
            // before `(` — a divergence (`decorated_expr_open_paren_comment`).
            // TODO: a same-line block comment after `(` is still dropped here.
            let expr_start = stmt.expression.span().start;
            let paren_open_own_line_comment = needs_parens
                && stmt.span.start < expr_start
                && comments_in_range(self.comments, stmt.span.start + 1, expr_start)
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
                    Expression::ClassExpression(c) if c.decorators.is_some_and(|dec| !dec.is_empty())
                );

            if paren_open_own_line_comment {
                let mut inner: DocBuf = smallvec![d.hardline()];
                for comment in comments_in_range(self.comments, stmt.span.start + 1, expr_start) {
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
        let after = self.split_separator_gap_comments(&mut parts, expr_end, semicolon_pos, true);
        parts.push(d.text(";"));
        parts.extend(after);
        d.concat(&parts)
    }

    /// Check if an expression statement had parentheses in the source that should be preserved.
    ///
    /// Prettier preserves parens around string literal expression statements:
    /// `('hello');` stays as-is, not stripped to `'hello';`.
    /// Detected via span: if ExpressionStatement.span.start < Expression.span.start,
    /// the source had a `(` before the expression.
    fn has_expression_statement_source_parens(
        &self,
        stmt: &internal::ExpressionStatement<'_>,
    ) -> bool {
        if stmt.span.start >= stmt.expression.span().start {
            return false;
        }
        matches!(
            &stmt.expression,
            Expression::Literal(lit) if matches!(lit.value, LiteralValue::String { .. })
        )
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
        let after = self.split_separator_gap_comments(&mut parts, keyword_end, semicolon_pos, true);
        parts.push(d.text(";"));
        parts.extend(after);
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

        // Extract inline comments between keyword and argument
        // Uses line-comment-safe spacing to prevent `return // comment expr`
        let keyword_end = keyword_start + keyword.len() as u32;
        let inline_comments = self.build_rhs_comments_opt(keyword_end, arg.span().start);

        // Trailing comments from stripped grouping parens: `return (x /* c */)` → `return x /* c */;`
        let argument_end = arg.span().end;
        let has_trailing_comments = self.has_comments_between(argument_end, span_end);

        if self.argument_has_own_line_comment(keyword_start, arg) {
            return self.build_comment_paren_doc(keyword, arg, inline_comments);
        }

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
            let after = if self.has_comments_between(after_start, span_end) {
                self.split_terminator_gap_comments(&mut parts, after_start, span_end, false)
            } else {
                DocBuf::new()
            };
            parts.push(d.text(";"));
            parts.extend(after);
            return d.concat(&parts);
        }

        if let Expression::BinaryExpression(binary) = arg {
            return self.build_binary_paren_doc(keyword, binary, inline_comments);
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
    fn argument_has_own_line_comment(&self, keyword_start: u32, arg: &Expression<'_>) -> bool {
        // Check for own-line comments before the argument itself
        // (e.g., `return // comment\n expr`)
        if self.has_leading_own_line_comment_in_range(keyword_start, arg.span().start) {
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
                // Check for leading own-line comments between object and property.
                // Must NOT be on the same line as the object — trailing comments
                // like `foo() // comment` don't trigger paren wrapping.
                let obj_end = member.object.span().end;
                let prop_start = member.property.span().start;
                if self.has_leading_own_line_comment_in_range(obj_end, prop_start) {
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

    /// Check if there are any leading own-line comments in a range.
    ///
    /// "Leading own-line" means the comment is NOT on the same line as `start`
    /// (i.e., it's on its own line, not trailing the previous expression).
    /// This matches Prettier's `hasLeadingOwnLineComment` which checks for
    /// comments with a newline after them that are leading on a node.
    fn has_leading_own_line_comment_in_range(&self, start: u32, end: u32) -> bool {
        comments_in_range(self.comments, start, end)
            .any(|c| !self.is_same_line(start, c.span.start))
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
        let rhs_doc = if let Some(comments_doc) = inline_comments {
            d.concat(&[comments_doc, expr_doc])
        } else {
            expr_doc
        };
        d.concat(&[
            d.text(keyword),
            d.text(" ("),
            d.indent(d.concat(&[d.hardline(), rhs_doc])),
            d.hardline(),
            d.text(");"),
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
        // after it.
        let expr_end = binary.span.end;
        let semicolon_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            expr_end as usize,
            self.source.len(),
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
        let has_operand_line_comment = comments_in_range(self.comments, expr_end, semicolon_pos)
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
        let broken_doc = d.concat(&[
            d.text(" ("),
            d.indent(d.concat(&[d.softline(), d.group(expr_doc), trailing_comments_doc])),
            d.softline(),
            d.text(")"),
        ]);

        let flat_doc = d.concat(&[d.text(" "), expr_doc, trailing_comments_doc]);

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
