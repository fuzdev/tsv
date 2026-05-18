// New expression printing for TypeScript
//
// Handles: new Foo(), new Foo(arg1, arg2), new Foo<T>()

use super::calls::{
    PartitionedComments, arrow_has_type_reference_return, build_args_joined_with_comments,
    build_args_split_last, build_arrow_call_body_states, build_arrow_sig_doc,
    build_break_body_state, build_expand_all_args, build_inline_args, build_inline_or_expand_all,
    could_expand_arrow_chain, has_blank_line_between_args, has_inter_argument_comments_slice,
    has_trailing_comments_slice, has_trailing_line_comments_slice, last_two_args_same_type,
    prepend_arrow_body_comments, wrap_call_with_hard_breaks, wrap_call_with_soft_breaks,
    wrap_call_with_will_break_guard,
};
use super::comments::{CommentFilter, CommentSpacing};
use super::utils::{
    arrow_has_trailing_param_comments, is_array_or_object_unwrapped, is_block_function,
    is_concise_numeric_array, is_function_composition_args, is_short_second_arg_for_expand_first,
    is_ternary_arrow_body, preceding_args_allow_expand_last,
};
use super::{
    ParenContext, Printer, has_multiline_content, has_newline_before_position,
    is_multiline_template_expression, needs_parens,
};
use crate::ast::internal;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a new expression with argument wrapping
    pub(super) fn build_new_doc_with_wrapping(&self, new_expr: &internal::NewExpression) -> DocId {
        let d = self.d();
        // Wrap callee in parens if needed (e.g., `new (a || b)()`, `new (a ? b : c)()`)
        let callee = if needs_parens(&new_expr.callee, ParenContext::NewCallee) {
            // For binary expressions (including logical), use a group with softlines
            // so the parens can break independently when the content is too long:
            // new (
            //     a || b || c
            // )()
            //
            // Use ungrouped binary doc so the inner expression doesn't have its own
            // group - the outer group controls whether to break after `(`.
            if let internal::Expression::BinaryExpression(binary) = &*new_expr.callee {
                let inner_doc = self.build_binary_chain_doc_ungrouped(binary);
                d.group(d.concat(&[
                    d.text("("),
                    d.indent_softline(inner_doc),
                    d.softline(),
                    d.text(")"),
                ]))
            } else {
                let callee_doc = self.build_expression_doc(&new_expr.callee);
                d.parens(callee_doc)
            }
        } else {
            self.build_expression_doc(&new_expr.callee)
        };

        // Check for comments between removed parentheses and callee
        // e.g., new (/* comment */ Foo)() has comments in the gap between 'new ' and 'Foo'
        let callee = self.prepend_removed_paren_comments(
            new_expr.span.start,
            new_expr.callee.span().start,
            callee,
        );

        // Build type arguments: `<K, V>`
        let type_args_doc = new_expr
            .type_arguments
            .as_ref()
            .map(|ta| self.build_type_parameter_instantiation_doc(ta));

        // Combine callee with type arguments, preserving comments in the gap
        // e.g., `new Foo/* c */ <string>()` — comment between callee and `<`
        // Uses build_name_to_type_params_comments for safe line comment handling
        let callee_with_types_base = match (&type_args_doc, &new_expr.type_arguments) {
            (Some(ta_doc), Some(ta)) => {
                match self.build_name_to_type_params_comments_opt(
                    new_expr.callee.span().end,
                    ta.span.start,
                    CommentSpacing::Trailing,
                ) {
                    Some(comments_doc) => d.concat(&[callee, comments_doc, *ta_doc]),
                    None => d.concat(&[callee, *ta_doc]),
                }
            }
            (Some(ta_doc), _) => d.concat(&[callee, *ta_doc]),
            _ => callee,
        };

        // Empty args: just `new Foo()` or `new Foo<K, V>()`, preserving dangling comments
        if new_expr.arguments.is_empty() {
            let after_type_args = new_expr
                .type_arguments
                .as_ref()
                .map_or_else(|| new_expr.callee.span().end, |ta| ta.span.end);
            let paren_close = new_expr.span.end;
            // Find the actual `(` to separate pre-paren comments from inside-paren comments
            let actual_paren = self.find_char_outside_comments(after_type_args, paren_close, b'(');
            let mut parts = vec![d.text("new "), callee_with_types_base];
            if let Some(paren_pos) = actual_paren {
                let pre_paren_comments = self.build_comments_between_filtered_opt(
                    after_type_args,
                    paren_pos,
                    CommentSpacing::Leading,
                    CommentFilter::All,
                );
                let inside_paren_comments = self
                    .build_inline_comments_between_doc_no_leading_space_opt(
                        paren_pos + 1,
                        paren_close,
                    );
                if let Some(pre) = pre_paren_comments {
                    parts.push(pre);
                }
                match inside_paren_comments {
                    Some(inner) => {
                        parts.push(d.text("("));
                        parts.push(inner);
                        parts.push(d.text(")"));
                    }
                    None => parts.push(d.text("()")),
                }
            } else {
                parts.push(d.text("()"));
            }
            return d.concat(&parts);
        }

        // Build callee with type args: `new Foo<K, V>`
        let callee_with_types = d.concat(&[d.text("new "), callee_with_types_base]);

        // Single huggable argument: object literal or function
        // These stay on the same line as the opening paren: `new Cls({...})` not `new Cls(\n{...})`
        // Skip hugging if there are trailing comments (line OR block) - let the comment handling below handle it
        let single_arg_has_trailing_comment = new_expr.arguments.len() == 1
            && has_trailing_comments_slice(&new_expr.arguments, new_expr.span.end, self);

        if new_expr.arguments.len() == 1 && !single_arg_has_trailing_comment {
            match &new_expr.arguments[0] {
                // Object literal: hug it
                internal::Expression::ObjectExpression(_) => {
                    return d.concat(&[
                        callee_with_types,
                        d.text("("),
                        self.build_expression_doc(&new_expr.arguments[0]),
                        d.text(")"),
                    ]);
                }
                // Array literal: hug it
                internal::Expression::ArrayExpression(_) => {
                    return d.concat(&[
                        callee_with_types,
                        d.text("("),
                        self.build_expression_doc(&new_expr.arguments[0]),
                        d.text(")"),
                    ]);
                }
                // Block arrow (or expandable arrow chain): use conditional_group to let Doc decide hug vs wrap
                internal::Expression::ArrowFunctionExpression(arrow)
                    if !arrow.body.is_expression()
                        || (!arrow_has_type_reference_return(arrow)
                            && could_expand_arrow_chain(arrow)) =>
                {
                    let mut arrow_doc = self.build_expression_doc(&new_expr.arguments[0]);

                    // Prepend leading comments (e.g., /** @param {any} x */ before arrow)
                    // and force wrapped state when present (prettier expands args with leading comments)
                    let paren_open = new_expr
                        .type_arguments
                        .as_ref()
                        .map_or_else(|| new_expr.callee.span().end, |ta| ta.span.end);
                    let arg_start = new_expr.arguments[0].span().start;
                    let has_leading_comment =
                        if let Some(leading) = self.build_rhs_comments_opt(paren_open, arg_start) {
                            arrow_doc = d.concat(&[leading, arrow_doc]);
                            true
                        } else {
                            false
                        };

                    // If the arrow has trailing param comments or leading comments,
                    // force wrapped state
                    let arrow_token = self.find_arrow_token_for(arrow);
                    let has_trailing_param_comments =
                        arrow_has_trailing_param_comments(arrow, arrow_token, |start, end| {
                            self.has_comments_between(start, end)
                        });

                    if has_trailing_param_comments || has_leading_comment {
                        return d.concat(&[
                            callee_with_types,
                            d.text("("),
                            d.indent(d.concat(&[d.softline(), arrow_doc, d.text(",")])),
                            d.softline(),
                            d.text(")"),
                        ]);
                    }

                    return d.conditional_group(&[
                        // State 1: hugged - new Callee((arrow) => { body })
                        d.concat(&[callee_with_types, d.text("("), arrow_doc, d.text(")")]),
                        // State 2: wrapped - new Callee(\n\t(arrow) => { body },\n)
                        d.concat(&[
                            callee_with_types,
                            d.text("("),
                            d.indent(d.concat(&[d.softline(), arrow_doc, d.text(",")])),
                            d.softline(),
                            d.text(")"),
                        ]),
                    ]);
                }
                // Function expression: hug it
                internal::Expression::FunctionExpression(_) => {
                    return d.concat(&[
                        callee_with_types,
                        d.text("("),
                        self.build_expression_doc(&new_expr.arguments[0]),
                        d.text(")"),
                    ]);
                }
                // Expression-body arrow: break at => not at (
                // Mirrors call_formatting.rs expression arrow handling
                internal::Expression::ArrowFunctionExpression(arrow)
                    if arrow.body.is_expression() =>
                {
                    if let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body {
                        // Expandable body (ternary): conditional parens
                        // Flat: `new Xy((x) => (x ? y : z))`
                        // Break: `new Xy((x) =>\n  x ? y : z,\n)`
                        // Arrows with TSTypeReference return types are NOT expandable.
                        if is_ternary_arrow_body(body_expr)
                            && !arrow_has_type_reference_return(arrow)
                        {
                            let sig_doc = build_arrow_sig_doc(self, arrow);
                            let body_doc = self.build_expression_doc(body_expr);
                            let body_doc = prepend_arrow_body_comments(
                                self,
                                arrow,
                                body_expr.span().start,
                                body_doc,
                            );

                            let state_break = d.concat(&[
                                callee_with_types,
                                d.text("("),
                                sig_doc,
                                d.text(" =>"),
                                d.indent(d.concat(&[d.hardline(), body_doc, d.text(",")])),
                                d.hardline(),
                                d.text(")"),
                            ]);

                            if d.will_break(body_doc) {
                                return state_break;
                            }

                            let state_flat = d.concat(&[
                                callee_with_types,
                                d.text("("),
                                sig_doc,
                                d.text(" => ("),
                                body_doc,
                                d.text("))"),
                            ]);

                            let state_all_broken = d.concat(&[
                                callee_with_types,
                                d.text("("),
                                d.indent(d.concat(&[
                                    d.hardline(),
                                    sig_doc,
                                    d.text(" =>"),
                                    d.indent(d.concat(&[
                                        d.hardline(),
                                        body_doc,
                                        d.trailing_comma(),
                                    ])),
                                ])),
                                d.hardline(),
                                d.text(")"),
                            ]);

                            return d.conditional_group(&[
                                state_flat,
                                state_break,
                                state_all_broken,
                            ]);
                        }

                        // Simple call body: 2-state break at =>
                        // Arrows with TSTypeReference return types are NOT expandable.
                        if matches!(&**body_expr, internal::Expression::CallExpression(_))
                            && !arrow_has_type_reference_return(arrow)
                        {
                            let arrow_doc = self.build_expression_doc(&new_expr.arguments[0]);
                            let body_doc = self.build_expression_doc(body_expr);
                            let body_doc = prepend_arrow_body_comments(
                                self,
                                arrow,
                                body_expr.span().start,
                                body_doc,
                            );
                            let sig_doc = build_arrow_sig_doc(self, arrow);
                            return build_arrow_call_body_states(
                                d,
                                callee_with_types,
                                arrow_doc,
                                sig_doc,
                                body_doc,
                            );
                        }
                    }
                    // Non-call/non-expandable expression body or typed arrows: fall through
                }
                _ => {}
            }
        }

        // Compute paren_open: position after callee and type args (just before `(`)
        let paren_open = new_expr
            .type_arguments
            .as_ref()
            .map_or_else(|| new_expr.callee.span().end, |ta| ta.span.end);

        // Function composition pattern: when any argument is a call containing a callback
        // OR when there are multiple function arguments
        // e.g., new Cls(arr.map((x) => x), b) → new Cls(\n\t...,\n)
        // e.g., new Cls(() => a, () => b) → new Cls(\n\t...,\n)
        // Skip this path if there are trailing comments - let the comment handling paths handle it
        if is_function_composition_args(&new_expr.arguments)
            && !has_trailing_comments_slice(&new_expr.arguments, new_expr.span.end, self)
        {
            let arg_parts = build_args_joined_with_comments(
                self,
                &new_expr.arguments,
                paren_open,
                true,
                #[allow(clippy::redundant_closure_for_method_calls)]
                |p, a| p.build_arg_expression_doc(a),
            );
            return wrap_call_with_hard_breaks(d, callee_with_types, arg_parts);
        }

        // Single template literal argument with embedded newlines:
        // Same logic as CallExpression (call_formatting.rs) — hug when template
        // is on the same line as (, expand when it's on its own line.
        if new_expr.arguments.len() == 1 && is_multiline_template_expression(&new_expr.arguments[0])
        {
            let template_start = new_expr.arguments[0].span().start;
            if !has_newline_before_position(self.source, template_start) {
                // Template on same line as ( — hug it
                let arg_doc = self.build_expression_doc(&new_expr.arguments[0]);
                let mut parts = vec![callee_with_types, d.text("("), arg_doc, d.text(")")];

                // Add trailing comments as line suffix
                let last_arg = &new_expr.arguments[0];
                let paren_close = new_expr.span.end;
                if let Some(suffix) =
                    self.build_trailing_comments_line_suffix(last_arg.span().end, paren_close)
                {
                    parts.push(suffix);
                }

                return d.concat(&parts);
            }
            // Template on its own line — fall through to has_multiline_content
        }

        // Check if any argument has multiline content
        let has_multiline = new_expr
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, self.source));

        if has_multiline {
            // Force expansion with hardlines for multiline content
            let arg_parts = build_args_joined_with_comments(
                self,
                &new_expr.arguments,
                paren_open,
                true,
                #[allow(clippy::redundant_closure_for_method_calls)]
                |p, a| p.build_arg_expression_doc(a),
            );
            return wrap_call_with_hard_breaks(d, callee_with_types, arg_parts);
        }

        // Check for blank lines between arguments (forces expansion and preservation)
        let has_blank_lines = new_expr.arguments.windows(2).any(|window| {
            has_blank_line_between_args(
                self.source,
                self.line_breaks,
                window[0].span().end,
                window[1].span().start,
            )
        });

        if has_blank_lines {
            let mut arg_parts = Vec::new();
            for (i, arg) in new_expr.arguments.iter().enumerate() {
                // Check for blank line before this arg (no-comment case only).
                // When comments exist, blank lines are handled in the separator
                // logic of the previous iteration.
                if i > 0 {
                    let prev_end = new_expr.arguments[i - 1].span().end;
                    let curr_start = arg.span().start;
                    if !self.has_comments_between(prev_end, curr_start)
                        && has_blank_line_between_args(
                            self.source,
                            self.line_breaks,
                            prev_end,
                            curr_start,
                        )
                    {
                        arg_parts.push(d.literalline());
                        arg_parts.push(d.hardline());
                    }
                }

                arg_parts.push(self.build_expression_doc(arg));

                if i < new_expr.arguments.len() - 1 {
                    let arg_end = arg.span().end;
                    let next_start = new_expr.arguments[i + 1].span().start;

                    if self.has_comments_between(arg_end, next_start) {
                        let pc = PartitionedComments::new(
                            self.comments,
                            self.line_breaks,
                            arg_end,
                            next_start,
                        );

                        arg_parts.push(d.text(","));
                        pc.emit_trailing_comments(&mut arg_parts, self);

                        let next_has_blank = pc.has_blank_line_in_gap(
                            self.source,
                            self.line_breaks,
                            arg_end,
                            next_start,
                        );
                        if next_has_blank {
                            arg_parts.push(d.literalline());
                        }
                        arg_parts.push(d.hardline());
                        pc.emit_leading_comments(&mut arg_parts, self);
                    } else {
                        arg_parts.push(d.text(","));
                        // Skip hardline if next arg has blank line
                        // (handled at top of next iteration)
                        let next_has_blank = has_blank_line_between_args(
                            self.source,
                            self.line_breaks,
                            arg_end,
                            next_start,
                        );
                        if !next_has_blank {
                            arg_parts.push(d.hardline());
                        }
                    }
                }
            }

            let arg_doc = d.concat(&arg_parts);
            return wrap_call_with_hard_breaks(d, callee_with_types, arg_doc);
        }

        // "Expand first arg" pattern: callback first, short/empty container last
        // e.g., new Proxy((x) => { ... }, {}) - callback hugs, empty obj stays inline
        if new_expr.arguments.len() == 2 {
            let first_arg = &new_expr.arguments[0];
            let second_arg = &new_expr.arguments[1];

            if is_block_function(first_arg)
                // Prettier's couldExpandArg returns true for objects/arrays with leading
                // comments (hasComment includes leading). Block expand-first to avoid
                // dropping the comment (SAFETY).
                && !(matches!(
                    second_arg,
                    internal::Expression::ObjectExpression(_)
                        | internal::Expression::ArrayExpression(_)
                ) && self.has_comments_between(first_arg.span().end, second_arg.span().start))
                && is_short_second_arg_for_expand_first(second_arg, |start, end| {
                    self.has_comments_between(start, end)
                })
            {
                let first_arg_doc = self.build_expression_doc(first_arg);
                let second_arg_doc = self.build_expression_doc(second_arg);

                return d.concat(&[
                    callee_with_types,
                    d.text("("),
                    first_arg_doc,
                    d.text(", "),
                    second_arg_doc,
                    d.text(")"),
                ]);
            }
        }

        // Check for trailing LINE comments on arguments (forces hardline expansion)
        // Must check this BEFORE the "last arg is array/object" pattern below,
        // otherwise trailing comments on the last arg cause it to be hugged incorrectly.
        // e.g., new Class(arg1, // comment\n  arg2)
        if has_trailing_line_comments_slice(&new_expr.arguments, new_expr.span.end, self) {
            let mut arg_parts = Vec::new();
            let mut has_trailing_comma_on_last = false;

            for (i, arg) in new_expr.arguments.iter().enumerate() {
                // Build the argument (use build_expression_doc to match calls.rs comment handling)
                arg_parts.push(self.build_expression_doc(arg));

                // Check for comments after this argument
                if i < new_expr.arguments.len() - 1 {
                    let arg_end = arg.span().end;
                    let next_arg_start = new_expr.arguments[i + 1].span().start;

                    let pc = PartitionedComments::new(
                        self.comments,
                        self.line_breaks,
                        arg_end,
                        next_arg_start,
                    );

                    arg_parts.push(d.text(","));
                    pc.emit_trailing_comments(&mut arg_parts, self);
                    arg_parts.push(d.hardline());
                    pc.emit_leading_comments(&mut arg_parts, self);
                } else {
                    // Last argument - check for trailing comments before closing paren
                    let arg_end = arg.span().end;
                    let paren_close = new_expr.span.end;

                    let pc = PartitionedComments::new(
                        self.comments,
                        self.line_breaks,
                        arg_end,
                        paren_close,
                    );

                    if pc.has_trailing_line() {
                        // Line comment present - need to handle block comments specially
                        // If both block and line: `() => {} /* block */, // line`
                        // If only line: `() => {}, // line`
                        for comment in &pc.trailing_block {
                            arg_parts.push(d.text(" "));
                            arg_parts.push(self.build_comment_doc(comment));
                        }
                        arg_parts.push(d.text(","));
                        for comment in &pc.trailing_line {
                            arg_parts.push(d.text(" "));
                            arg_parts.push(self.build_comment_doc(comment));
                        }
                        has_trailing_comma_on_last = true;
                    } else if pc.has_trailing_block() {
                        // Block comment only in hardline expansion: emit comment before comma
                        // e.g., new A(() => {}, () => {} /* comment */,)
                        pc.emit_trailing_comments(&mut arg_parts, self);
                        // has_trailing_comma_on_last stays false, so trailing comma will be added
                    }
                }
            }

            let arg_doc = d.concat(&arg_parts);
            let trailing = if has_trailing_comma_on_last {
                d.empty()
            } else {
                d.text(",")
            };

            return d.concat(&[
                callee_with_types,
                d.text("("),
                d.indent(d.concat(&[d.hardline(), arg_doc, trailing])),
                d.hardline(),
                d.text(")"),
            ]);
        }

        // Check for trailing BLOCK comments only (no line comments)
        // Block comments should stay inline for simple args: new A(a, b /* comment */)
        // But function composition cases should expand: new A(() => {}, () => {} /* comment */,)
        let has_trailing_block_only = new_expr.arguments.last().is_some_and(|last_arg| {
            let arg_end = last_arg.span().end;
            let paren_close = new_expr.span.end;
            self.has_comments_between(arg_end, paren_close)
                && !self.has_line_comments_between(arg_end, paren_close)
        });

        if has_trailing_block_only {
            // Build args with trailing block comment
            let last_idx = new_expr.arguments.len() - 1;
            let mut arg_docs: Vec<DocId> = new_expr
                .arguments
                .iter()
                .map(|arg| self.build_arg_expression_doc(arg))
                .collect();

            // Add trailing block comment to last arg
            let last_arg = &new_expr.arguments[last_idx];
            let mut effective_arg_end = last_arg.span().end;

            // For spread elements, also check inside the spread span
            if let internal::Expression::SpreadElement(spread) = last_arg
                && self.has_comments_between(spread.argument.span().end, spread.span.end)
            {
                effective_arg_end = spread.argument.span().end;
            }

            let pc = PartitionedComments::new(
                self.comments,
                self.line_breaks,
                effective_arg_end,
                new_expr.span.end,
            );

            // Own-line block comments after the last arg (before closing paren).
            // These appear as siblings after the trailing comma, forcing expansion.
            let leading_block: Vec<_> = pc.leading.iter().filter(|c| c.is_block).collect();
            if !leading_block.is_empty()
                && let Some(last_doc) = arg_docs.pop()
            {
                let mut last_parts = vec![last_doc, d.text(",")];
                for comment in &leading_block {
                    last_parts.push(d.hardline());
                    last_parts.push(self.build_comment_doc(comment));
                }
                arg_docs.push(d.concat(&last_parts));

                let arg_parts = if new_expr.arguments.len() > 1 {
                    d.join_doc(arg_docs, d.comma_hardline())
                } else {
                    d.concat(&arg_docs)
                };
                return d.concat(&[
                    callee_with_types,
                    d.text("("),
                    d.indent(d.concat(&[d.hardline(), arg_parts])),
                    d.hardline(),
                    d.text(")"),
                ]);
            }

            if let Some(last_doc) = arg_docs.pop() {
                let mut last_with_comment = vec![last_doc];
                pc.emit_trailing_comments(&mut last_with_comment, self);
                arg_docs.push(d.concat(&last_with_comment));

                // For function composition (multiple callbacks), use hardlines
                // For simple args, use soft breaks (can stay inline)
                if is_function_composition_args(&new_expr.arguments) {
                    let arg_parts = d.join_doc(arg_docs, d.comma_hardline());
                    return wrap_call_with_hard_breaks(d, callee_with_types, arg_parts);
                }
                let arg_parts = d.join_doc(arg_docs, d.comma_line());
                return wrap_call_with_soft_breaks(d, callee_with_types, arg_parts);
            }
        }

        // "Expand last arg" pattern — matches call_formatting.rs logic.
        // Split into function/arrow last arg and array/object last arg paths.
        // NOTE: This must come AFTER the trailing comment check above.
        {
            let last_arg = new_expr.arguments.last();
            let last_is_function = matches!(
                last_arg,
                Some(
                    internal::Expression::ArrowFunctionExpression(_)
                        | internal::Expression::FunctionExpression(_)
                )
            );
            let last_is_expandable_collection = last_arg.is_some_and(|arg| {
                is_array_or_object_unwrapped(arg) && !is_concise_numeric_array(arg)
            });

            if new_expr.arguments.len() >= 2
                && (last_is_function || last_is_expandable_collection)
                && preceding_args_allow_expand_last(&new_expr.arguments, self.line_breaks)
                && !has_inter_argument_comments_slice(&new_expr.arguments, self)
            {
                let (head_parts, last_arg_doc, all_args_broken) =
                    build_args_split_last(&new_expr.arguments, self, paren_open);

                // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
                if head_parts.iter().any(|&id| d.will_break(id)) {
                    return build_expand_all_args(d, callee_with_types, all_args_broken);
                }

                if last_is_function {
                    // Function/arrow last arg path (matches call_formatting.rs:714-866)
                    // Expression arrows with call/conditional body get break-body state
                    if let Some(internal::Expression::ArrowFunctionExpression(arrow)) =
                        new_expr.arguments.last()
                        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
                        && matches!(
                            &**body_expr,
                            internal::Expression::CallExpression(_)
                                | internal::Expression::ConditionalExpression(_)
                        )
                        && !arrow_has_type_reference_return(arrow)
                    {
                        let sig_doc = build_arrow_sig_doc(self, arrow);
                        let body_doc = self.build_expression_doc(body_expr);
                        let body_doc = prepend_arrow_body_comments(
                            self,
                            arrow,
                            body_expr.span().start,
                            body_doc,
                        );

                        let prefix = d.concat(&[callee_with_types, d.text("(")]);
                        let state_break_body =
                            build_break_body_state(d, prefix, &head_parts, sig_doc, body_doc);

                        let state_expand_all =
                            build_expand_all_args(d, callee_with_types, all_args_broken);

                        // Prettier: when willBreak(lastArg), skip flat state
                        if d.will_break(last_arg_doc) {
                            return d.conditional_group(&[state_break_body, state_expand_all]);
                        }

                        let state_inline =
                            build_inline_args(d, callee_with_types, head_parts, last_arg_doc);

                        return d.conditional_group(&[
                            state_inline,
                            state_break_body,
                            state_expand_all,
                        ]);
                    }

                    // Block-body arrow/function: inline vs expand-all (no hug state)
                    let state_inline =
                        build_inline_args(d, callee_with_types, head_parts, last_arg_doc);
                    let state_expand_all =
                        build_expand_all_args(d, callee_with_types, all_args_broken);
                    return d.conditional_group(&[state_inline, state_expand_all]);
                }

                // Array/object last arg path (matches call_formatting.rs:870-999)
                // Same outer type: skip hug, use expand-all
                if last_two_args_same_type(&new_expr.arguments) {
                    // Same type: Prettier uses expand-all when last arg will break
                    if d.will_break(last_arg_doc) {
                        return build_expand_all_args(d, callee_with_types, all_args_broken);
                    }
                    return build_inline_or_expand_all(
                        d,
                        callee_with_types,
                        head_parts,
                        last_arg_doc,
                        all_args_broken,
                    );
                }

                // Different types: if last arg has forced breaks, use inline-or-expand-all
                if d.has_forced_break(last_arg_doc) {
                    return build_inline_or_expand_all(
                        d,
                        callee_with_types,
                        head_parts,
                        last_arg_doc,
                        all_args_broken,
                    );
                }

                // No forced breaks: 3-state (inline → hug → expand all)
                let state_inline =
                    build_inline_args(d, callee_with_types, head_parts.clone(), last_arg_doc);
                let state_hug = d.concat(&[
                    callee_with_types,
                    d.text("("),
                    d.concat(&head_parts),
                    d.group_break(last_arg_doc),
                    d.text(")"),
                ]);
                let state_expand_all = build_expand_all_args(d, callee_with_types, all_args_broken);
                return d.conditional_group(&[state_inline, state_hug, state_expand_all]);
            }
        }

        // Check for leading comments or inter-argument block comments
        // These need explicit handling that the simple join_doc path doesn't provide
        let has_leading_comments = !new_expr.arguments.is_empty()
            && self.has_comments_between(paren_open, new_expr.arguments[0].span().start);
        let has_inter_arg_comments = has_inter_argument_comments_slice(&new_expr.arguments, self);

        if has_leading_comments || has_inter_arg_comments {
            let arg_parts = build_args_joined_with_comments(
                self,
                &new_expr.arguments,
                paren_open,
                false,
                #[allow(clippy::redundant_closure_for_method_calls)]
                |p, a| p.build_expression_doc(a),
            );
            return wrap_call_with_will_break_guard(d, callee_with_types, arg_parts);
        }

        // Build args with line separators (one per line when broken)
        let arg_parts = d.join_doc(
            new_expr
                .arguments
                .iter()
                .map(|arg| self.build_arg_expression_doc(arg)),
            d.comma_line(),
        );

        // Wrap in group with parens, forcing break when args contain hardlines
        wrap_call_with_will_break_guard(d, callee_with_types, arg_parts)
    }

    /// Build a Doc for a new expression (for nested contexts)
    pub(super) fn build_new_doc(&self, new_expr: &internal::NewExpression) -> DocId {
        self.build_new_doc_with_wrapping(new_expr)
    }
}
