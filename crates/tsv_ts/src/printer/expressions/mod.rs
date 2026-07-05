// Expression printing for TypeScript
//
// This module coordinates expression printing and delegates to specialized submodules:
// - literals.rs: Literals, identifiers, regex, spread, normalize helper
// - functions.rs: Arrow functions and function expressions
// - blocks.rs: Block statements (reusable utility)
// - patterns.rs: All destructuring patterns (object, array, assignment, rest)
// - objects.rs: Object expressions and property handling
// - arrays.rs: Array expressions
// - operators.rs: Unary, binary, and update expressions
// - assignment.rs: Assignment layout engine (declarators, properties, returns)
// - conditional.rs: Ternary/conditional expressions
// - template_literal.rs: Template literals (both regular and tagged)
// - ../calls/: Call, new, and member-chain expressions
//
// This module handles:
// - Expression dispatch (print_expression, build_expression_doc)

mod arrays;
pub(in crate::printer) mod assignment;
mod blocks;
mod conditional;
mod functions;
pub(crate) mod literals;
mod objects;
mod operators;
mod patterns;
mod template_literal;

use self::operators::OperatorBuf;
use crate::ast::internal::{BinaryExpression, Expression, TSType};
use crate::printer::comments::{CommentFilter, CommentSpacing};
use crate::printer::{ParenContext, PatternContext, Printer, chain};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Print an expression using doc-based formatting
    pub(crate) fn print_expression(&mut self, expression: &Expression<'_>) {
        let doc = self.build_expression_doc(expression);
        self.write_arena_doc(doc);
    }

    /// Wrap `doc` in parens when `span` is the object/function/class node that
    /// starts the enclosing expression statement (set by `build_expression_statement_doc`
    /// via `leftmost_no_lookahead`). Consumes the target so it fires exactly once:
    /// `(class {}).foo` wraps the class, not the whole member expression.
    fn maybe_wrap_expr_stmt_paren(&self, span: Span, doc: DocId) -> DocId {
        // Matched by span, not consumed: a chain may rebuild its base across
        // conditional-group variants (`({a: 1}).b().c()`), so consuming the target on
        // the first (possibly discarded) build would leave the selected variant
        // unwrapped. The target is cleared once per statement in build_expression_statement.
        if self.expr_stmt_paren_target.get() == Some(span) {
            self.d().parens(doc)
        } else {
            doc
        }
    }

    /// Build a Doc for an expression (for use in object/array contexts and statements)
    pub(crate) fn build_expression_doc(&self, expr: &Expression<'_>) -> DocId {
        let d = self.d();

        // Take and clear is_expression_statement so it doesn't leak to sub-expressions.
        // Only chain formatting needs this flag (for the isShort merge heuristic).
        // Re-set it only for expression types that enter chain formatting:
        // CallExpression, MemberExpression, TSNonNullExpression.
        let was_expr_stmt = self.is_expression_statement.replace(false);

        match expr {
            Expression::Literal(lit) => self.build_literal_doc(lit),
            Expression::Identifier(id) => self.build_identifier_doc(id),
            Expression::PrivateIdentifier(pid) => self.build_private_identifier_doc(pid),
            Expression::ObjectExpression(obj) => {
                // Wrap in parens when this is the leftmost object of an arrow body
                // (`() => ({}) && a`). Matched by span (not consumed): a chain may rebuild
                // its base across conditional-group variants, and a nested call-argument
                // object has a different span so it never matches.
                let needs_arrow_parens =
                    self.arrow_body_object_parens_target.get() == Some(obj.span);
                let doc = self.build_object_doc(obj);
                let doc = if needs_arrow_parens {
                    self.d().parens(doc)
                } else {
                    doc
                };
                self.maybe_wrap_expr_stmt_paren(obj.span, doc)
            }
            Expression::ArrayExpression(arr) => self.build_array_doc(arr),
            Expression::UnaryExpression(unary) => self.build_unary_doc(unary),
            Expression::UpdateExpression(update) => self.build_update_doc(update),
            Expression::BinaryExpression(binary) => self.build_binary_doc(binary),
            Expression::CallExpression(call) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_call_doc(call)
            }
            Expression::NewExpression(new_expr) => self.build_new_doc(new_expr),
            Expression::MemberExpression(member) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_member_doc(member)
            }
            Expression::ConditionalExpression(cond) => {
                self.build_conditional_doc_with_wrapping(cond)
            }
            Expression::ArrowFunctionExpression(arrow) => self.build_arrow_doc(arrow),
            Expression::FunctionExpression(func) => {
                self.maybe_wrap_expr_stmt_paren(func.span, self.build_function_doc(func))
            }
            Expression::ClassExpression(class_expr) => self.maybe_wrap_expr_stmt_paren(
                class_expr.span,
                self.build_class_expression_doc(class_expr),
            ),
            Expression::SpreadElement(spread) => self.build_spread_doc(spread),
            Expression::TemplateLiteral(template) => self.build_template_literal_doc(template),
            Expression::TaggedTemplateExpression(tagged) => self.build_tagged_template_doc(tagged),
            Expression::AwaitExpression(await_expr) => self.build_await_doc(await_expr),
            Expression::YieldExpression(yield_expr) => self.build_yield_doc(yield_expr),
            Expression::SequenceExpression(seq) => self.build_sequence_doc(seq),
            Expression::RegexLiteral(regex) => self.build_regex_doc(regex),
            Expression::ThisExpression(_) => d.text("this"),
            Expression::Super(_) => d.text("super"),
            Expression::AssignmentExpression(assign) => self.build_assignment_doc(assign),
            Expression::ObjectPattern(obj) => self.build_object_pattern_doc(obj),
            Expression::ArrayPattern(arr) => self.build_array_pattern_doc(arr),
            Expression::AssignmentPattern(pattern) => self.build_assignment_pattern_doc(pattern),
            Expression::RestElement(rest) => self.build_rest_element_doc(rest),
            Expression::TSTypeAssertion(type_assert) => {
                self.build_ts_type_assertion_doc(type_assert)
            }
            Expression::TSAsExpression(as_expr) => {
                self.build_binary_cast_doc(as_expr.expression, as_expr.type_annotation, "as")
            }
            Expression::TSSatisfiesExpression(sat_expr) => self.build_binary_cast_doc(
                sat_expr.expression,
                sat_expr.type_annotation,
                "satisfies",
            ),
            Expression::TSInstantiationExpression(inst_expr) => {
                self.build_ts_instantiation_doc(inst_expr)
            }
            Expression::TSNonNullExpression(non_null_expr) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_ts_non_null_doc(non_null_expr)
            }
            Expression::ImportExpression(import_expr) => {
                self.build_import_expression_doc(import_expr)
            }
            Expression::MetaProperty(meta) => self.build_meta_property_doc(meta),
            Expression::TSParameterProperty(param_prop) => {
                self.build_ts_parameter_property_doc(param_prop)
            }
            Expression::JsdocCast(cast) => self.build_jsdoc_cast_doc(cast),
            // Preserved grouping parens are layout-transparent: render the inner,
            // which re-derives whatever parens it needs (matching prettier, which
            // strips redundant parens and re-adds required ones). Only the wire
            // AST keeps the `ParenthesizedExpression`.
            Expression::ParenthesizedExpression(paren) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_expression_doc(paren.expression)
            }
        }
    }

    /// Build a Doc for a JSDoc type cast: `/** @type {T} */ (inner)`.
    ///
    /// The leading `@type`/`@satisfies` comment for this cast level lives in the
    /// gap before the `(` and is emitted by the *caller* (statement / RHS / arg
    /// leading-comment path, keyed on `cast.span.start` = the `(`). This method
    /// emits the parens plus any comments between this `(` and the inner — which
    /// is how a **nested** cast's own `@type` comment lands (`/** @type {A} */
    /// (/** @type {B} */ (expr))`). The inner is built bare: the cast's own
    /// parens provide grouping, so `needs_parens` must not double-wrap it.
    ///
    /// Layout follows prettier-plugin-svelte's `ParenthesizedExpression`: an
    /// object/array inner **hugs** the parens (`({…})` / `([…])`), every other
    /// inner gets a breakable group (`(inner)` flat, `(⏎\tinner⏎)` when wide). A
    /// line comment in the gap forces a hardline layout so it can't swallow the
    /// inner and the closing `)`.
    fn build_jsdoc_cast_doc(&self, cast: &crate::ast::internal::JsdocCast<'_>) -> DocId {
        let d = self.d();
        let open = cast.span.start; // the `(`
        let inner_start = cast.inner.span().start;
        let inner_doc = self.build_expression_doc(cast.inner);

        // A line comment in the gap before the inner must force a hardline layout —
        // otherwise `(// c <inner>)` runs the inner and the `)` into the comment
        // (silent content loss). Mirrors `build_expression_doc_keep_paren_comments`.
        if self.has_line_comments_between(open + 1, inner_start) {
            let mut parts: DocBuf = smallvec![d.hardline()];
            for comment in comments_in_range(self.comments, open + 1, inner_start) {
                parts.push(self.build_comment_doc(comment));
                // A line comment runs to end-of-line, so it must break; a block
                // comment hugs the next token inline (`/** @type {B} */ (x)`).
                parts.push(if comment.is_block {
                    d.text(" ")
                } else {
                    d.hardline()
                });
            }
            parts.push(inner_doc);
            return d.concat(&[
                d.text("("),
                d.indent(d.concat(&parts)),
                d.hardline(),
                d.text(")"),
            ]);
        }

        // Comments between this cast's `(` and the inner expression (a nested
        // cast's own `@type` comment, when `inner` is itself a JsdocCast) — all
        // block comments here, so they hug inline.
        let interior = self.build_comments_between(open + 1, inner_start, CommentSpacing::Trailing);
        let body = d.concat(&[interior, inner_doc]);

        // Object/array literals hug the parens; the inner's own group breaks it.
        if matches!(
            cast.inner,
            Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
        ) {
            d.concat(&[d.text("("), body, d.text(")")])
        } else {
            d.group(d.concat(&[
                d.text("("),
                d.indent(d.concat(&[d.softline(), body])),
                d.softline(),
                d.text(")"),
            ]))
        }
    }

    /// Build doc for function parameter expression, using FunctionParameter context for patterns
    pub(super) fn build_function_parameter_doc(&self, expr: &Expression<'_>) -> DocId {
        match expr {
            Expression::ObjectPattern(obj) => {
                self.build_object_pattern_doc_with_context(obj, PatternContext::FunctionParameter)
            }
            // For other expressions, use normal doc building
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build doc for expression in call argument or array element context
    ///
    /// Binary/logical expressions get continuation indent when they break:
    /// ```text
    /// fn(
    ///     aaa &&
    ///         bbb,  // extra indent on continuation
    /// )
    /// ```
    ///
    /// Assignment expressions are wrapped in parens for clarity:
    /// `fn((a = b))` not `fn(a = b)`
    pub(super) fn build_arg_expression_doc(&self, expr: &Expression<'_>) -> DocId {
        // Member-chain arg-doc sharing: a chain builds the same group flat and expanded
        // across `conditional_group` candidates; reuse the one build instead of
        // re-recursing (kills the O(4^depth) rebuild — see the `chain_arg_share` field
        // doc). Eligibility guarantees a hit is byte-identical to a rebuild.
        if self.chain_arg_share_eligible() {
            let key = std::ptr::from_ref(expr) as usize;
            if let Some(&doc) = self.chain_arg_share.borrow().get(&key) {
                return doc;
            }
            let doc = self.build_arg_expression_doc_uncached(expr);
            self.chain_arg_share.borrow_mut().insert(key, doc);
            return doc;
        }
        self.build_arg_expression_doc_uncached(expr)
    }

    fn build_arg_expression_doc_uncached(&self, expr: &Expression<'_>) -> DocId {
        let d = self.d();
        // Assignment expressions need parens in argument context for clarity
        if self.needs_parens(expr, ParenContext::Argument) {
            return d.parens(self.build_expression_doc(expr));
        }

        match expr {
            Expression::BinaryExpression(binary) => {
                // Use indented binary chain - continuation lines get extra indent
                self.build_binary_chain_doc_indented(binary)
            }
            Expression::ConditionalExpression(cond) => {
                // Ternary in call/new args: binary expressions in branches use
                // continuation indent. Matches Prettier's shouldNotIndent = false
                // when grandparent is CallExpression/NewExpression (binaryish.js:112).
                self.build_conditional_doc_with_binary_test_indent(cond)
            }
            // For other expressions, use normal doc building
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build a Doc for an expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the object/array to expand
    /// internally with hardlines so fits() can correctly measure the first line.
    /// For example, `.fn({prop})` should become `.fn({\n  prop,\n})` when expanded.
    pub(super) fn build_arg_expression_doc_expanded(&self, expr: &Expression<'_>) -> DocId {
        match expr {
            Expression::ObjectExpression(obj) => self.build_object_doc_expanded(obj),
            Expression::ArrayExpression(arr) => self.build_array_doc_expanded(arr),
            // For other expressions, use normal doc building
            _ => self.build_arg_expression_doc(expr),
        }
    }

    //
    // TypeScript Type Assertions
    //

    /// Build a Doc for a TypeScript angle-bracket type assertion: `<Type>expr`
    fn build_ts_type_assertion_doc(
        &self,
        type_assert: &crate::ast::internal::TSTypeAssertion<'_>,
    ) -> DocId {
        let d = self.d();
        let expr_needs_parens =
            self.needs_parens(type_assert.expression, ParenContext::AngleBracketAssertion);
        // Cast boundary positions: `<` … type … `>` … expression. The `>` is found
        // past any comment that itself contains a `>` (`<T /* > */>`).
        let open_pos = type_assert.span.start; // the `<`
        let angle_end = open_pos + 1; // after `<`
        let type_start = type_assert.type_annotation.span().start;
        let type_end = type_assert.type_annotation.span().end;
        let expr_start = type_assert.expression.span().start;
        let close_angle = self.find_assertion_close_angle(type_end, expr_start);
        let type_doc = self.build_type_doc_with_wrapping_type_args(type_assert.type_annotation);

        // Comments in the cast stay where the author wrote them. Block comments hug
        // inline (`</* c */ T>`, `<T /* c */>`, `<T>/* c */ expr`); a `//` runs to
        // end-of-line, so it forces the cast to break — and where prettier relocates
        // it across the `<`/`>` boundary, tsv preserves position. See
        // conformance_prettier.md §Comment relocation (Angle-bracket type assertion)
        // and the `type_assertion_line_comment` /
        // `type_assertion_close_own_line_comment` divergence fixtures.
        let cast_doc = if self.has_line_comments_between(angle_end, type_start)
            || self.has_line_comments_between(type_end, close_angle)
        {
            self.build_assertion_broken_cast(open_pos, type_start, type_end, close_angle, type_doc)
        } else {
            // Mirror Prettier's `printTypeAssertion`: the cast `<Type>` is its own
            // group, breaking after `<` with the type on an indented line and `>`
            // back at the outer indent. Crucially, a union cast type prints *flat* on
            // that line — Prettier's `shouldIndentUnionType` returns false for
            // `TSTypeAssertion`, so it never gets the leading-`|` hanging indent that
            // `as`/`satisfies` casts use (see `build_union_hanging_indent_doc`).
            let comments_doc =
                self.build_comments_between(angle_end, type_start, CommentSpacing::Trailing);
            let before_close_doc = self.build_comments_between_filtered(
                type_end,
                close_angle,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            );
            d.group(d.concat(&[
                d.text("<"),
                d.indent(d.concat(&[d.softline(), comments_doc, type_doc, before_close_doc])),
                d.softline(),
                d.text(">"),
            ]))
        };

        let inner_expr = self.build_expression_doc(type_assert.expression);
        let expr_doc = if expr_needs_parens {
            d.parens(inner_expr)
        } else {
            inner_expr
        };

        // A line comment after `>` drops the expression to a continuation line one
        // indent in (prettier instead relocates it into the cast — a divergence).
        // Each comment holds its position: a same-line `<T> // c` trails the `>`,
        // an own-line comment keeps its own line leading the expression
        // (`build_trailing_comments_multiline`). A block comment in the gap stays
        // inline ahead of the expression.
        //
        // TODO: a binary-expression operand that *breaks across lines* misaligns — its
        // first operand sits at this `indent` level but the chain's continuation `line`s
        // snap back to the enclosing assignment-level indent (binary chains take their
        // continuation indent from the parent context, not a nested `indent`), so
        // `aaaa ||` lands one level deeper than `bbbb ||`. Idempotent and lossless, and
        // only reachable via cast + after-`>` line comment + a wrapping binary operand
        // (absent from any real corpus). A real fix means threading parent-indent context
        // into the chain printer — out of scope here. No fixture guards it on purpose:
        // `input.svelte` must be idempotent, so it could only bake the misaligned output
        // in as canonical, sanctioning a known imperfection as a deliberate divergence —
        // which the fixture rules forbid. The fix is to align it, after which an ordinary
        // fixture follows.
        if self.has_line_comments_between(close_angle + 1, expr_start) {
            let trailing = self.build_trailing_comments_multiline(close_angle + 1, expr_start);
            return d.concat(&[
                cast_doc,
                d.indent(d.concat(&[d.concat(&trailing), d.hardline(), expr_doc])),
            ]);
        }

        // A block comment after `>` leads the expression in every layout branch
        // below, so fold it onto the cast once rather than into each branch.
        let after_close_doc = self.build_comments_between_filtered(
            close_angle + 1,
            expr_start,
            CommentSpacing::Trailing,
            CommentFilter::BlockOnly,
        );
        let cast_group = d.concat(&[cast_doc, after_close_doc]);

        // `shouldBreakAfterCast`: object/array-literal expressions hug the cast
        // (they expand themselves), everything else may break the expression into
        // its own parenthesized block before the cast group itself breaks.
        let should_break_after_cast = !matches!(
            type_assert.expression,
            Expression::ArrayExpression(_) | Expression::ObjectExpression(_)
        );

        if should_break_after_cast {
            let expr_contents = d.group_break(d.concat(&[
                d.if_break(d.text("("), d.empty()),
                d.indent(d.concat(&[d.softline(), expr_doc])),
                d.softline(),
                d.if_break(d.text(")"), d.empty()),
            ]));
            d.conditional_group(&[
                d.concat(&[cast_group, expr_doc]),
                d.concat(&[cast_group, expr_contents]),
                d.concat(&[cast_group, expr_doc]),
            ])
        } else {
            d.group(d.concat(&[cast_group, expr_doc]))
        }
    }

    /// Build a forced-break cast `<Type>` that preserves line comments in place.
    ///
    /// Used when a `//` sits between `<` and the type, or between the type and `>`
    /// — it runs to end-of-line, so the cast can't stay inline. Each comment holds
    /// the position the author gave it: a same-line `< // c` is pulled onto the `<`
    /// line (`delimiter_line_comment_prefix`, the open-delimiter family — prettier
    /// relocates it to its own line); own-line comments after `<` sit on their own
    /// lines; a trailing-type `T // c` stays on the type line and an own-line
    /// comment before `>` keeps its own line (`build_trailing_comments_multiline`).
    /// See conformance_prettier.md §Comment relocation (Angle-bracket type assertion).
    ///
    /// Positions are the caller's already-computed cast boundaries, in source order:
    /// `open_pos` is the `<`, `type_start`/`type_end` bound the type, `close_angle`
    /// is the `>`. Taking them explicitly keeps this a pure doc assembler with no
    /// `TSTypeAssertion` dependency or re-derivation.
    fn build_assertion_broken_cast(
        &self,
        open_pos: u32,
        type_start: u32,
        type_end: u32,
        close_angle: u32,
        type_doc: DocId,
    ) -> DocId {
        let d = self.d();
        let angle_end = open_pos + 1; // after `<`
        let (angle_prefix, angle_pull_pos) =
            self.delimiter_line_comment_prefix(open_pos, type_start);
        let leading =
            self.build_leading_comments_multiline_opt(angle_end, type_start, angle_pull_pos);
        let trailing = self.build_trailing_comments_multiline(type_end, close_angle);
        d.concat(&[
            d.text("<"),
            d.concat(&angle_prefix),
            d.indent(d.concat(&[
                d.hardline(),
                d.concat(&leading),
                type_doc,
                d.concat(&trailing),
            ])),
            d.hardline(),
            d.text(">"),
        ])
    }

    /// Build a Doc for a TypeScript binary cast expression — `expr as Type` or
    /// `expr satisfies Type`. Mirrors Prettier's `printBinaryCastExpression`,
    /// which prints both with one function (`isSatisfiesExpression ? "satisfies" : "as"`).
    ///
    /// `keyword` is the bare keyword (`"as"` / `"satisfies"`).
    ///
    /// Comments are preserved where the author wrote them — between the
    /// expression and the keyword, and between the keyword and the type. `as
    /// const` is no exception: its `const` is treated like any other cast type.
    /// (Prettier relocates an `as const` inner comment before the whole
    /// expression; tsv keeps it in place — a documented divergence.)
    fn build_binary_cast_doc(
        &self,
        expression: &Expression<'_>,
        type_annotation: &TSType<'_>,
        keyword: &'static str,
    ) -> DocId {
        let d = self.d();
        let needs_parens = self.needs_parens(expression, ParenContext::TypeAssertion);
        let mut parts = d.pooled_docbuf();
        if needs_parens {
            parts.push(d.text("("));
        }
        parts.push(self.build_expression_doc(expression));
        if needs_parens {
            parts.push(d.text(")"));
        }

        // Find the keyword position
        let expr_end = expression.span().end;
        let type_start = type_annotation.span().start;
        let keyword_pos = self.find_keyword_in_range(expr_end, type_start, keyword);

        // Comments between expression and keyword → place before the keyword
        if let Some(kw_pos) = keyword_pos {
            parts.push(self.build_inline_comments_between_doc(expr_end, kw_pos));
        }

        // A comment between the keyword and the type that can't be inlined forces the
        // type onto the next line, keeping the comment with the cast: a line comment
        // (inlining would let `//` swallow the type — `x as // c A`), or a multiline
        // block comment. A single-line block comment (own-line, trailing, or glued)
        // collapses inline (`x as /* c */ A`). Applies uniformly, including `as const`.
        // See as_satisfies_value_line_comment / as_satisfies_value_own_line_block_comment.
        if let Some(kw_pos) = keyword_pos {
            let kw_end = kw_pos + keyword.len() as u32;
            // A line comment or multiline block hangs the type on its own line; a
            // single-line block comment collapses inline (the fall-through below).
            // Prettier relocates the collapsed comment before the keyword instead.
            if self.comments_force_own_line_between(kw_end, type_start) {
                parts.push(d.text(" "));
                parts.push(d.text(keyword));
                let type_doc = self.build_type_doc_with_wrapping_type_args(type_annotation);
                self.append_keyword_value_line_comments(&mut parts, kw_end, type_start, type_doc);
                return d.concat(&parts);
            }
        }

        // Strip redundant comment-free parens so `(A | B)` / `(A & B)` cast types
        // get the same hanging layout as the bare form (prettier strips them too).
        let value_type = self.unwrap_redundant_parens(type_annotation);

        // Union cast types break after the keyword with a hanging indent.
        if let Some(tail) =
            self.cast_union_hanging_tail(keyword, keyword_pos, value_type, type_start)
        {
            parts.push(tail);
            return d.concat(&parts);
        }

        // Intersection cast types: the first member hugs the keyword, continuations
        // wrap with a hanging indent (mirrors the type-alias / annotation layout).
        if let Some(tail) =
            self.cast_intersection_hanging_tail(keyword, keyword_pos, value_type, type_start)
        {
            parts.push(tail);
            return d.concat(&parts);
        }

        parts.push(d.text(" "));
        parts.push(d.text(keyword));
        parts.push(d.text(" "));

        // Comments between keyword and type → kept in place, trailing the keyword
        // (uniform for every cast type, including `as const`).
        if let Some(kw_pos) = keyword_pos {
            let kw_end = kw_pos + keyword.len() as u32;
            parts.push(self.build_comments_between(kw_end, type_start, CommentSpacing::Trailing));
            parts.push(self.build_type_doc_with_wrapping_type_args(type_annotation));
        } else {
            parts.push(self.build_type_doc_with_wrapping_type_args(type_annotation));
        }

        d.concat(&parts)
    }

    /// The keyword-plus-type tail for an `as`/`satisfies` cast when the cast type
    /// is a non-hugging union: it breaks after the keyword with a hanging indent
    /// (Prettier's `shouldIndentUnionType`). `keyword` is the bare keyword
    /// (`"as"` / `"satisfies"`).
    ///
    /// Returns `None` to fall through to the caller's inline layout — for
    /// non-union or hugging types, or when a comment sits between the keyword and
    /// the type.
    ///
    /// TODO: a comment before a *breaking* union (`x as /* c */ A | B` past print
    /// width) still misses the hanging indent. Prettier is non-idempotent here (it
    /// relocates the comment across the keyword), so the target is a
    /// comment-position-philosophy case, not a clean match — deferred.
    fn cast_union_hanging_tail(
        &self,
        keyword: &'static str,
        keyword_pos: Option<u32>,
        type_annotation: &TSType<'_>,
        type_start: u32,
    ) -> Option<DocId> {
        let keyword_len = keyword.len() as u32;
        if keyword_pos.is_some_and(|pos| self.has_comments_between(pos + keyword_len, type_start)) {
            return None;
        }
        let hanging = self.build_union_hanging_indent_doc(type_annotation)?;
        let d = self.d();
        Some(d.concat(&[d.text(" "), d.text(keyword), hanging]))
    }

    /// The keyword-plus-type tail for an `as`/`satisfies` cast when the cast type
    /// is an intersection: the first member hugs the keyword, continuation members
    /// wrap with a hanging indent (via the shared `intersection_hanging_with_indent`,
    /// the same layout the type-alias RHS arm uses).
    ///
    /// Returns `None` to fall through to the caller's inline layout — for
    /// non-intersection types, or when a comment sits between the keyword and the
    /// type.
    fn cast_intersection_hanging_tail(
        &self,
        keyword: &'static str,
        keyword_pos: Option<u32>,
        type_annotation: &TSType<'_>,
        type_start: u32,
    ) -> Option<DocId> {
        let keyword_len = keyword.len() as u32;
        if keyword_pos.is_some_and(|pos| self.has_comments_between(pos + keyword_len, type_start)) {
            return None;
        }
        let TSType::Intersection(i) = type_annotation else {
            return None;
        };
        let d = self.d();
        let body = self.intersection_hanging_with_indent(i);
        Some(d.concat(&[d.text(" "), d.text(keyword), d.text(" "), body]))
    }

    /// Build a Doc for a TypeScript instantiation expression
    fn build_ts_instantiation_doc(
        &self,
        inst_expr: &crate::ast::internal::TSInstantiationExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        let needs_parens =
            self.needs_parens(inst_expr.expression, ParenContext::InstantiationExpression);
        if needs_parens {
            parts.push(d.text("("));
        }
        parts.push(self.build_expression_doc(inst_expr.expression));
        if needs_parens {
            parts.push(d.text(")"));
        }
        // Preserve comments between expression and type args: `fn/* c */ <string>`
        let expr_end = inst_expr.expression.span().end;
        let ta_start = inst_expr.type_arguments.span.start;
        if let Some(doc) = self.build_name_to_type_params_comments_opt(
            expr_end,
            ta_start,
            CommentSpacing::Trailing,
        ) {
            parts.push(doc);
        }
        parts.push(self.build_type_parameter_instantiation_doc(&inst_expr.type_arguments));
        d.concat(&parts)
    }

    /// Build a Doc for a TypeScript non-null assertion expression
    ///
    /// When wrapping certain expressions in parens (binary, ternary, etc.),
    /// prettier indents continuations when the expression breaks:
    /// ```text
    /// (veryLongExpr ||
    ///     continuation)!
    /// ```
    fn build_ts_non_null_doc(
        &self,
        non_null_expr: &crate::ast::internal::TSNonNullExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let needs_parens = self.needs_parens(non_null_expr.expression, ParenContext::NonNull);

        // A leading comment from the stripped grouping parens, before the operand
        // (`(/* b */ x + y)!`), is emitted before the operand/`(`, matching prettier
        // (`/* b */ (x + y)!`) — tsv previously dropped it. None of the branches below
        // emit it, so it is prepended once here.
        let argument_start = non_null_expr.expression.span().start;
        let leading = self.build_rhs_comments_opt(non_null_expr.span.start, argument_start);

        let core = if needs_parens {
            // For expressions that need parens, use a special doc structure
            // that indents continuations when breaking
            let inner_doc =
                self.build_expression_doc_with_indent_on_break(non_null_expr.expression);
            let argument_end = non_null_expr.expression.span().end;
            // Keep comments from the stripped grouping parens INSIDE them, where the
            // author wrote them — leading before the operand (`(/* b */ x + y)!`),
            // trailing before the `)` (`(x + y /* c */)!`). Prettier relocates them
            // outside (before `(` / between `)` and `!`); tsv preserves the position.
            let mut parts: DocBuf = smallvec![d.text("(")];
            if let Some(lead) = leading {
                parts.push(lead);
            }
            parts.push(inner_doc);
            if self.has_comments_between(argument_end, non_null_expr.span.end) {
                self.append_trailing_paren_comments(
                    &mut parts,
                    argument_end,
                    non_null_expr.span.end,
                );
            }
            parts.push(d.text(")!"));
            d.concat(&parts)
        } else if self
            .has_comments_between(non_null_expr.expression.span().end, non_null_expr.span.end)
        {
            // A comment between the operand and `!` (`p?.q /* c */!`, or from stripped
            // grouping parens `(x /* c */)!`) trails the operand — preserve it rather
            // than dropping it. The redundant grouping parens are stripped per tsv's
            // non-null seal canonicalization (`(p?.q)!` → `p?.q!`); prettier keeps them
            // when the source had them. Comments can't be threaded through the
            // linearized chain, so this path renders the operand directly for chain and
            // non-chain operands alike.
            let argument_end = non_null_expr.expression.span().end;
            let inner_doc = self.build_expression_doc(non_null_expr.expression);
            let mut parts: DocBuf = smallvec![inner_doc];
            self.append_trailing_paren_comments(&mut parts, argument_end, non_null_expr.span.end);
            parts.push(d.text("!"));
            d.concat(&parts)
        } else if Self::is_chain_expression(non_null_expr.expression) {
            // When inner expression is a chain (member or call), use chain architecture
            // to properly handle breaking. This ensures the outer `!` is included
            // in the linearized chain for proper segment grouping.
            let nodes = chain::linearize_chain_from_non_null(non_null_expr);
            let groups = chain::group_chain_nodes(&nodes);
            chain::build_chain_doc(&groups, self)
        } else {
            let inner_doc = self.build_expression_doc(non_null_expr.expression);
            d.concat(&[inner_doc, d.text("!")])
        };

        // For paren-stripped branches the leading comment goes before the operand
        // (parens are gone, matching prettier); the needs_parens branch above already
        // placed it inside the kept parens.
        match leading {
            Some(lead) if !needs_parens => d.concat(&[lead, core]),
            _ => core,
        }
    }

    /// When `expr` is a non-null assertion sealing a parenthesized optional chain
    /// (`(a?.b)!` / `(a?.())!` — the `!` outside the source parens, detected via the
    /// span gap), render it as `(chain)!` with the parens kept. Returns `None` for
    /// any other expression.
    ///
    /// Used in always-required-parens positions (`new` callee, tagged-template tag)
    /// where an optional chain may not appear unsealed (`` a?.b!`x` `` /
    /// `new a?.b!()` are syntax errors). The standalone non-null path strips the
    /// now-redundant parens (`(a?.b)!` → `a?.b!`), so they are restored per-context
    /// here. Normalizes to the `!`-outside form, matching the Sprint-2 sealed-base
    /// rendering (`push_sealed_chain_base` / the chain linearizer's non-null arm).
    pub(crate) fn build_sealed_non_null_paren_doc(&self, expr: &Expression<'_>) -> Option<DocId> {
        let Expression::TSNonNullExpression(non_null) = expr else {
            return None;
        };
        if non_null.seals_optional_chain() {
            let d = self.d();
            let inner_doc = self.build_expression_doc_with_indent_on_break(non_null.expression);
            Some(d.concat(&[d.text("("), inner_doc, d.text(")!")]))
        } else {
            None
        }
    }

    /// Check if an expression is part of a chain (member, call, or non-null)
    fn is_chain_expression(expr: &Expression<'_>) -> bool {
        matches!(
            expr,
            Expression::MemberExpression(_)
                | Expression::CallExpression(_)
                | Expression::TSNonNullExpression(_)
        )
    }

    /// Build expression doc with indentation added to line breaks
    /// Used when expression is inside inline parens like `(expr)!`
    pub(crate) fn build_expression_doc_with_indent_on_break(&self, expr: &Expression<'_>) -> DocId {
        match expr {
            Expression::BinaryExpression(binary) => {
                // Build binary chain with indented continuations
                self.build_binary_chain_doc_indented(binary)
            }
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build binary chain doc with indented continuations
    /// Used when the binary expression is inside inline parens
    fn build_binary_chain_doc_indented(&self, binary: &BinaryExpression<'_>) -> DocId {
        let d = self.d();
        d.group(self.build_binary_chain_parts_indented(binary))
    }

    /// Build binary chain parts with indented continuations (no group wrapper)
    ///
    /// Returns the concat without a group wrapper, for cases where the caller
    /// wants to control the grouping (e.g., chain printing).
    pub(crate) fn build_binary_chain_parts_indented(&self, binary: &BinaryExpression<'_>) -> DocId {
        let d = self.d();
        // If there are comments within the binary expression, use the comment-aware
        // implementation from operators.rs which preserves comments and their line breaks.
        // This handles cases like: fn(a && // comment\n    b)
        if self.has_comments_between(binary.span.start, binary.span.end) {
            // Use the parts version (no group wrapper) since our caller controls grouping
            return self.build_binary_chain_parts_with_continuation_indent(binary);
        }

        // Collect all operands and operators in the chain
        let mut operands = DocBuf::new();
        let mut operators = OperatorBuf::new();
        self.collect_binary_operands_for_indent(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Fallback to regular expression doc
            return self.build_binary_doc(binary);
        }

        // Build with indented continuations for chains:
        // "first +
        //     second -
        //     third"
        //
        // When shouldGroup is true (operand types differ from current node type,
        // e.g., `(LogicalExpr) + (ConditionalExpr)`), wrap each continuation in
        // its own sub-group so it can independently evaluate whether it fits on
        // the current line when the outer group breaks. This matches Prettier's
        // binaryish.js where shouldGroup controls whether `right` gets a group.
        //
        // When shouldGroup is false (operands are same AST type category, e.g.,
        // `(BinaryExpr) * 100`), all continuations break together with the outer
        // group. This matches Prettier's behavior for same-type chains.
        let should_group = Self::should_group_binary_continuation(binary);
        // shouldInlineLogicalExpression: when the outermost logical has a non-empty
        // object/array on the right, keep operator and RHS on the same line.
        // Prettier ref: binaryish.js:275, 361
        let should_inline_last = assignment::should_inline_logical_expression(binary);
        let mut parts = d.pooled_docbuf();

        for (i, operand) in operands.iter().enumerate() {
            let is_last = i == operands.len() - 1;
            if i == 0 {
                parts.push(*operand);
            } else if is_last && should_inline_last {
                // shouldInlineLogicalExpression: keep operator and object/array on same line
                // Use indent with space (no line break) instead of indent_line.
                // For 2-operand chains: prettier returns group(parts) with no indent
                //   (shouldInline && !samePrecedence → flat). We skip indent.
                // For 3+ operand chains: prettier uses indent(rest) which applies to all
                //   continuation operands. We need indent to match the level.
                // Prettier ref: binaryish.js:275-280, 131, 169-178
                let is_chained = operands.len() > 2;
                let op_and_operand = if is_chained {
                    // In a chain, use indent (matches other continuations' indent level)
                    // but space instead of line (keeps operator and object on same line)
                    d.concat(&[
                        d.text(" "),
                        d.text(operators[i - 1].as_str()),
                        d.indent(d.concat(&[d.text(" "), *operand])),
                    ])
                } else {
                    // 2-operand: flat, no indent (prettier returns group(parts) directly)
                    d.concat(&[
                        d.text(" "),
                        d.text(operators[i - 1].as_str()),
                        d.text(" "),
                        *operand,
                    ])
                };
                if should_group {
                    parts.push(d.group(op_and_operand));
                } else {
                    parts.push(op_and_operand);
                }
            } else if should_group {
                // Sub-group for independent fitting
                parts.push(d.group(d.concat(&[
                    d.text(" "),
                    d.text(operators[i - 1].as_str()),
                    d.indent_line(*operand),
                ])));
            } else {
                parts.push(d.text(" "));
                parts.push(d.text(operators[i - 1].as_str()));
                parts.push(d.indent_line(*operand));
            }
        }

        d.concat(&parts)
    }

    /// Collect operands and operators from a binary chain (helper for indented version)
    ///
    /// Uses `can_flatten_with()` to determine which operators can be chained together.
    /// Flattens both left and right sides when operators are compatible.
    fn collect_binary_operands_for_indent(
        &self,
        expr: &BinaryExpression<'_>,
        operands: &mut DocBuf,
        operators: &mut OperatorBuf,
    ) {
        // Recursively flatten left side if it can be chained with current operator
        if let Expression::BinaryExpression(left_binary) = expr.left {
            if expr.operator.can_flatten_with(left_binary.operator) {
                self.collect_binary_operands_for_indent(left_binary, operands, operators);
            } else {
                // Can't flatten - build operand with parens if needed
                operands.push(self.build_binary_operand_doc(expr.left, expr.operator, false));
            }
        } else {
            operands.push(self.build_binary_operand_doc(expr.left, expr.operator, false));
        }

        // Add current operator
        operators.push(expr.operator);

        // Also flatten right side for truly associative operators (removes redundant parens)
        // Only logical operators are truly associative; arithmetic preserves right-side parens
        if let Expression::BinaryExpression(right_binary) = expr.right
            && expr.operator.can_flatten_with(right_binary.operator)
            && expr.operator.is_logical()
            && right_binary.operator.is_logical()
        {
            self.collect_binary_operands_for_indent(right_binary, operands, operators);
            return;
        }

        // Right operand can't be flattened - add as-is
        operands.push(self.build_binary_operand_doc(expr.right, expr.operator, true));
    }

    /// Build binary chain specifically for parenthesized context in chain printing
    ///
    /// Structure: operand1 " /", line, operand2 " /", line, operand3
    /// In flat: `a / b / c`
    /// In break: `a /\nb /\nc` (with outer indent providing indentation)
    pub(crate) fn build_binary_chain_for_parens(&self, binary: &BinaryExpression<'_>) -> DocId {
        let d = self.d();
        // Collect all operands and operators in the chain
        let mut operands = DocBuf::new();
        let mut operators = OperatorBuf::new();
        self.collect_binary_operands_for_indent(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Fallback to regular expression doc
            return self.build_binary_doc(binary);
        }

        // For 2-operand non-logical chains, wrap in a group with line() so the
        // binary can independently decide whether to break at the operator.
        // The group stays flat when the operands fit; when they don't, line()
        // fires and breaks at the operator (e.g., `left +\nright`), preventing
        // the operands' internal break points (like member chain dots) from
        // firing instead.
        if operands.len() == 2 && !operators[0].is_logical() {
            return d.group(d.concat(&[
                operands[0],
                d.text(" "),
                d.text(operators[0].as_str()),
                d.line(),
                operands[1],
            ]));
        }

        // For 3+ operand chains, use line breaks between operands:
        // operand1 " /", line, operand2 " /", line, operand3
        let mut parts: DocBuf = DocBuf::new();

        for (i, operand) in operands.iter().enumerate() {
            if i == 0 {
                // First operand
                parts.push(*operand);
            } else {
                // Subsequent operands: line break then operand
                parts.push(d.line()); // space in flat, newline in break
                parts.push(*operand);
            }

            // Add operator after operand (except for last)
            if i < operators.len() {
                parts.push(d.text(" "));
                parts.push(d.text(operators[i].as_str()));
            }
        }

        d.concat(&parts)
    }

    /// Build a Doc for a TypeScript parameter property
    fn build_ts_parameter_property_doc(
        &self,
        param_prop: &crate::ast::internal::TSParameterProperty<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();

        // Print modifiers in canonical TS order: accessibility, override, readonly
        if let Some(acc) = &param_prop.accessibility {
            parts.push(d.text(acc.as_str()));
            parts.push(d.text(" "));
        }

        if param_prop.r#override {
            parts.push(d.text("override "));
        }

        if param_prop.readonly {
            parts.push(d.text("readonly "));
        }

        // Print the parameter (identifier or assignment pattern)
        parts.push(self.build_expression_doc(param_prop.parameter));

        d.concat(&parts)
    }
}
