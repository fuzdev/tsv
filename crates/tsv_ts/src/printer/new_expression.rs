// New expression printing for TypeScript
//
// Handles: new Foo(), new Foo(arg1, arg2), new Foo<T>()

use super::calls::{
    PartitionedComments, build_args_split_last, find_comma_pos, has_inter_argument_comments_slice,
    has_trailing_comments_slice, has_trailing_line_comments_slice, is_comment_after_comma,
    is_comment_before_comma, wrap_call_with_hard_breaks, wrap_call_with_soft_breaks,
};
use super::utils::{
    arrow_has_trailing_param_comments, is_block_function, is_function_composition_args,
    is_short_second_arg_for_expand_first, last_arg_is_array_or_object, preceding_args_allow_hug,
};
use super::{ParenContext, Printer, has_multiline_content, needs_parens};
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

        // Empty args: just `new Foo()` or `new Foo<K, V>()`
        if new_expr.arguments.is_empty() {
            let mut parts = vec![d.text("new "), callee];
            if let Some(ta_doc) = type_args_doc {
                parts.push(ta_doc);
            }
            parts.push(d.text("()"));
            return d.concat(&parts);
        }

        // Build callee with type args: `new Foo<K, V>`
        let callee_with_types = match type_args_doc {
            Some(ta_doc) => d.concat(&[d.text("new "), callee, ta_doc]),
            None => d.concat(&[d.text("new "), callee]),
        };

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
                // Block arrow function: use conditional_group to let Doc decide hug vs wrap
                internal::Expression::ArrowFunctionExpression(arrow)
                    if !arrow.body.is_expression() =>
                {
                    let arrow_doc = self.build_expression_doc(&new_expr.arguments[0]);

                    // If the arrow has trailing param comments, force wrapped state
                    let has_trailing_param_comments =
                        arrow_has_trailing_param_comments(arrow, |start, end| {
                            self.has_comments_between(start, end)
                        });

                    if has_trailing_param_comments {
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
                _ => {}
            }
        }

        // Function composition pattern: when any argument is a call containing a callback
        // OR when there are multiple function arguments
        // e.g., new Cls(arr.map((x) => x), b) → new Cls(\n\t...,\n)
        // e.g., new Cls(() => a, () => b) → new Cls(\n\t...,\n)
        // Skip this path if there are trailing comments - let the comment handling paths handle it
        if is_function_composition_args(&new_expr.arguments)
            && !has_trailing_comments_slice(&new_expr.arguments, new_expr.span.end, self)
        {
            let arg_parts = d.join_doc(
                new_expr
                    .arguments
                    .iter()
                    .map(|arg| self.build_arg_expression_doc(arg)),
                d.comma_hardline(),
            );

            return wrap_call_with_hard_breaks(d, callee_with_types, arg_parts);
        }

        // Check if any argument has multiline content
        let has_multiline = new_expr
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, self.source));

        if has_multiline {
            // Force expansion with hardlines for multiline content
            let arg_parts = d.join_doc(
                new_expr
                    .arguments
                    .iter()
                    .map(|arg| self.build_arg_expression_doc(arg)),
                d.comma_hardline(),
            );

            return wrap_call_with_hard_breaks(d, callee_with_types, arg_parts);
        }

        // "Expand first arg" pattern: callback first, short/empty container last
        // e.g., new Proxy((x) => { ... }, {}) - callback hugs, empty obj stays inline
        if new_expr.arguments.len() == 2 {
            let first_arg = &new_expr.arguments[0];
            let second_arg = &new_expr.arguments[1];

            if is_block_function(first_arg)
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
            let pc = PartitionedComments::new(
                self.comments,
                self.line_breaks,
                last_arg.span().end,
                new_expr.span.end,
            );

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

        // "First args inline with last array/object" pattern (same as CallExpression):
        // When last arg is array/object and preceding args are short,
        // keep short args inline with the opening bracket/brace.
        // NOTE: This must come AFTER the trailing comment check above.
        if new_expr.arguments.len() >= 2
            && last_arg_is_array_or_object(&new_expr.arguments)
            && preceding_args_allow_hug(&new_expr.arguments, self.line_breaks)
            && !has_inter_argument_comments_slice(&new_expr.arguments, self)
        {
            let (head_parts, last_arg_doc, _) = build_args_split_last(&new_expr.arguments, self);

            // Keep short args inline with last arg's opener
            return d.group(d.concat(&[
                callee_with_types,
                d.text("("),
                d.concat(&head_parts),
                last_arg_doc,
                d.text(")"),
            ]));
        }

        // Check for leading comments or inter-argument block comments
        // These need explicit handling that the simple join_doc path doesn't provide
        let paren_open = new_expr.callee.span().end;
        let has_leading_comments = !new_expr.arguments.is_empty()
            && self.has_comments_between(paren_open, new_expr.arguments[0].span().start);
        let has_inter_arg_comments = has_inter_argument_comments_slice(&new_expr.arguments, self);

        if has_leading_comments || has_inter_arg_comments {
            // Build arguments with explicit comment handling
            let mut arg_parts = Vec::new();

            for (i, arg) in new_expr.arguments.iter().enumerate() {
                // Handle leading comments before first argument
                if i == 0 && has_leading_comments {
                    let first_arg_start = arg.span().start;
                    arg_parts.push(self.build_inline_comments_between_doc_no_leading_space(
                        paren_open,
                        first_arg_start,
                    ));
                    arg_parts.push(d.line());
                }

                // Build the argument
                arg_parts.push(self.build_expression_doc(arg));

                // Check for comments after this argument (before next arg or closing paren)
                if i < new_expr.arguments.len() - 1 {
                    let arg_end = arg.span().end;
                    let next_arg_start = new_expr.arguments[i + 1].span().start;

                    if self.has_comments_between(arg_end, next_arg_start) {
                        let pc = PartitionedComments::new(
                            self.comments,
                            self.line_breaks,
                            arg_end,
                            next_arg_start,
                        );

                        let comma_pos = find_comma_pos(self.source, arg_end, next_arg_start);

                        if pc.has_trailing_line() {
                            // Trailing line comments: comma, comment, hardline
                            arg_parts.push(d.text(","));
                            for comment in &pc.trailing_line {
                                arg_parts.push(d.text(" "));
                                arg_parts.push(self.build_comment_doc(comment));
                            }
                            arg_parts.push(d.hardline());
                        } else if pc.has_trailing_block() {
                            // Trailing block comments: place relative to comma based on source position
                            if let Some(cpos) = comma_pos {
                                for comment in &pc.trailing_block {
                                    if is_comment_before_comma(comment, cpos) {
                                        arg_parts.push(d.text(" "));
                                        arg_parts.push(self.build_comment_doc(comment));
                                    }
                                }
                            }
                            arg_parts.push(d.text(","));
                            if let Some(cpos) = comma_pos {
                                for comment in &pc.trailing_block {
                                    if is_comment_after_comma(comment, cpos) {
                                        arg_parts.push(d.text(" "));
                                        arg_parts.push(self.build_comment_doc(comment));
                                    }
                                }
                            }
                            arg_parts.push(d.line());
                        } else {
                            // No trailing comments, add comma and line
                            arg_parts.push(d.text(","));
                            arg_parts.push(d.line());
                        }

                        // Add leading comments for next arg
                        pc.emit_leading_comments_inline_aware(&mut arg_parts, self, next_arg_start);
                    } else {
                        // No comments, just comma and line
                        arg_parts.push(d.comma_line());
                    }
                }
            }

            let arg_doc = d.concat(&arg_parts);
            return wrap_call_with_soft_breaks(d, callee_with_types, arg_doc);
        }

        // Build args with line separators (one per line when broken)
        let arg_parts = d.join_doc(
            new_expr
                .arguments
                .iter()
                .map(|arg| self.build_arg_expression_doc(arg)),
            d.comma_line(),
        );

        // Wrap in group with parens
        wrap_call_with_soft_breaks(d, callee_with_types, arg_parts)
    }

    /// Build a Doc for a new expression (for nested contexts)
    pub(super) fn build_new_doc(&self, new_expr: &internal::NewExpression) -> DocId {
        self.build_new_doc_with_wrapping(new_expr)
    }
}
