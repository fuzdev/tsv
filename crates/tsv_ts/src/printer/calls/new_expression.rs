// New expression printing for TypeScript
//
// Handles: new Foo(), new Foo(arg1, arg2), new Foo<T>()

use super::arg_comments::{find_comma_pos, is_comment_after_comma};
use super::arg_wrapping::{
    append_type_args_with_gap_comments, build_args_with_blank_lines, build_empty_args_doc,
    should_expand_first_arg, try_hug_multiline_template_arg, wrap_call_with_hard_breaks_suffix,
    wrap_call_with_soft_breaks_suffix,
};
use crate::ast::internal;
use crate::printer::calls::arg_predicates::{
    arrow_body_is_call_through_non_null, arrow_has_trailing_param_comments,
    is_array_or_object_unwrapped, is_concise_numeric_array, is_function_composition_args,
    is_ternary_arrow_body, preceding_args_allow_expand_last,
};
use crate::printer::calls::{
    PartitionedComments, build_args_joined_with_comments, build_args_split_last,
    build_arrow_call_body_states, build_arrow_sig_doc, build_break_body_state,
    build_expand_all_args, build_inline_args, build_inline_or_expand_all, could_expand_arrow_chain,
    emit_first_arg_leading_comments, has_blank_line_between_args,
    has_inter_argument_comments_slice, has_trailing_comments_slice,
    has_trailing_line_comments_slice, last_two_args_same_type, prepend_arrow_body_comments,
    should_force_expansion_for_comments, wrap_call_with_hard_breaks,
    wrap_call_with_will_break_guard,
};
use crate::printer::{CommentVec, ParenContext, Printer, has_multiline_content};
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a new expression with argument wrapping
    pub(in crate::printer) fn build_new_doc_with_wrapping(
        &self,
        new_expr: &internal::NewExpression<'_>,
    ) -> DocId {
        let d = self.d();
        // Wrap callee in parens if needed (e.g., `new (a || b)()`, `new (a ? b : c)()`,
        // an optional chain `new (a?.b)()` — a chain can't be a `new` callee per spec).
        // A non-null assertion sealing a parenthesized chain (`new (a?.b)!()`) keeps the
        // parens via the sealed-base rendering (checked first; the `!`-outside form is
        // not stripped here even though the standalone path would).
        let callee = if let Some(sealed) = self.build_sealed_non_null_paren_doc(new_expr.callee) {
            sealed
        } else if self.needs_parens(new_expr.callee, ParenContext::NewCallee) {
            // For binary expressions (including logical), use a group with softlines
            // so the parens can break independently when the content is too long:
            // new (
            //     a || b || c
            // )()
            //
            // Use ungrouped binary doc so the inner expression doesn't have its own
            // group - the outer group controls whether to break after `(`.
            if let internal::Expression::BinaryExpression(binary) = new_expr.callee {
                let inner_doc = self.build_binary_chain_doc_ungrouped(binary);
                d.group(d.concat(&[
                    d.text("("),
                    d.indent_softline(inner_doc),
                    d.softline(),
                    d.text(")"),
                ]))
            } else {
                let callee_doc = self.build_expression_doc(new_expr.callee);
                d.parens(callee_doc)
            }
        } else {
            self.build_expression_doc(new_expr.callee)
        };

        // Check for comments between removed parentheses and callee
        // e.g., new (/* comment */ Foo)() has comments in the gap between 'new ' and 'Foo'
        let callee = self.prepend_removed_paren_comments(
            new_expr.span.start,
            new_expr.callee.span().start,
            callee,
        );

        // Combine callee with type arguments (`new Foo<K, V>`), preserving comments
        // in the gap, e.g. `new Foo/* c */ <string>()` — comment between callee and `<`
        let callee_with_types_base = append_type_args_with_gap_comments(
            self,
            callee,
            new_expr.callee.span().end,
            new_expr.type_arguments.as_ref(),
        );

        // Empty args: just `new Foo()` or `new Foo<K, V>()`, preserving dangling comments
        if new_expr.arguments.is_empty() {
            let after_type_args = new_expr
                .type_arguments
                .as_ref()
                .map_or_else(|| new_expr.callee.span().end, |ta| ta.span.end);
            return build_empty_args_doc(
                self,
                d.concat(&[d.text("new "), callee_with_types_base]),
                after_type_args,
                new_expr.span.end,
            );
        }

        // Build callee with type args: `new Foo<K, V>`
        let callee_with_types = d.concat(&[d.text("new "), callee_with_types_base]);

        // Single huggable argument: object literal or function
        // These stay on the same line as the opening paren: `new Cls({...})` not `new Cls(\n{...})`
        // Skip hugging if there are trailing comments (line OR block) - let the comment handling below handle it
        let single_arg_has_trailing_comment = new_expr.arguments.len() == 1
            && has_trailing_comments_slice(new_expr.arguments, new_expr.span.end, self);

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
                    if !arrow.body.is_expression() || could_expand_arrow_chain(arrow) =>
                {
                    let mut arrow_doc = self.build_expression_doc(&new_expr.arguments[0]);

                    // Prepend leading comments (e.g., /** @param {any} x */ before arrow)
                    // and force wrapped state when present (prettier expands args with leading comments)
                    let paren_open = new_expr
                        .type_arguments
                        .as_ref()
                        .map_or_else(|| new_expr.callee.span().end, |ta| ta.span.end);
                    let arg_start = new_expr.arguments[0].span().start;
                    // Glued like the regular-call leading-arg paths (prettier shares
                    // one `printCallArguments` for Call and New): a single-line block
                    // hugged to `(` stays with the argument across a source newline.
                    let has_leading_comment = if let Some(leading) =
                        self.build_rhs_comments_glued_opt(paren_open, arg_start)
                    {
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
                            d.indent(d.concat(&[d.softline(), arrow_doc])),
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
                            d.indent(d.concat(&[d.softline(), arrow_doc])),
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
                        // couldExpandArg keys on the body type, looking through the
                        // return-type annotation, so typed-return arrows are eligible.
                        if is_ternary_arrow_body(body_expr) {
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
                                d.indent(d.concat(&[d.hardline(), body_doc])),
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
                                    d.indent(d.concat(&[d.hardline(), body_doc])),
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
                        // couldExpandArg keys on the body type, looking through the
                        // return-type annotation and a trailing non-null `!`.
                        if arrow_body_is_call_through_non_null(body_expr) {
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
        if is_function_composition_args(new_expr.arguments)
            && !has_trailing_comments_slice(new_expr.arguments, new_expr.span.end, self)
        {
            let arg_parts = build_args_joined_with_comments(
                self,
                new_expr.arguments,
                paren_open,
                true,
                #[allow(clippy::redundant_closure_for_method_calls)]
                |p, a| p.build_arg_expression_doc(a),
            );
            return wrap_call_with_hard_breaks(d, callee_with_types, arg_parts);
        }

        // Single template literal argument with embedded newlines on the same line
        // as `(` — hug it. A template on its own line falls through to
        // has_multiline_content below.
        if let Some(doc) = try_hug_multiline_template_arg(
            self,
            callee_with_types,
            new_expr.arguments,
            new_expr.span.end,
        ) {
            return doc;
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
                new_expr.arguments,
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
            let arg_doc = build_args_with_blank_lines(self, new_expr.arguments);
            return wrap_call_with_hard_breaks(d, callee_with_types, arg_doc);
        }

        // "Expand first arg" pattern: callback first, short/empty container last
        // e.g., new Proxy((x) => { ... }, {}) - callback hugs, empty obj stays inline
        if should_expand_first_arg(self, new_expr.arguments) {
            let first_arg_doc = self.build_expression_doc(&new_expr.arguments[0]);
            let second_arg_doc = self.build_expression_doc(&new_expr.arguments[1]);

            return d.concat(&[
                callee_with_types,
                d.text("("),
                first_arg_doc,
                d.text(", "),
                second_arg_doc,
                d.text(")"),
            ]);
        }

        // Check for trailing LINE comments on arguments (forces hardline expansion)
        // Must check this BEFORE the "last arg is array/object" pattern below,
        // otherwise trailing comments on the last arg cause it to be hugged incorrectly.
        // e.g., new Class(arg1, // comment\n  arg2)
        if has_trailing_line_comments_slice(new_expr.arguments, new_expr.span.end, self) {
            let mut arg_parts = DocBuf::new();

            for (i, arg) in new_expr.arguments.iter().enumerate() {
                // Leading comments before the first argument (e.g. `new Foo(/* c */ a, // t)`).
                // The inter-argument loop below only emits leading comments for args 1..n
                // (via the previous arg's gap), so the first arg's leading comment must be
                // emitted here or it's dropped.
                if i == 0 {
                    emit_first_arg_leading_comments(
                        self,
                        &mut arg_parts,
                        paren_open,
                        arg.span().start,
                    );
                }

                // Build the argument with the argument-context builder so a binary/
                // logical chain (or conditional) keeps its continuation indent in this
                // trailing-line-comment path — matching the no-comment path (the
                // call/member-chain comment paths do the same).
                arg_parts.push(self.build_arg_expression_doc(arg));

                // Check for comments after this argument
                if i < new_expr.arguments.len() - 1 {
                    let arg_end = arg.span().end;
                    let next_arg_start = new_expr.arguments[i + 1].span().start;

                    let pc = self.open_inter_arg_gap(&mut arg_parts, arg_end, next_arg_start);
                    arg_parts.push(d.hardline());
                    // hugging after-comma + own-line comments lead the next arg (`C`).
                    pc.emit_leading_comments_inline_aware(&mut arg_parts, self);
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

                    // Same-line trailing comments split around the source comma position
                    // (after-comma blocks stay past it, `b /* c */` — the tsv divergence —
                    // and a line comment follows via `line_suffix`), then own-line dangling
                    // comments. No trailing comma (trailingComma: 'none'). Matches the
                    // call/member-chain last-arg paths.
                    pc.emit_last_arg_comments(&mut arg_parts, self);
                }
            }

            let arg_doc = d.concat(&arg_parts);

            return d.concat(&[
                callee_with_types,
                d.text("("),
                d.indent(d.concat(&[d.hardline(), arg_doc])),
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
            let mut arg_docs: DocBuf = new_expr
                .arguments
                .iter()
                .map(|arg| self.build_arg_expression_doc(arg))
                .collect();

            // Prepend leading comments before the first arg (e.g. `new Foo(/* c */ a /* t */)`);
            // this path otherwise emits only trailing comments, dropping the leading one.
            if let Some(first_arg) = new_expr.arguments.first() {
                let mut lead = DocBuf::new();
                emit_first_arg_leading_comments(
                    self,
                    &mut lead,
                    paren_open,
                    first_arg.span().start,
                );
                if !lead.is_empty() {
                    lead.push(arg_docs[0]);
                    arg_docs[0] = d.concat(&lead);
                }
            }

            // Add trailing block comment to last arg. For spread elements, scan
            // inside the spread span for comments from stripped parens.
            let last_arg = &new_expr.arguments[last_idx];
            let effective_arg_end = self.last_arg_comment_scan_start(last_arg);

            let pc = PartitionedComments::new(
                self.comments,
                self.line_breaks,
                effective_arg_end,
                new_expr.span.end,
            );

            // Own-line block comments after the last arg (before closing paren).
            // These appear as siblings after the last arg (no trailing comma), forcing expansion.
            let leading_block: CommentVec<'_> =
                pc.leading.iter().filter(|c| c.is_block).copied().collect();
            if !leading_block.is_empty()
                && let Some(last_doc) = arg_docs.pop()
            {
                let mut last_parts = DocBuf::new();
                last_parts.push(last_doc);
                let mut prev_end = effective_arg_end;
                for comment in &leading_block {
                    // Preserve an author blank line before the own-line trailing comment
                    // (`b⏎⏎/* c */` before `)`), matching prettier and the call path.
                    self.push_blank_preserving_hardline(
                        &mut last_parts,
                        prev_end,
                        comment.span.start,
                    );
                    last_parts.push(self.build_comment_doc(comment));
                    prev_end = comment.span.end;
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
                // Split same-line blocks around the last arg's source comma: before-comma
                // blocks (and any block when the source has no comma) hug the arg; an
                // after-comma block is preserved past where the comma was (`b /* c */`; no
                // trailing comma, trailingComma: 'none').
                // No line comments reach this block-only path.
                let comma_pos = find_comma_pos(self.source, effective_arg_end, new_expr.span.end);
                let mut last_with_comment: DocBuf = smallvec![last_doc];
                let mut after_comma = DocBuf::new();
                for comment in &pc.trailing_block {
                    if comma_pos.is_some_and(|cp| is_comment_after_comma(comment, cp)) {
                        after_comma.push(d.text(" "));
                        after_comma.push(self.build_comment_doc(comment));
                    } else {
                        last_with_comment.push(d.text(" "));
                        last_with_comment.push(self.build_comment_doc(comment));
                    }
                }
                arg_docs.push(d.concat(&last_with_comment));

                // The after-comma block (if any) is kept past where the comma was via the
                // wrap's `post_comma` suffix: `b /* c */` (no trailing comma in either
                // mode; trailingComma: 'none').
                let post_comma = d.concat(&after_comma);

                // For function composition (multiple callbacks), use hardlines
                // For simple args, use soft breaks (can stay inline)
                if is_function_composition_args(new_expr.arguments) {
                    let arg_parts = d.join_doc(arg_docs, d.comma_hardline());
                    return wrap_call_with_hard_breaks_suffix(
                        d,
                        callee_with_types,
                        arg_parts,
                        post_comma,
                    );
                }
                let arg_parts = d.join_doc(arg_docs, d.comma_line());
                return wrap_call_with_soft_breaks_suffix(
                    d,
                    callee_with_types,
                    arg_parts,
                    post_comma,
                );
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
                && preceding_args_allow_expand_last(new_expr.arguments, self.line_breaks)
                && !has_inter_argument_comments_slice(new_expr.arguments, self)
            {
                let (head_parts, last_arg_doc, all_args_broken) = build_args_split_last(
                    new_expr.arguments,
                    self,
                    paren_open,
                    self.has_comments_between(paren_open, new_expr.span.end),
                );

                // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
                if head_parts.iter().any(|&id| d.will_break(id)) {
                    return build_expand_all_args(d, callee_with_types, all_args_broken);
                }

                if last_is_function {
                    // Function/arrow last arg path (matches call_formatting.rs's expand-last function path)
                    // Expression arrows with call/conditional body get break-body state
                    if let Some(internal::Expression::ArrowFunctionExpression(arrow)) =
                        new_expr.arguments.last()
                        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
                        && (arrow_body_is_call_through_non_null(body_expr)
                            || matches!(
                                &**body_expr,
                                internal::Expression::ConditionalExpression(_)
                            ))
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
                            build_inline_args(d, callee_with_types, &head_parts, last_arg_doc);

                        return d.conditional_group(&[
                            state_inline,
                            state_break_body,
                            state_expand_all,
                        ]);
                    }

                    // Block-body arrow/function: inline vs expand-all (no hug state)
                    let state_inline =
                        build_inline_args(d, callee_with_types, &head_parts, last_arg_doc);
                    let state_expand_all =
                        build_expand_all_args(d, callee_with_types, all_args_broken);
                    return d.conditional_group(&[state_inline, state_expand_all]);
                }

                // Array/object last arg path (matches call_formatting.rs's expand-last array/object path)
                // Same outer type: skip hug, use expand-all
                if last_two_args_same_type(new_expr.arguments) {
                    // Same type: Prettier uses expand-all when last arg will break
                    if d.will_break(last_arg_doc) {
                        return build_expand_all_args(d, callee_with_types, all_args_broken);
                    }
                    return build_inline_or_expand_all(
                        d,
                        callee_with_types,
                        &head_parts,
                        last_arg_doc,
                        all_args_broken,
                    );
                }

                // Different types: if last arg has forced breaks, use inline-or-expand-all
                if d.has_forced_break(last_arg_doc) {
                    return build_inline_or_expand_all(
                        d,
                        callee_with_types,
                        &head_parts,
                        last_arg_doc,
                        all_args_broken,
                    );
                }

                // No forced breaks: 3-state (inline → hug → expand all)
                let state_inline =
                    build_inline_args(d, callee_with_types, &head_parts, last_arg_doc);
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
        let has_inter_arg_comments = has_inter_argument_comments_slice(new_expr.arguments, self);

        // Comments trailing the `(` on the same line stay on the `(` line, with
        // own-line comments on their own lines before the first arg — preserving
        // the author's placement and source order (divergence from prettier,
        // which floats a line comment past the statement and relocates a block
        // before `(`). Also fixes content loss: a line comment trailing `(` was
        // previously dropped. See conformance_prettier.md §Comment relocation.
        if has_leading_comments {
            let first_arg_start = new_expr.arguments[0].span().start;
            let gap_pc = PartitionedComments::new(
                self.comments,
                self.line_breaks,
                paren_open,
                first_arg_start,
            );
            let has_paren_line =
                !gap_pc.trailing_block.is_empty() || !gap_pc.trailing_line.is_empty();
            if has_paren_line
                && should_force_expansion_for_comments(self, paren_open, first_arg_start)
            {
                let mut paren_line_prefix = DocBuf::new();
                gap_pc.emit_trailing_comments(&mut paren_line_prefix, self);

                let mut inner = DocBuf::new();
                for comment in &gap_pc.leading {
                    inner.push(self.build_comment_doc(comment));
                    inner.push(d.hardline());
                }
                // Build the args without re-emitting the first-arg leading gap
                // (pass first_arg_start so the gap scan finds nothing).
                inner.push(build_args_joined_with_comments(
                    self,
                    new_expr.arguments,
                    first_arg_start,
                    true,
                    #[allow(clippy::redundant_closure_for_method_calls)]
                    |p, a| p.build_arg_expression_doc(a),
                ));

                return d.concat(&[
                    callee_with_types,
                    d.text("("),
                    d.concat(&paren_line_prefix),
                    d.indent(d.concat(&[d.hardline(), d.concat(&inner)])),
                    d.hardline(),
                    d.text(")"),
                ]);
            }
        }

        if has_leading_comments || has_inter_arg_comments {
            let arg_parts = build_args_joined_with_comments(
                self,
                new_expr.arguments,
                paren_open,
                false,
                #[allow(clippy::redundant_closure_for_method_calls)]
                |p, a| p.build_arg_expression_doc(a),
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
    pub(in crate::printer) fn build_new_doc(
        &self,
        new_expr: &internal::NewExpression<'_>,
    ) -> DocId {
        self.build_new_doc_with_wrapping(new_expr)
    }
}
