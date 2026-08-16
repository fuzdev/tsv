// New expression printing for TypeScript
//
// Handles: new Foo(), new Foo(arg1, arg2), new Foo<T>()

use super::arg_comments::{
    any_arg_empty_line, any_comment_forces_expansion_slice, first_arg_has_any_comments,
    last_arg_has_comments,
};
use super::arg_wrapping::{
    append_type_args_with_gap_comments, build_call_args_with_blank_lines, build_empty_args_doc,
    build_expand_first_arg_doc, build_own_line_post_arrow_state,
    first_arg_signature_refuses_expand_first, last_arg_has_own_line_post_arrow_comment,
    should_expand_first_arg, try_hook_deps_args_doc, try_hug_multiline_template_arg,
    wrap_call_with_soft_breaks,
};
use crate::ast::internal;
use crate::printer::calls::arg_predicates::{
    arrow_body_is_call_through_non_null, is_array_or_object_unwrapped, is_concise_numeric_array,
    is_function_composition_args, is_ternary_arrow_body,
};
use crate::printer::calls::{
    ArgItem, ArgsJoin, PartitionedComments, arrow_hug_refused_by_comments,
    build_args_joined_with_comments, build_args_split_last, build_arrow_call_body_states,
    build_arrow_sig_doc, build_break_body_state, build_call_args_expanded, build_expand_all_args,
    build_inline_args, build_inline_hug_or_expand_all, build_inline_or_expand_all,
    build_printed_argument_doc, could_expand_arrow_chain, has_inter_argument_comments_slice,
    has_trailing_comments_slice, has_trailing_line_comments_slice, last_two_args_same_type,
    prebuild_expand_last_break_body, prepend_arrow_body_comments,
    should_force_expansion_for_comments, wrap_call_with_hard_breaks_paren_line,
    wrap_call_with_will_break_guard,
};
use crate::printer::expressions::functions::arrow_signature_has_breaking_comments;
use crate::printer::{
    ParenContext, Printer, arrow_chain_should_break, container_may_have_multiline_content,
    has_multiline_content, is_curried_arrow_chain,
};
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
        // The keyword→operand gap, shared with `await` — see
        // `prepend_keyword_operand_comments`.
        let callee = self.prepend_keyword_operand_comments(
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

        // Position just before `(` (after the callee and any type arguments); in
        // the empty-args case it doubles as the dangling-comment boundary.
        let paren_open = new_expr
            .type_arguments
            .as_ref()
            .map_or_else(|| new_expr.callee.span().end, |ta| ta.span.end);

        // Empty args: just `new Foo()` or `new Foo<K, V>()`, preserving dangling comments
        if new_expr.arguments.is_empty() {
            return build_empty_args_doc(
                self,
                d.concat(&[d.text("new "), callee_with_types_base]),
                paren_open,
                new_expr.span.end,
                false,
            );
        }

        // Build callee with type args: `new Foo<K, V>`
        let callee_with_types = d.concat(&[d.text("new "), callee_with_types_base]);
        // Zero-comment fast gate: one binary search over the whole argument window
        // (`(` … `)`) short-circuits every per-branch argument-comment predicate
        // below (trailing-line / trailing-block / inter-argument / leading), each of
        // which scans a sub-range within it. The callee and type-argument gap
        // comments live before `paren_open` and are handled unconditionally above.
        // Canonical reference: build_params_doc_with_comments.
        //
        // **on page**: this is the master gate for the whole `new` cascade — including
        // the layout predicates (`first_arg_has_any_comments`, the huggable-argument
        // refusal). Skipping an owned annotation here would short-circuit them all and
        // silently hug an argument prettier expands. Its analogs (`call_has_comments`)
        // in `calls/mod.rs`, `call_formatting.rs` and `chain_args.rs` count too.
        let new_has_comments = self.has_comments_on_page_between(paren_open, new_expr.span.end);

        // Prettier's React-hook deps-array layout — the FIRST thing `printCallArguments`
        // asks, and `new` shares that printer, so a `new` written in the shape takes it too.
        // Above `anyArgEmptyLine`, so an author blank between the callback and the deps
        // array collapses here rather than forcing the arguments out.
        if let Some(doc) = try_hook_deps_args_doc(
            self,
            new_expr.arguments,
            paren_open,
            new_expr.span.end,
            new_has_comments,
            d.concat(&[callee_with_types, d.text("(")]),
        ) {
            return doc;
        }

        // Prettier's `anyArgEmptyLine` — `new` shares `printCallArguments` with a plain call,
        // so the rule and its POSITION are the same: an author blank in any inter-argument gap
        // forces `allArgsBrokenOut()`, ABOVE every specialized layout. See the twin in
        // `call_formatting.rs` for the full argument; the single-argument hug arms between here
        // and the first use are vacuously safe (a blank needs two arguments to sit between).
        let any_arg_empty_line = any_arg_empty_line(new_expr.arguments, self);

        // Single huggable argument: object literal or function
        // These stay on the same line as the opening paren: `new Cls({...})` not `new Cls(\n{...})`
        // Skip hugging if there are trailing comments (line OR block) - let the comment handling below handle it
        let single_arg_has_trailing_comment = new_expr.arguments.len() == 1
            && new_has_comments
            && has_trailing_comments_slice(new_expr.arguments, new_expr.span.end, self);

        // A comment in the `(`→argument gap that this expression would have to EMIT
        // disqualifies the whole single-argument block below: none of its arms has a
        // gap emitter, so the comment would be DROPPED (the hazard-4 shape in
        // docs/comments.md). Such a call falls through to the comment-aware paths
        // and expands, matching prettier.
        //
        // The three plain hug arms (object / array / function expression) decline on
        // the wider ON-PAGE question inside the match, like the arrow arm: an OWNED
        // glued comment rides inside the argument's doc — the hug does print it —
        // but it still defeats the hug, exactly as prettier's `couldExpandArg`
        // refuses to hug an argument whose leading comment sits before it
        // (prettier shares one `printCallArguments` for Call and New, and the plain
        // call already expands `fn(/* c */ { breaking })`). A to-emit gate went
        // blind to it and kept the hug, which collapsed prettier's expanded form
        // back to `new A(/* c */ {` on every pass. The declined argument falls to
        // the default soft-wrapped join at the bottom: flat when it fits — the
        // same bytes the hug rendered — expanded when the argument breaks.
        let single_arg_leading_emit_comment = new_expr.arguments.len() == 1
            && new_has_comments
            && self.has_comments_to_emit_between(paren_open, new_expr.arguments[0].span().start);
        let single_arg_leading_on_page_comment = new_expr.arguments.len() == 1
            && new_has_comments
            && self.has_comments_on_page_between(paren_open, new_expr.arguments[0].span().start);

        if new_expr.arguments.len() == 1
            && !single_arg_has_trailing_comment
            && !single_arg_leading_emit_comment
        {
            match &new_expr.arguments[0] {
                // Object / array / function-expression argument: hug it — the
                // argument's own doc expands internally. An on-page leading
                // comment declines the hug (see the guard's comment above);
                // prettier expands all three the same way.
                internal::Expression::ObjectExpression(_)
                | internal::Expression::ArrayExpression(_)
                | internal::Expression::FunctionExpression(_)
                    if !single_arg_leading_on_page_comment =>
                {
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
                    // Which of prettier's two renderings this ONE doc is — see
                    // `call_formatting.rs`'s `build_block_arrow_hug_states`, which states the
                    // rule (a chain with no forced break → the progressive layout, whose flat
                    // rendering the hug state measures identically; one that
                    // `arrow_chain_should_break` forces → `skip_arrow_chain`, whose
                    // nested-arrow-break suppression is what the expand-last hug wants). One
                    // doc, deliberately: a second build recurses into a nested call, and
                    // paying it here would make the doc-node count 2^depth.
                    let arg0 = &new_expr.arguments[0];
                    let mut arrow_doc = if is_curried_arrow_chain(arg0)
                        && arrow_chain_should_break(arrow)
                    {
                        self.skip_arrow_chain.set(true);
                        let doc = self.build_expression_doc(arg0);
                        self.skip_arrow_chain.set(false);
                        doc
                    } else {
                        build_printed_argument_doc(self, arg0, || self.build_expression_doc(arg0))
                    };

                    // Prepend leading comments (e.g., /** @param {any} x */ before arrow)
                    // and force wrapped state when present (prettier expands args with leading comments)
                    let arg_start = new_expr.arguments[0].span().start;
                    // Glued like the regular-call leading-arg paths (prettier shares
                    // one `printCallArguments` for Call and New): a single-line block
                    // hugged to `(` stays with the argument across a source newline.
                    let glued = if new_has_comments {
                        self.build_rhs_comments_glued_opt(paren_open, arg_start)
                    } else {
                        None
                    };
                    if let Some(leading) = glued {
                        arrow_doc = d.concat(&[leading, arrow_doc]);
                    }
                    // **on page**: a leading comment forces the wrapped (expanded) state,
                    // owned or not — an owned comment rides inside `arrow_doc` (so it's
                    // not in `glued`) but still defeats the hug, exactly as prettier
                    // expands a block-arrow arg whose leading comment precedes it. A
                    // to-emit gate here would go blind to it and wrongly hug.
                    let has_leading_comment = new_has_comments
                        && self.has_comments_on_page_between(paren_open, arg_start);

                    // If the arrow has trailing param comments or leading comments,
                    // force wrapped state
                    let has_trailing_param_comments =
                        new_has_comments && arrow_signature_has_breaking_comments(self, arrow);

                    if has_trailing_param_comments || has_leading_comment {
                        return d.concat(&[
                            callee_with_types,
                            d.text("("),
                            d.indent(d.concat(&[d.softline(), arrow_doc])),
                            d.softline(),
                            d.text(")"),
                        ]);
                    }

                    // An own-line comment between `=>` and the body forces the closing paren
                    // onto its own line — the rule is the gap's and not the body's, see
                    // [`last_arg_has_own_line_post_arrow_comment`].
                    if new_has_comments && last_arg_has_own_line_post_arrow_comment(self, arg0) {
                        return d.concat(&[
                            callee_with_types,
                            build_own_line_post_arrow_state(d, d.text("("), &[], arrow_doc),
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
                        // The reassembling arm's refusal pair — a break forced inside the
                        // signature, or a comment on the body's own tail, either of which
                        // these states cannot honor (`arrow_hug_refused_by_comments`).
                        if is_ternary_arrow_body(body_expr)
                            && !(new_has_comments
                                && arrow_hug_refused_by_comments(self, arrow, body_expr))
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
                                d.indent_hardline(body_doc),
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
                                    d.indent_hardline(body_doc),
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
                        if arrow_body_is_call_through_non_null(body_expr)
                            // Same refusal pair as the ternary arm above.
                            && !(new_has_comments
                                && arrow_hug_refused_by_comments(self, arrow, body_expr))
                        {
                            // Build the body ONCE (see `build_arrow_call_body_states`) — a
                            // separate whole-arrow doc re-built this body and recursed → O(2^depth).
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

        // Prettier's POSITION for `anyArgEmptyLine` — above every specialized layout, the
        // plain call's twin (see `call_formatting.rs` for the full argument). ONE site rather
        // than a conjunct per arm; the arms above are all inside the single-argument block, so
        // the question is vacuous there and lifting the gate over them changes nothing.
        //
        // Unlike the plain call, no comment path preempts this builder, so both edge gaps are
        // live here: it puts a `(`-line run on the `(` line and the last argument's trailing
        // comments after that argument.
        if any_arg_empty_line {
            return build_call_args_with_blank_lines(
                self,
                callee_with_types,
                new_expr.arguments,
                paren_open,
                new_expr.span.end,
            );
        }

        // Function composition pattern: when any argument is a call containing a callback
        // OR when there are multiple function arguments
        // e.g., new Cls(arr.map((x) => x), b) → new Cls(\n\t...,\n)
        // e.g., new Cls(() => a, () => b) → new Cls(\n\t...,\n)
        // Skip this path if there are trailing comments - let the comment handling paths handle it
        if is_function_composition_args(new_expr.arguments)
            && !(new_has_comments
                && has_trailing_comments_slice(new_expr.arguments, new_expr.span.end, self))
        {
            return build_call_args_expanded(
                self,
                callee_with_types,
                new_expr.arguments,
                paren_open,
                new_expr.span.end,
                ArgItem::ArgContext,
            );
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
        let has_multiline = container_may_have_multiline_content(new_expr.span, self.source)
            && new_expr
                .arguments
                .iter()
                .any(|arg| has_multiline_content(arg, self.source));

        if has_multiline {
            // Force expansion with hardlines for multiline content
            return build_call_args_expanded(
                self,
                callee_with_types,
                new_expr.arguments,
                paren_open,
                new_expr.span.end,
                ArgItem::ArgContext,
            );
        }

        // "Expand first arg" pattern: callback first, short/empty container last
        // e.g., new Proxy((x) => { ... }, {}) - callback hugs, empty obj stays inline.
        // Block for comments the inline tail can't carry (matching the plain-call path):
        // a line comment anywhere in the args, or any comment on the first arg — those
        // break all args instead (a before-comma trailing block, a leading first-arg
        // comment). An after-comma inline block leading the second arg is carried below.
        // Named once, like the plain call's twin — and factored on `new_has_comments`, the
        // cascade's zero-comment fast gate, so a comment-free `new` asks neither predicate.
        let expand_first_blocked = new_has_comments
            && (has_trailing_line_comments_slice(new_expr.arguments, new_expr.span.end, self)
                || first_arg_has_any_comments(new_expr.arguments, self, paren_open)
                || first_arg_signature_refuses_expand_first(self, new_expr.arguments));
        // One `printCallArguments` prints both spellings, so the layout is the plain call's
        // (`build_expand_first_arg_doc`), not a copy of it.
        if should_expand_first_arg(self, new_expr.arguments) && !expand_first_blocked {
            return build_expand_first_arg_doc(
                self,
                callee_with_types,
                new_expr.arguments,
                paren_open,
                new_expr.span.end,
            );
        }

        // Check for trailing LINE comments on arguments (forces hardline expansion)
        // Must check this BEFORE the "last arg is array/object" pattern below,
        // otherwise trailing comments on the last arg cause it to be hugged incorrectly.
        // e.g., new Class(arg1, // comment\n  arg2)
        if new_has_comments
            && has_trailing_line_comments_slice(new_expr.arguments, new_expr.span.end, self)
        {
            // The shared joined-argument builder owns every gap in the list — the
            // `(`→first-argument run (into `paren_line`), each inter-argument gap,
            // and the last argument's trailing region. Hardline-joined throughout,
            // so each gap's `forces_expansion` obligation is already met. The
            // hand-rolled loop this replaced re-spelled the builder's Hardline join
            // exactly (its unguarded `open_inter_arg_gap` on a comment-free gap
            // emits just the comma the builder's guarded arm bakes into
            // `comma_hardline`), and a copied loop is where the last-arg and
            // inter-arg emitters have drifted before.
            let mut paren_line = DocBuf::new();
            let arg_doc = build_args_joined_with_comments(
                self,
                new_expr.arguments,
                paren_open,
                new_expr.span.end,
                ArgsJoin::Hardline,
                ArgItem::ArgContext,
                &mut paren_line,
            );
            return wrap_call_with_hard_breaks_paren_line(
                d,
                callee_with_types,
                &paren_line,
                arg_doc,
            );
        }

        // The last argument trails comments, and no LINE comment sits in the gap after it
        // (one there ends the `)` line, which the expand-last / hug paths below cannot
        // express). A block stays inline for simple args (`new A(a, b /* comment */)`);
        // function composition expands (`new A(() => {}, () => {} /* comment */,)`).
        //
        // An argument whose stripped grouping parens hid a comment
        // (`new A(a, ...(b⏎/* c */))`) routes here too: it lies *before* the argument's
        // own end, so the `[arg_end, )` scan cannot see it and every collapsing path
        // below would drop it. That interior may hold a `//` — hence "no line comment in
        // the GAP" rather than "block-only": the spread's own doc defers its line
        // comments through `line_suffix`, and `hard` below forces the break they need.
        // `new_has_comments` (on page over `[paren_open, span.end)`, a superset of every
        // interior) gates the per-argument scan off the comment-free path, like the
        // call/chain entry gates.
        let spread_paren_comments_expand =
            new_has_comments && self.any_spread_paren_comment_forces_expansion(new_expr.arguments);
        let has_trailing_comments_no_gap_line = new_has_comments
            && new_expr.arguments.last().is_some_and(|last_arg| {
                let arg_end = last_arg.span().end;
                let paren_close = new_expr.span.end;
                (spread_paren_comments_expand
                    || self.has_comments_to_emit_between(arg_end, paren_close))
                    && !self.has_line_comments_between(arg_end, paren_close)
            });

        if has_trailing_comments_no_gap_line {
            // The shared joined-argument builder owns every gap in the list — the
            // `(`→first-argument run (into `paren_line`), each inter-argument gap, and
            // the last argument's trailing region (its spread interior included). The
            // hand-rolled loop this replaced built the argument docs and joined them with
            // commas, so it had no inter-argument emitter at all: a comment between two
            // arguments was DROPPED whenever the argument itself didn't own it (the
            // hazard-4 shape in docs/comments.md).
            let last_arg = &new_expr.arguments[new_expr.arguments.len() - 1];
            let arg_end = last_arg.span().end;
            let paren_close = new_expr.span.end;

            // An own-line comment — from the spread interior or from the `[arg_end, )`
            // gap — is a sibling of the last argument rather than a trailer on its line,
            // so the argument list must hard-break around it. So must a `(`-line run,
            // which ends in a `//` nothing may share a line with; asked here rather than
            // read off `paren_line` afterwards, since the join is chosen before the build.
            let paren_line_run = PartitionedComments::new(
                self.comments,
                self.comment_line_breaks,
                paren_open,
                new_expr.arguments[0].span().start,
            )
            .has_trailing_line();
            // The last-argument arm reads own-line-ness from the SOURCE
            // ([`Printer::has_own_line_block_comment_before_closer`]), the same predicate
            // the call and parameter lists ask of this position: the gap holds the list's
            // own comma, which `trailingComma: 'none'` deletes, so an `is_same_line(arg_end,
            // …)` reading called a comment glued to it own-line and hard-broke a `new` that
            // fits (`docs/comments.md` §Own-line-ness is a SOURCE question).
            let hard = paren_line_run
                || spread_paren_comments_expand
                || is_function_composition_args(new_expr.arguments)
                || self.has_own_line_block_comment_before_closer(arg_end, paren_close);

            let mut paren_line = DocBuf::new();
            let arg_parts = build_args_joined_with_comments(
                self,
                new_expr.arguments,
                paren_open,
                paren_close,
                if hard {
                    ArgsJoin::Hardline
                } else {
                    ArgsJoin::SoftLine
                },
                ArgItem::ArgContext,
                &mut paren_line,
            );
            if hard {
                return wrap_call_with_hard_breaks_paren_line(
                    d,
                    callee_with_types,
                    &paren_line,
                    arg_parts,
                );
            }
            return wrap_call_with_soft_breaks(d, callee_with_types, arg_parts);
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
            // Prettier's `couldExpandArg` for the last argument — the classification the
            // plain-call twin closes with (`try_expand_last_function_arg`'s
            // `is_non_expandable_expr_arrow`) and this printer never had. Spelled as
            // **reachability over this printer's own arms** rather than as a second body-kind
            // list, so the two cannot drift: an arrow argument is hug-eligible when
            // `could_expand_arrow_chain` claims it (a block body, an object/array body, or an
            // arrow chain ending in one — all of which the `build_inline_or_expand_all` tail
            // hugs whole), or when the break-body arm claims it (a call through a trailing `!`,
            // or a ternary) and that arm's own refusal stays silent — the SAME
            // `arrow_hug_refused_by_comments` the arm guards itself with, so the gate and the
            // arm cannot drift apart. Anything else — a template, a binary, a member, a `new`
            // body — no state below can hug, and the general path prints it whole.
            //
            // ⚠️ The gap was invisible without a FORCED break, because the general path and the
            // ladder below agree whenever `fits()` can see the whole argument. A break inside
            // such a body — a multiline template, a source-multiline object under a binary
            // operator, the body-tail comment the break-body arm declines for — truncates that
            // walk (tsv has no `propagateBreaks`, so a `conditional_group` measures a state flat
            // to its first hardline), so `state_inline` reported itself as fitting and `new`
            // hugged where every sibling printer expands. Prettier expands all three shapes at
            // its fixed point, `new` included.
            //
            // Asked HERE rather than where the twin asks it — after the argument docs are built —
            // because reaching it and then declining would build every argument TWICE, once here
            // and once on the general path, which recurses into any call nested in the body:
            // `new A(lead, p => new B(lead, q => …))` went 2^depth doc nodes (`fanout:audit`'s
            // `ts_nested_arrow_multiarg_new`). Every predicate it asks is a pure AST/comment
            // question, so none of them needs a doc.
            let last_arg_hug_eligible = match last_arg {
                Some(internal::Expression::ArrowFunctionExpression(arrow)) => {
                    could_expand_arrow_chain(arrow)
                        || match &arrow.body {
                            internal::ArrowFunctionBody::Expression(body_expr) => {
                                (arrow_body_is_call_through_non_null(body_expr)
                                    || is_ternary_arrow_body(body_expr))
                                    && !(new_has_comments
                                        && arrow_hug_refused_by_comments(self, arrow, body_expr))
                            }
                            internal::ArrowFunctionBody::BlockStatement(_) => false,
                        }
                }
                _ => true,
            };
            // …with one exception, which is why the twin can ask this late: an own-line comment
            // after `=>` selects its own state ([`last_arg_has_own_line_post_arrow_comment`])
            // ahead of any body-kind question, for a non-eligible body too. That state needs
            // the argument docs, so it must keep the block — and it returns from inside it, so no
            // second build follows.
            let last_arg_takes_own_line_post_arrow_state = new_has_comments
                && last_arg
                    .is_some_and(|last| last_arg_has_own_line_post_arrow_comment(self, last));

            if new_expr.arguments.len() >= 2
                && (last_is_function || last_is_expandable_collection)
                && (last_arg_hug_eligible || last_arg_takes_own_line_post_arrow_state)
                // The same question both twins ask (`any_comment_forces_expansion`), over a
                // slice: a comment that needs a line of its own defeats the hug, an inline
                // block glued between an argument and its comma does not. Asking "is there
                // ANY inter-argument comment" instead broke out `new A(a /* c */, () => …)`,
                // a list prettier keeps hugged — and asked nothing about the `(`→first-argument
                // gap or a spread's stripped-paren interior, which the shared predicate covers.
                && !(new_has_comments
                    && any_comment_forces_expansion_slice(
                        new_expr.arguments,
                        self,
                        paren_open,
                        new_expr.span.end,
                    ))
                // On-page: a leading comment on the last argument defeats the expand-last
                // hug (prettier's `shouldExpandLastArg`), owned or not — an owned comment
                // rides inside the argument's doc, so this must count it (on page), not just
                // the emit-keyed ones, or it hugs blind. Mirrors the call/chain paths.
                && !(new_has_comments
                    && last_arg_has_comments(new_expr.arguments, self, new_expr.span.end, paren_open))
            {
                // Expand-last arrow with a call body: build the body ONCE and inject it so
                // the whole-arrow arg doc reuses it (the break-body state below reuses it
                // too) — building it in both places recurses into itself → O(2^depth).
                let body_reuse = prebuild_expand_last_break_body(
                    self,
                    new_expr.arguments.last(),
                    new_has_comments,
                );
                let inject_prev = body_reuse.map(|(span, doc)| self.inject_arrow_body(span, doc));

                let (head_parts, last_arg_doc, all_args_broken) =
                    build_args_split_last(new_expr.arguments, self, paren_open, new_has_comments);

                if let Some(prev) = inject_prev {
                    self.restore_arrow_body_inject(prev);
                }

                // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
                if head_parts.iter().any(|&id| d.will_break(id)) {
                    return build_expand_all_args(d, callee_with_types, all_args_broken);
                }

                // The multi-argument twin of the single-argument arm above: an own-line
                // comment after `=>` drops the closing paren to its own line, asked above
                // every body-kind arm for the reason
                // [`last_arg_has_own_line_post_arrow_comment`] gives.
                if new_has_comments
                    && let Some(last) = new_expr.arguments.last()
                    && last_arg_has_own_line_post_arrow_comment(self, last)
                {
                    return d.concat(&[
                        callee_with_types,
                        build_own_line_post_arrow_state(d, d.text("("), &head_parts, last_arg_doc),
                    ]);
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
                        // The multi-argument twin of the single-argument refusal above, and the
                        // same question `last_arg_hug_eligible` already answered to let this
                        // block be entered at all (`arrow_hug_refused_by_comments`).
                        && !(new_has_comments
                            && arrow_hug_refused_by_comments(self, arrow, body_expr))
                    {
                        let sig_doc = build_arrow_sig_doc(self, arrow);
                        // Reuse the pre-built call body (see above); conditional bodies build fresh.
                        let body_doc = body_reuse
                            .map_or_else(|| self.build_expression_doc(body_expr), |(_, doc)| doc);
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

                    // Block-body arrow/function: inline vs expand-all (no hug state).
                    //
                    // A break forced inside the last argument's signature invalidates
                    // `state_inline`, as it does at every other hug state — but the question is
                    // asked of an **arrow only**, deliberately. Prettier hugs a `function`
                    // argument under `new` uniformly, so there is nothing incoherent to correct
                    // there and tsv matches it; see
                    // `functions::function_signature_has_breaking_comments`.
                    if let Some(internal::Expression::ArrowFunctionExpression(arrow)) =
                        new_expr.arguments.last()
                        && arrow_signature_has_breaking_comments(self, arrow)
                    {
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

                // Different types: the 3-state ladder (inline → hug → expand all), the same
                // helper the plain-call twin uses. Prettier's printCallArguments is shared
                // by `new` (its header comment lists NewExpression), and for a breaking
                // last arg it keeps the hug: `[breakParent, conditionalGroup([hug,
                // allArgsBrokenOut])]` — there is no forced-break → inline-or-expand-all
                // form.
                return build_inline_hug_or_expand_all(
                    d,
                    callee_with_types,
                    &head_parts,
                    last_arg_doc,
                    all_args_broken,
                );
            }
        }

        // Check for leading comments or inter-argument block comments
        // These need explicit handling that the simple join_doc path doesn't provide
        let has_leading_comments = new_has_comments
            && !new_expr.arguments.is_empty()
            && self.has_comments_to_emit_between(paren_open, new_expr.arguments[0].span().start);
        let has_inter_arg_comments =
            new_has_comments && has_inter_argument_comments_slice(new_expr.arguments, self);

        // Comments trailing the `(` on the same line stay on the `(` line, with
        // own-line comments on their own lines before the first arg — preserving
        // the author's placement and source order (divergence from prettier,
        // which floats a line comment past the statement and relocates a block
        // before `(`). Also fixes content loss: a line comment trailing `(` was
        // previously dropped. See conformance_prettier_ts_comments.md §Comment relocation.
        if has_leading_comments {
            let first_arg_start = new_expr.arguments[0].span().start;
            let gap_pc = PartitionedComments::new(
                self.comments,
                self.comment_line_breaks,
                paren_open,
                first_arg_start,
            );
            let has_paren_line = gap_pc.has_trailing_comments();
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
                // The `(`→first-argument gap is emitted above (the paren-line prefix and
                // the leading run), so the builder must not print it a second time — hence
                // its `paren_line` out-param stays unused here and must come back empty.
                let mut unused_paren_line = DocBuf::new();
                inner.push(build_args_joined_with_comments(
                    self,
                    new_expr.arguments,
                    paren_open,
                    new_expr.span.end,
                    ArgsJoin::HardlineLeadingGapEmitted,
                    ArgItem::ArgContext,
                    &mut unused_paren_line,
                ));
                debug_assert!(unused_paren_line.is_empty());

                return d.concat(&[
                    callee_with_types,
                    d.text("("),
                    d.concat(&paren_line_prefix),
                    d.indent_hardline(d.concat(&inner)),
                    d.hardline(),
                    d.text(")"),
                ]);
            }
        }

        if has_leading_comments || has_inter_arg_comments {
            let mut paren_line = DocBuf::new();
            let arg_parts = build_args_joined_with_comments(
                self,
                new_expr.arguments,
                paren_open,
                new_expr.span.end,
                ArgsJoin::SoftLine,
                ArgItem::ArgContext,
                &mut paren_line,
            );
            // Both trailing-comment arms above return before this one, so the last
            // argument's gap is empty here and the soft wrap can still collapse. A
            // `(`-line run can't: its `//` ends the line.
            if !paren_line.is_empty() {
                return wrap_call_with_hard_breaks_paren_line(
                    d,
                    callee_with_types,
                    &paren_line,
                    arg_parts,
                );
            }
            return wrap_call_with_will_break_guard(d, callee_with_types, arg_parts);
        }

        // Build args with line separators (one per line when broken). Prettier shares one
        // `printCallArguments` for Call and New, so this is its `printedArguments` — printed
        // with no `expandLastArg` (`build_printed_argument_doc`), exactly as the plain call's
        // twin does.
        let arg_parts = d.join_doc(
            new_expr.arguments.iter().map(|arg| {
                build_printed_argument_doc(self, arg, || self.build_arg_expression_doc(arg))
            }),
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
