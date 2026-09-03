// New expression printing for TypeScript
//
// Handles: new Foo(), new Foo(arg1, arg2), new Foo<T>()

use super::arg_comments::{
    PartitionedComments, any_arg_empty_line, first_arg_has_any_comments,
    has_inter_argument_comments_slice, has_trailing_comments_slice,
    has_trailing_line_comments_slice,
};
use super::arg_predicates::{
    arrow_body_is_call_through_non_null, is_array_or_object_unwrapped,
    is_function_composition_args, is_ternary_arrow_body,
};
use super::arg_wrapping::{
    ArgOpener, ArgsJoin, append_type_args_with_gap_comments, arrow_hug_refused_by_comments,
    build_args_joined_with_comments, build_arrow_call_body_states,
    build_arrow_gap_break_single_arg_doc, build_arrow_hug_printed_doc, build_arrow_sig_doc,
    build_call_args_expanded, build_call_args_with_blank_lines, build_empty_args_doc,
    build_expand_first_arg_doc, build_printed_argument_doc, build_single_arrow_hug_doc,
    build_single_container_arg_doc, build_ternary_arrow_hug_ladder, could_expand_arrow_chain,
    first_arg_signature_refuses_expand_first, last_arg_arrow_gap_break,
    prepend_arrow_body_comments, should_expand_first_arg, try_hook_deps_args_doc,
    try_hug_multiline_template_arg, wrap_call_with_hard_breaks_paren_line,
    wrap_call_with_soft_breaks, wrap_call_with_will_break_guard,
};
use super::expand_last::{ArgOwner, try_expand_last_arg};
use crate::ast::internal;
use crate::printer::comments::KeywordOperandGap;
use crate::printer::expressions::functions::{
    arrow_signature_has_breaking_comments, prepend_leading,
};
use crate::printer::{ParenContext, Printer};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a new expression with argument wrapping.
    ///
    /// The `new`→callee gap is one of the two expression-level keyword→**value** gaps
    /// (`await`'s is the other), so it takes the family's shared seam,
    /// [`Printer::append_keyword_value_line_comments`]: a `//` there runs to end-of-line,
    /// so the callee cannot stay on the keyword's line, and the whole tail — callee, type
    /// arguments **and** argument list — drops one level rather than landing flush, where
    /// it would read as a sibling statement. The tail is built keyword-less for that
    /// reason: indenting the callee alone would leave a broken argument list rendering at
    /// the outer column.
    ///
    /// The seam, not [`Printer::build_continuation_indent`], because a comment here
    /// **leads the callee**: own-line-ness is authoring signal for a leading position
    /// (conformance_prettier.md §Comment Position Philosophy), so an own-line comment
    /// keeps its line instead of being pulled up to trail `new`.
    pub(super) fn build_new_doc_with_wrapping(
        &self,
        new_expr: &internal::NewExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let keyword_end = new_expr.span.start + "new".len() as u32;
        let callee_start = new_expr.callee.span().start;
        // The `new`→callee value head ([`Printer::value_head_frozen_span`]): an own-line
        // directive in the gap freezes the CALLEE alone — the type arguments and the argument
        // list follow the callee's span and stay parent-owned, so they keep normalizing.
        let frozen = self.value_head_frozen_span(keyword_end, new_expr.callee.span());
        match self.keyword_operand_gap(keyword_end, callee_start) {
            KeywordOperandGap::Continuation => {
                let tail = self.build_new_doc_after_keyword(new_expr, d.empty(), frozen);
                let mut parts: DocBuf = smallvec![d.text("new")];
                self.append_keyword_value_line_comments(
                    &mut parts,
                    keyword_end,
                    callee_start,
                    tail,
                );
                d.concat(&parts)
            }
            KeywordOperandGap::Inline(run) => {
                let keyword = match run {
                    Some(run) => d.concat(&[d.text("new "), run]),
                    None => d.text("new "),
                };
                self.build_new_doc_after_keyword(new_expr, keyword, frozen)
            }
        }
    }

    /// [`Self::build_new_doc_with_wrapping`] past the keyword, which it emits as
    /// `keyword` — `new ` on the ordinary path, and `empty()` where the caller has
    /// already emitted `new` and the gap's continuation supplies the separator.
    fn build_new_doc_after_keyword(
        &self,
        new_expr: &internal::NewExpression<'_>,
        keyword: DocId,
        frozen: Option<Span>,
    ) -> DocId {
        let d = self.d();
        // Wrap callee in parens if needed (e.g., `new (a || b)()`, `new (a ? b : c)()`,
        // an optional chain `new (a?.b)()` — a chain can't be a `new` callee per spec).
        // A non-null assertion sealing a parenthesized chain (`new (a?.b)!()`) keeps the
        // parens via the sealed-base rendering (checked first; the `!`-outside form is
        // not stripped here even though the standalone path would).
        //
        // A FROZEN callee short-circuits the cascade: the slice is its own bytes and the
        // pair the `NewCallee` position supplies rides outside it
        // ([`Printer::build_frozen_value_doc`]), so the arms below — each of which exists to
        // lay out the callee's *contents* — have nothing left to decide.
        //
        // A member chain in callee position prints its `.prop` lookups with no break
        // point, so the constructor name stays one unit and the width falls to the
        // argument list ([`Printer::mark_new_callee_member_lookups`]). Marked ahead of
        // the whole cascade rather than in one arm: the chain reaches this position
        // through three of them — bare (`new a.b.C()`), parenthesized where the callee
        // holds a call (`new (fn(k).a.B)()`), and frozen, where the mark is simply inert.
        self.mark_new_callee_member_lookups(new_expr.callee);
        let callee = if let Some(frozen) = frozen {
            self.build_frozen_value_doc(new_expr.callee, frozen, ParenContext::NewCallee)
        } else if let Some(sealed) = self.build_sealed_non_null_paren_doc(new_expr.callee) {
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
                super::build_callee_parens_doc(self, new_expr.callee, callee_doc)
            }
        } else {
            self.build_expression_doc(new_expr.callee)
        };

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
                d.concat(&[keyword, callee_with_types_base]),
                paren_open,
                new_expr.span.end,
                false,
            );
        }

        // Build callee with type args: `new Foo<K, V>`
        let callee_with_types = d.concat(&[keyword, callee_with_types_base]);

        // Single template literal argument with embedded newlines on the same line as `(` —
        // hug it. A template on its own line declines and falls through to the layouts
        // below, whose `will_break` guards see the template's own newline.
        //
        // Prettier's own position, shared with the plain call and dynamic `import()`:
        // `isTemplateLiteralSingleArg` is the first thing `printCallExpression` asks, above
        // `printCallArguments` and every layout inside it. `new` would reach the same ANSWER from
        // further down only because the two gates it would sit under — `anyArgEmptyLine`
        // and the composition check — are vacuous for a single argument; stating the position
        // once is what keeps the four spellings one rule rather than four that happen to
        // agree.
        if let Some(doc) = try_hug_multiline_template_arg(
            self,
            callee_with_types,
            new_expr.arguments,
            paren_open,
            new_expr.span.end,
        ) {
            return doc;
        }

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
                // Object / array container argument, cast or not: the shared hug, which also
                // owns the truly-empty softline case ([`build_single_container_arg_doc`]). An
                // on-page leading comment declines it (see the guard's comment above);
                // prettier expands the same way.
                arg if is_array_or_object_unwrapped(arg) && !single_arg_leading_on_page_comment => {
                    return build_single_container_arg_doc(self, callee_with_types, arg);
                }
                // Function-expression argument: hug it — the body handles its own formatting.
                internal::Expression::FunctionExpression(_)
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
                    let arg0 = &new_expr.arguments[0];
                    let build = || self.build_expression_doc(arg0);

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
                    // **on page**: a leading comment forces the wrapped (expanded) state,
                    // owned or not — an owned comment rides inside the argument's doc (so
                    // it's not in `glued`) but still defeats the hug, exactly as prettier
                    // expands a block-arrow arg whose leading comment precedes it. A
                    // to-emit gate here would go blind to it and wrongly hug.
                    let has_leading_comment = new_has_comments
                        && self.has_comments_on_page_between(paren_open, arg_start);

                    // If the arrow has trailing param comments or leading comments,
                    // force wrapped state
                    let has_trailing_param_comments =
                        new_has_comments && arrow_signature_has_breaking_comments(self, arrow);

                    // ⚠️ All three refusals are asked BEFORE the printing pair below and read
                    // only the `printedArguments` one — the pair is a second build of the
                    // argument ([`build_arrow_hug_printed_doc`]).
                    if has_trailing_param_comments || has_leading_comment {
                        let printed = prepend_leading(
                            d,
                            glued,
                            build_arrow_hug_printed_doc(self, arg0, arrow, build),
                        );
                        return d.concat(&[
                            callee_with_types,
                            d.text("("),
                            d.indent(d.concat(&[d.softline(), printed])),
                            d.softline(),
                            d.text(")"),
                        ]);
                    }

                    // A broken `=>`→body gap forces the closing paren onto its own line — the
                    // rule is the gap's and not the arms' body-kind split; the one body
                    // question it does owe, prettier's `couldExpandArg`, the helper applies
                    // itself, see [`last_arg_arrow_gap_break`].
                    if new_has_comments
                        && let Some(gap_break) = last_arg_arrow_gap_break(self, arg0)
                    {
                        return build_arrow_gap_break_single_arg_doc(
                            self,
                            ArgOpener::Callee(callee_with_types),
                            arg0,
                            &gap_break,
                            glued,
                            build,
                        );
                    }

                    // The same two printings and state ladder the plain call's and the member
                    // chain's single-argument hugs take ([`build_single_arrow_hug_doc`]) — an
                    // object/array terminal takes the middle state, which expands the body
                    // internally while the arrow stays hugged to `new A(`. The hand-rolled
                    // two-state copy here had no such state, so a FLAT-written body broke the
                    // argument out where prettier hugs.
                    return build_single_arrow_hug_doc(
                        self,
                        ArgOpener::Callee(callee_with_types),
                        arg0,
                        arrow,
                        glued,
                        build,
                    );
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

                            // The ladder, shared with the plain call's
                            // `build_ternary_arrow_hug_states` and the member chain's arm —
                            // asked, like the call's, of the BODY.
                            return build_ternary_arrow_hug_ladder(
                                d,
                                ArgOpener::Callee(callee_with_types),
                                sig_doc,
                                body_doc,
                                body_doc,
                            );
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
                ArgOpener::Callee(callee_with_types),
                new_expr.arguments,
                paren_open,
                new_expr.span.end,
            );
        }

        // No multiline-content arm here, for the reason the plain-call twin states: an
        // argument's own newline breaks the layouts below through their `will_break`
        // guards, which is prettier's only mechanism too.

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
                ArgOpener::Callee(callee_with_types),
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
                self.source.as_bytes(),
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

        // Prettier's `shouldExpandLastArg`, shared with the plain call — one
        // `printCallArguments` prints both spellings, so the layout is not a copy of the
        // call's, it IS the call's (`expand_last.rs`). Must come AFTER the trailing-comment
        // paths above, which own the gaps these states cannot express.
        if let Some(doc) = try_expand_last_arg(
            self,
            new_expr.arguments,
            callee_with_types,
            paren_open,
            new_expr.span.end,
            new_has_comments,
            ArgOwner::New,
        ) {
            return doc;
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
        // before `(`). Also content-preserving: a line comment trailing `(` is
        // kept. See conformance_prettier_ts_comments.md §Comment relocation.
        if has_leading_comments {
            let first_arg_start = new_expr.arguments[0].span().start;
            let gap_pc = PartitionedComments::new(
                self.comments,
                self.source.as_bytes(),
                self.comment_line_breaks,
                paren_open,
                first_arg_start,
            );
            if gap_pc.pulls_to_delimiter_line(self) {
                let mut paren_line_prefix = DocBuf::new();
                gap_pc.emit_delimiter_line_pull(&mut paren_line_prefix, self);

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
    pub(crate) fn build_new_doc(&self, new_expr: &internal::NewExpression<'_>) -> DocId {
        self.build_new_doc_with_wrapping(new_expr)
    }
}
