// Main call expression formatting logic
//
// Contains the primary `build_call_doc_with_wrapping` function that handles
// all the special cases for call expression formatting.

use super::super::{ParenContext, Printer, has_multiline_content, needs_parens};
use super::arg_comments::{
    PartitionedComments, any_comment_forces_expansion, find_comma_pos, first_arg_has_any_comments,
    has_blank_line_between_args, has_inter_argument_comments, has_trailing_comments_on_args,
    is_comment_after_comma, is_comment_before_comma, last_arg_has_comments,
    should_force_expansion_for_comments,
};
use super::arg_predicates::{
    arrow_has_trailing_param_comments, is_array_or_object_unwrapped, is_concise_numeric_array,
    is_curried_arrow, is_function_composition_args, is_ternary_arrow_body,
    last_arg_is_array_or_object, preceding_args_allow_expand_last,
};
use super::arg_wrapping::{
    append_type_args_with_gap_comments, arg_needs_soft_wrap, arrow_has_type_annotations,
    arrow_has_type_reference_return, build_args_joined_with_comments, build_args_split_last,
    build_args_with_blank_lines, build_arrow_call_body_states, build_arrow_inline_signature,
    build_arrow_sig_doc, build_break_body_state, build_empty_args_doc, build_expand_all_args,
    build_inline_args, build_inline_or_expand_all, could_expand_arrow_chain,
    last_two_args_same_type, prepend_arrow_body_comments, should_expand_first_arg,
    try_hug_multiline_template_arg, wrap_call_with_hard_breaks, wrap_call_with_soft_breaks,
};
use super::module_paths::{get_module_path_chain_break, is_boolean_call, is_module_path_no_break};
use super::test_patterns::{callee_chain_string, is_test_call};
use crate::ast::internal;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::arena::DocId;

/// Print a call expression: `foo()`, `obj.method(arg1, arg2)`
///
/// For method chains like `arr.filter().map()`, wraps with leading `.`:
/// ```javascript
/// arr
///     .filter(...)
///     .map(...)
/// ```
///
/// For standalone calls and simple method calls, wraps args when they exceed print_width:
/// ```javascript
/// fn(
///     arg1,
///     arg2,
/// )
/// assert.deepStrictEqual(
///     longArg1,
///     [1, 2],
/// )
/// ```
pub(super) fn build_call_doc_with_wrapping(
    printer: &Printer,
    call: &internal::CallExpression,
) -> DocId {
    let d = printer.d();
    let callee_doc = printer.build_expression_doc(&call.callee);

    // Wrap callee in parens if needed (e.g., ternary: `(a ? b : c)()`)
    // This must happen BEFORE adding removed-paren comments so comments stay outside
    let callee_doc = if needs_parens(&call.callee, ParenContext::Callee) {
        d.parens(callee_doc)
    } else {
        callee_doc
    };

    // Check for comments between removed parentheses and callee
    // e.g., (/* comment */ foo)() has call.span.start at '(' and callee.span.start at 'foo'
    // The comment is in the range [call.span.start, callee.span.start) and needs to be preserved
    // Note: This happens AFTER parens wrapping so `(/* c */ (a ? b : c))()` -> `/* c */ (a ? b : c)()`
    let callee = printer.prepend_removed_paren_comments(
        call.span.start,
        call.callee.span().start,
        callee_doc,
    );

    // Handle optional chaining
    let callee = if call.optional {
        d.concat(&[callee, d.text("?.")])
    } else {
        callee
    };

    // Combine callee with type arguments (`fn<T>`), preserving comments in the gap
    // e.g., `fn/* c1 */ <string>()` — comment between callee and `<`
    let callee = append_type_args_with_gap_comments(
        printer,
        callee,
        call.callee.span().end,
        call.type_arguments.as_ref(),
    );

    // Empty args: just `fn()` or `fn<T>()`, preserving dangling comments
    if call.arguments.is_empty() {
        let after_type_args = call
            .type_arguments
            .as_ref()
            .map_or_else(|| call.callee.span().end, |ta| ta.span.end);
        return build_empty_args_doc(printer, callee, after_type_args, call.span.end);
    }

    // Check for comments inside call arguments (e.g., require(/* comment */ 'a'))
    // If there are line comments, expand to multi-line format
    if call.arguments.len() == 1 {
        let first_arg = &call.arguments[0];
        // Find the opening paren position (after type args if present, otherwise after callee)
        let paren_open = call
            .type_arguments
            .as_ref()
            .map_or_else(|| call.callee.span().end, |ta| ta.span.end);
        let arg_start = first_arg.span().start;
        let arg_end = first_arg.span().end;
        let paren_close = call.span.end;

        // Own-line trailing comments after the arg (any line comment, or a block
        // comment on a line below the arg) aren't handled by the single-arg
        // branches below — defer to the general comment path (which emits them
        // after the trailing comma). Same-line inline trailing block comments
        // (e.g. `fn(/* c */ a /* t */)`) stay on this fast path.
        let has_own_line_trailing_comment =
            tsv_lang::comments_in_range(printer.comments, arg_end, paren_close).any(|c| {
                !c.is_block
                    || !tsv_lang::printing::is_same_line_fast(
                        printer.line_breaks,
                        arg_end,
                        c.span.start,
                    )
            });

        let has_line_comments = printer.has_line_comments_between(paren_open, arg_start);
        if has_line_comments && !has_own_line_trailing_comment {
            // Multi-line format: fn( // comment\n\targ,\n)
            // Comments trailing the `(` on its own line stay there (a divergence from
            // prettier, which relocates them to their own line); own-line comments
            // stay on their own lines before the arg. See conformance_prettier.md
            // §Comment relocation (Call open paren `(`).
            let gap_pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                paren_open,
                arg_start,
            );

            let mut paren_line_prefix = Vec::new();
            gap_pc.emit_trailing_comments(&mut paren_line_prefix, printer);

            let mut inner = Vec::new();
            for comment in &gap_pc.leading {
                inner.push(printer.build_comment_doc(comment));
                inner.push(d.hardline());
            }
            inner.push(printer.build_expression_doc(first_arg));

            return d.concat(&[
                callee,
                d.text("("),
                d.concat(&paren_line_prefix),
                d.indent(d.concat(&[d.hardline(), d.concat(&inner), d.text(",")])),
                d.hardline(),
                d.text(")"),
            ]);
        }

        // Check for inline block comments (single binary search via _opt)
        // Use build_rhs_comments_opt to get spaces between consecutive block comments:
        // fn(/** @type {A} */ /** @type {B} */ expr) — not fn(/** @type {A} *//** @type {B} */ expr)
        if let Some(inline_comments) = printer
            .build_rhs_comments_opt(paren_open, arg_start)
            .filter(|_| !has_own_line_trailing_comment)
        {
            let arg_doc = printer.build_expression_doc(first_arg);

            // Build comment + arg, including any trailing comments after the arg
            // Note: build_rhs_comments_opt already adds trailing space after each comment
            let mut parts = vec![inline_comments, arg_doc];
            if let Some(trailing) =
                printer.build_inline_comments_between_doc_opt(arg_end, paren_close)
            {
                parts.push(trailing);
            }
            let arg_with_comment = d.concat(&parts);

            // If the arg will break internally (multiline content), use expanded format
            // e.g., fn(/* c */ {\n  prop,\n}) → fn(\n  /* c */ {\n    prop,\n  },\n)
            if d.will_break(arg_doc) {
                return wrap_call_with_hard_breaks(d, callee, arg_with_comment);
            }

            // Use soft-break wrapping so outer call can expand when content exceeds print width
            // e.g., fn(/** @type {T} */ call(long_args)) → fn(\n\t/** @type {T} */ call(\n\t\tlong_args,\n\t),\n)
            return wrap_call_with_soft_breaks(d, callee, arg_with_comment);
        }
    }

    // Test function calls (it, test, describe, etc.) stay on one line
    // even if they exceed print width
    if is_test_call(call, printer) {
        // Build callee as flat string (no conditionalGroup)
        // This prevents breaking at `.skip` etc. even when very long
        let flat_callee = match callee_chain_string(&call.callee, printer) {
            Some(callee_str) => d.text_owned(callee_str),
            None => callee,
        };

        // Check for trailing comments on last arg
        let Some(last_arg) = call.arguments.last() else {
            unreachable!("is_test_call requires arguments");
        };
        let paren_close = call.span.end;
        let mut parts = vec![
            flat_callee,
            d.text("("),
            d.join(
                call.arguments
                    .iter()
                    .map(|arg| printer.build_expression_doc(arg)),
                ", ",
            ),
            d.text(")"),
        ];

        // Add trailing comments as line suffix (stays on same line)
        if let Some(suffix) =
            printer.build_trailing_comments_line_suffix(last_arg.span().end, paren_close)
        {
            parts.push(suffix);
        }

        return d.concat(&parts);
    }

    // Module path calls that should not break at arguments (e.g., require.resolve)
    // Keep the call on one line; let assignment/parent break instead
    if is_module_path_no_break(call, printer) && !has_trailing_comments_on_args(call, printer) {
        return d.concat(&[
            callee,
            d.text("("),
            d.join(
                call.arguments
                    .iter()
                    .map(|arg| printer.build_expression_doc(arg)),
                ", ",
            ),
            d.text(")"),
        ]);
    }

    // Module path calls (require.resolve.paths, import.meta.resolve) break at chain
    // rather than at arguments, keeping the path on the same line as the method
    if let Some((base_expr, method_name)) = get_module_path_chain_break(call, printer)
        .filter(|_| !has_trailing_comments_on_args(call, printer))
    {
        let base_doc = printer.build_expression_doc(base_expr);
        let method_str = printer.resolve_symbol(method_name.name);
        let arg_doc = printer.build_expression_doc(&call.arguments[0]);

        // Format: base\n\t.method(arg)
        // When it fits on one line, don't break
        return d.group(d.concat(&[
            base_doc,
            d.indent_softline(d.concat(&[
                d.text_owned(format!(".{method_str}(")),
                arg_doc,
                d.text(")"),
            ])),
        ]));
    }

    // Single function argument: "hugged" formatting
    // - Block arrows stay hugged if first line fits, wrap if it doesn't
    // - Expression arrows use width-aware group (wrap when exceeds line limit)
    // Skip hugging if there are trailing comments - let comment handling block handle it
    // Note: Check both trailing line comments AND trailing block comments
    let has_trailing_block_comment = call.arguments.last().is_some_and(|last_arg| {
        tsv_lang::comments_in_range(printer.comments, last_arg.span().end, call.span.end)
            .any(|c| c.is_block)
    });
    if call.arguments.len() == 1
        && !has_trailing_comments_on_args(call, printer)
        && !has_trailing_block_comment
    {
        let arg = &call.arguments[0];

        // Non-huggable arguments: use soft-break wrapping so outer call can break first
        // (call expressions, member expressions, new expressions, identifiers, conditionals)
        if arg_needs_soft_wrap(arg) {
            let arg_doc = printer.build_arg_expression_doc(arg);
            return wrap_call_with_soft_breaks(d, callee, arg_doc);
        }

        match arg {
            // Block arrow (or expandable arrow chain): use conditional_group to let Doc decide hug vs wrap
            //
            // Expandable arrow chains: `() => () => { block }`, `() => () => ({obj})`
            // are treated identically to block-body arrows. Matches prettier's
            // couldExpandArg recursive check with arrowChainRecursion=true.
            internal::Expression::ArrowFunctionExpression(arrow)
                if !arrow.body.is_expression()
                    || (!arrow_has_type_reference_return(arrow)
                        && could_expand_arrow_chain(arrow)) =>
            {
                // For curried arrows (body is another arrow), skip chain detection
                // so the outer arrow hugs its body — prettier's shouldPrintAsChain
                // is false when expandLastArg is true.
                let curried = is_curried_arrow(arg);
                if curried {
                    printer.skip_arrow_chain.set(true);
                }
                let arrow_doc = printer.build_expression_doc(arg);
                if curried {
                    printer.skip_arrow_chain.set(false);
                }

                // If the arrow has trailing param comments, the params will be multiline,
                // so we should force the wrapped state (prettier behavior)
                let arrow_token = printer.find_arrow_token_for(arrow);
                let has_trailing_param_comments =
                    arrow_has_trailing_param_comments(arrow, arrow_token, |start, end| {
                        printer.has_comments_between(start, end)
                    });
                if has_trailing_param_comments {
                    // Force wrapped state when arrow has trailing param comments
                    return d.concat(&[
                        callee,
                        d.text("("),
                        d.indent(d.concat(&[d.softline(), arrow_doc, d.text(",")])),
                        d.softline(),
                        d.text(")"),
                    ]);
                }

                // For expression-body arrows with obj/array bodies:
                // use 3-state conditional_group matching prettier's shouldExpandLastArg.
                // State 1 forces the arrow to break, causing the body to expand
                // internally (e.g., array items on separate lines) while staying hugged.
                // See also: chain_args.rs's parallel chain-context implementation.
                if let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
                    && matches!(
                        &**body_expr,
                        internal::Expression::ObjectExpression(_)
                            | internal::Expression::ArrayExpression(_)
                    )
                {
                    // When the arrow has own-line comments between => and body,
                    // keep the arrow start on the same line as callee(, but add
                    // trailing comma and break closing paren to its own line.
                    // Matches Prettier's expandLastArg behavior where the arrow
                    // is reprinted with trailingComma + softline appended.
                    let body_start = body_expr.span().start;
                    let arrow_token = printer.find_arrow_token_for(arrow);
                    if printer.has_own_line_post_arrow_comment(arrow_token, body_start) {
                        // group_break forces the arrow to break. Trailing comma
                        // and softline after it cause `,\n)` when the group breaks.
                        let inner = d.concat(&[
                            d.text("("),
                            d.group_break(arrow_doc),
                            d.text(","),
                            d.softline(),
                            d.text(")"),
                        ]);
                        // The group wrapping the call args breaks because of
                        // group_break, causing softline → newline.
                        return d.concat(&[callee, d.group_break(inner)]);
                    }

                    let state_hug = d.concat(&[callee, d.text("("), arrow_doc, d.text(")")]);
                    let state_arrow_break =
                        d.concat(&[callee, d.text("("), d.group_break(arrow_doc), d.text(")")]);
                    let state_all_broken = d.concat(&[
                        callee,
                        d.group_break(d.concat(&[
                            d.text("("),
                            d.indent(d.concat(&[d.line(), arrow_doc, d.trailing_comma()])),
                            d.line(),
                            d.text(")"),
                        ])),
                    ]);
                    return d.conditional_group(&[state_hug, state_arrow_break, state_all_broken]);
                }

                return d.conditional_group(&[
                    // State 1: hugged - callee((arrow) => { body })
                    d.concat(&[callee, d.text("("), arrow_doc, d.text(")")]),
                    // State 2: wrapped - callee(\n\t(arrow) => { body },\n)
                    d.concat(&[
                        callee,
                        d.text("("),
                        d.indent(d.concat(&[d.softline(), arrow_doc, d.text(",")])),
                        d.softline(),
                        d.text(")"),
                    ]),
                ]);
            }

            // Regular function expression: keep hugged (block body handles own formatting)
            internal::Expression::FunctionExpression(_) => {
                return d.concat(&[
                    callee,
                    d.text("("),
                    printer.build_expression_doc(arg),
                    d.text(")"),
                ]);
            }

            // Object/array literals (or type assertions wrapping them): hug them
            // e.g., @decorator({...}), fn([item]), fn({...} as T), fn([...] satisfies T)
            //
            // Non-empty arrays/objects are hugged: the object expands internally
            // while the call stays flat. e.g., `fn({a: 1, b: 2})` → `fn({\n\ta: 1,\n\tb: 2,\n})`
            //
            // Empty arrays/objects use soft wrapping so the call has softlines
            // for fits() evaluation. This allows fluid assignment layouts to
            // detect a break point — without softlines, the marker's fits()
            // measures the full flat width and breaks at `=` instead of letting
            // the call args break. e.g., `const x: LongType = fn([])` should
            // break call args, not at `=`.
            _ if is_array_or_object_unwrapped(arg) => {
                // Truly empty (no elements/properties AND no comments inside):
                // use soft wrapping so the call has softlines for fits() to detect
                // break points in fluid assignment layouts.
                //
                // Non-empty or comment-only objects/arrays: hug them. Comments
                // produce hardlines in the doc, which already provide break points.
                let is_truly_empty = match arg {
                    internal::Expression::ArrayExpression(arr) => {
                        arr.elements.is_empty()
                            && !printer.has_comments_between(arr.span.start, arr.span.end)
                    }
                    internal::Expression::ObjectExpression(obj) => {
                        obj.properties.is_empty()
                            && !printer.has_comments_between(obj.span.start, obj.span.end)
                    }
                    _ => false,
                };
                let arg_doc = printer.build_expression_doc(arg);
                if is_truly_empty {
                    return wrap_call_with_soft_breaks(d, callee, arg_doc);
                }
                return d.concat(&[callee, d.text("("), arg_doc, d.text(")")]);
            }

            // Short literals (non-string or short string): hug them
            // Long string literals and multiline strings should use standard wrapping
            internal::Expression::Literal(lit) => {
                let span_len = (lit.span.end - lit.span.start) as usize;
                let raw = lit.span.extract(printer.source);
                let is_multiline = raw.contains('\n');
                // Hug short, single-line literals (<=25 chars)
                if span_len <= 25 && !is_multiline {
                    return d.concat(&[
                        callee,
                        d.text("("),
                        printer.build_expression_doc(arg),
                        d.text(")"),
                    ]);
                }
                // Long or multiline string - fall through to standard wrapping
            }

            // Expression arrow: check special cases
            internal::Expression::ArrowFunctionExpression(arrow) => {
                if let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body {
                    // Object/array literal: hug it (array breaks internally when long)
                    if matches!(
                        &**body_expr,
                        internal::Expression::ObjectExpression(_)
                            | internal::Expression::ArrayExpression(_)
                    ) {
                        return d.concat(&[
                            callee,
                            d.text("("),
                            printer.build_expression_doc(arg),
                            d.text(")"),
                        ]);
                    }

                    // Expandable body (ternary): use conditional parens
                    // Prettier's "expand last arg" pattern:
                    // - Flat: `map((x) => (x ? y : z))` - parens prevent `<=` ambiguity
                    // - Break: `map((x) =>\n  x ? y : z,)` - no parens, indented
                    // Arrows with TSTypeReference return types are NOT expandable
                    // (prettier's couldExpandArg returns false), so they fall through.
                    if is_ternary_arrow_body(body_expr) && !arrow_has_type_reference_return(arrow) {
                        let sig_doc = build_arrow_sig_doc(printer, arrow);

                        // Build body expression with comments between `=>` and body
                        let body_doc = printer.build_expression_doc(body_expr);
                        let body_doc = prepend_arrow_body_comments(
                            printer,
                            arrow,
                            body_expr.span().start,
                            body_doc,
                        );

                        // Build state 1: break version with params on call line, body breaks
                        // Structure: callee + "(" + sig + " =>" + indent([hardline, body, ","]) + hardline + ")"
                        // First hardline: breaks after "=>"
                        // Second hardline: breaks before ")" to put closing paren on its own line
                        // Note: Use literal "," not trailing_comma() because state[1] is used in Flat mode
                        let state_break = d.concat(&[
                            callee,
                            d.text("("),
                            sig_doc,
                            d.text(" =>"),
                            d.indent(d.concat(&[d.hardline(), body_doc, d.text(",")])),
                            d.hardline(),
                            d.text(")"),
                        ]);

                        // If body has hardlines (e.g., ternary branches with block arrow bodies),
                        // state 0 (flat with parens) would be incorrectly selected by fits()
                        // because hardlines truncate the fit check. Use state_break directly.
                        // (Matches the will_break guard in chain_args.rs)
                        if d.will_break(body_doc) {
                            return state_break;
                        }

                        // Build state 0: fully flat version including call wrapping
                        // Structure: callee + "(" + sig + " => (" + body + ")" + ")"
                        // This includes the full context so conditional_group can measure correctly
                        let state_flat = d.concat(&[
                            callee,
                            d.text("("),
                            sig_doc,
                            d.text(" => ("),
                            body_doc,
                            d.text("))"), // Close both arrow body and call
                        ]);

                        // Build state 2: all args broken out (fallback for Break mode)
                        // This is used when the parent group breaks and we need maximum expansion
                        // Structure: callee + "(\n" + indent([sig + " =>" + indent([hardline, body, ","]) + softline]) + "\n)"
                        let state_all_broken = d.concat(&[
                            callee,
                            d.text("("),
                            d.indent(d.concat(&[
                                d.hardline(),
                                sig_doc,
                                d.text(" =>"),
                                d.indent(d.concat(&[d.hardline(), body_doc, d.trailing_comma()])),
                            ])),
                            d.hardline(),
                            d.text(")"),
                        ]);

                        // Use conditional_group with 3 states to match Prettier
                        // State 0: fully flat
                        // State 1: arrow breaks (checked during fits())
                        // State 2: all broken (only used in Break mode)
                        return d.conditional_group(&[state_flat, state_break, state_all_broken]);
                    }
                }
                // Other expression arrows: fall through to wrap
            }

            // Other arguments: fall through to standard handling
            _ => {}
        }

        // Wrap callback with width-aware breaking
        if let internal::Expression::ArrowFunctionExpression(arrow) = &call.arguments[0] {
            if let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body {
                // Prettier keeps `fn((x) =>` together (sig on opening line) only when:
                // 1. Body is a call expression
                // 2. Arrow is expandable (no TSTypeReference return type)
                // Arrows with TSTypeReference returns (e.g., `: Promise<any>`) are NOT
                // expandable per prettier's couldExpandArg, so they wrap at `fn(`.
                // Keyword returns (`: void`, `: string`) and type predicates are fine.
                if matches!(&**body_expr, internal::Expression::CallExpression(_))
                    && !arrow_has_type_reference_return(arrow)
                {
                    let arrow_doc = printer.build_expression_doc(&call.arguments[0]);
                    let body_doc = printer.build_expression_doc(body_expr);
                    let body_doc = prepend_arrow_body_comments(
                        printer,
                        arrow,
                        body_expr.span().start,
                        body_doc,
                    );
                    let sig_doc = build_arrow_sig_doc(printer, arrow);

                    return build_arrow_call_body_states(d, callee, arrow_doc, sig_doc, body_doc);
                }
                // Other expression types: fall through to standard wrapping
            }
            // Block arrow or non-call expression body: standard wrapping
            let arg_doc = printer.build_expression_doc(&call.arguments[0]);
            return wrap_call_with_soft_breaks(d, callee, arg_doc);
        }
    }

    // Single template literal argument with embedded newlines on the same line
    // as `(` — hug it. A template on its own line falls through to
    // has_multiline_content, which produces the expanded form via
    // wrap_call_with_hard_breaks.
    if let Some(doc) =
        try_hug_multiline_template_arg(printer, callee, &call.arguments, call.span.end)
    {
        return doc;
    }

    // Position after type args (or callee if no type args) — the `(` follows this
    let paren_open = call
        .type_arguments
        .as_ref()
        .map_or_else(|| call.callee.span().end, |ta| ta.span.end);

    // Check if any argument has multiline content (e.g., line continuation strings)
    // Prettier expands calls containing multiline strings (recursively)
    let has_multiline = call
        .arguments
        .iter()
        .any(|arg| has_multiline_content(arg, printer.source));

    if has_multiline {
        // Force expansion with hardlines for multiline content
        let arg_parts =
            build_args_joined_with_comments(printer, &call.arguments, paren_open, true, |p, a| {
                p.build_arg_expression_doc(a)
            });
        return wrap_call_with_hard_breaks(d, callee, arg_parts);
    }

    // Function composition pattern: when any argument is a call containing a callback
    // e.g., fn(arr.map((x) => x), b) → fn(\n\tarr.map((x) => x),\n\tb,\n)
    // Prettier's isFunctionCompositionArgs: 2+ args, any arg is call with function/arrow inside
    // Skip if there are trailing comments - let the comment handling code deal with expansion
    if is_function_composition_args(&call.arguments)
        && !has_trailing_comments_on_args(call, printer)
    {
        let arg_parts =
            build_args_joined_with_comments(printer, &call.arguments, paren_open, true, |p, a| {
                p.build_arg_expression_doc(a)
            });
        return wrap_call_with_hard_breaks(d, callee, arg_parts);
    }

    // "Expand first arg" pattern: when first arg is a function with block body
    // and remaining args are short, hug the function and put tail args after closing }
    // e.g., setTimeout(() => { tick(); }, 100);
    if should_expand_first_arg(printer, &call.arguments)
        && !has_trailing_comments_on_args(call, printer)
        && !first_arg_has_any_comments(&call.arguments, printer, paren_open)
    {
        let first_arg_doc = printer.build_expression_doc(&call.arguments[0]);

        // Build tail args (everything after first)
        let mut tail_parts = Vec::new();
        for arg in call.arguments.iter().skip(1) {
            tail_parts.push(d.text(", "));
            tail_parts.push(printer.build_expression_doc(arg));
        }

        // Prettier: if (tailArgs.some(willBreak)) return allArgsBrokenOut()
        // When any tail arg's doc will break, the inline tail won't work.
        if tail_parts.iter().any(|&id| d.will_break(id)) {
            let arg_parts = build_args_joined_with_comments(
                printer,
                &call.arguments,
                paren_open,
                true,
                // Closure required: method references cause HRTB lifetime errors
                #[allow(clippy::redundant_closure_for_method_calls)]
                |p, a| p.build_expression_doc(a),
            );
            return wrap_call_with_hard_breaks(d, callee, arg_parts);
        }

        // Structure: callee + ( + first_arg_with_breaks + , + tail_args + )
        // The first arg can expand internally, but tail args stay inline
        return d.concat(&[
            callee,
            d.text("("),
            first_arg_doc,
            d.concat(&tail_parts),
            d.text(")"),
        ]);
    }

    // Multiple arrow function arguments: always expand to multiple lines
    // Prettier always expands 2+ arrow function arguments, regardless of source formatting.
    // This matches Prettier's behavior: fn(() => x, () => y) → fn(\n  () => x,\n  () => y,\n)
    let all_args_are_arrows = call.arguments.len() >= 2
        && call
            .arguments
            .iter()
            .all(|arg| matches!(arg, internal::Expression::ArrowFunctionExpression(_)));

    if all_args_are_arrows && !has_trailing_comments_on_args(call, printer) {
        let arg_parts =
            build_args_joined_with_comments(printer, &call.arguments, paren_open, true, |p, a| {
                p.build_expression_doc(a)
            });
        return wrap_call_with_hard_breaks(d, callee, arg_parts);
    }

    // Expand last arg pattern for function/arrow last arguments.
    //
    // Expression-body arrows get special hug/break-body states (call body, object body).
    // Block-body arrows and function expressions use conditional_group (inline vs expand-all).
    //
    // IMPORTANT: Block-body functions CANNOT use the normal group path (wrap_call_with_soft_breaks)
    // because will_break() recurses into nested groups and finds hardlines in the block body,
    // forcing the parent group to break without trying fits(). conditional_group uses fits()
    // directly, correctly measuring the first line including `(x) => {`.
    if call.arguments.len() >= 2 {
        let last_is_function = matches!(
            call.arguments.last(),
            Some(
                internal::Expression::ArrowFunctionExpression(_)
                    | internal::Expression::FunctionExpression(_)
            )
        );

        if last_is_function
            && preceding_args_allow_expand_last(&call.arguments, printer.line_breaks)
            && !any_comment_forces_expansion(call, printer, paren_open)
            && !last_arg_has_comments(&call.arguments, printer, call.span.end, paren_open)
        {
            let (head_parts, last_arg_doc, all_args_broken) =
                build_args_split_last(&call.arguments, printer, paren_open);

            // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
            // When any head arg's doc will break (e.g., new Map([...multiline...])),
            // the hug/inline states won't work — fall through to expand-all.
            if head_parts.iter().any(|&id| d.will_break(id)) {
                return build_expand_all_args(d, callee, all_args_broken);
            }

            // Special case: expression arrow with call/conditional expression body
            // Prettier keeps preceding args inline and only breaks arrow body after =>
            // e.g., fn({a: 1}, (x) =>\n  call(x, ...),\n)
            //
            // Prettier's couldExpandArg includes CallExpression and ConditionalExpression
            // for non-chain arrow bodies. When the last arg doc will_break (e.g., the
            // inner call has a source-multiline object), Prettier skips the flat state
            // and uses only 2 states: break-body and expand-all (matching willBreak path
            // in callArguments.js:167-175).
            if let Some(internal::Expression::ArrowFunctionExpression(arrow)) =
                call.arguments.last()
                && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
                && matches!(
                    &**body_expr,
                    internal::Expression::CallExpression(_)
                        | internal::Expression::ConditionalExpression(_)
                )
                && !arrow_has_type_reference_return(arrow)
            {
                let sig_doc = build_arrow_sig_doc(printer, arrow);
                let body_doc = printer.build_expression_doc(body_expr);
                let body_doc =
                    prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);

                let prefix = d.concat(&[callee, d.text("(")]);
                let state_break_body =
                    build_break_body_state(d, prefix, &head_parts, sig_doc, body_doc);

                let state_expand_all = build_expand_all_args(d, callee, all_args_broken);

                // Prettier: when willBreak(lastArg) is true, skip flat state.
                // The flat state would be selected by fits() (first line is short)
                // but produces wrong closing brackets (e.g., `}));` instead of `}),\n)`).
                if d.will_break(last_arg_doc) {
                    return d.conditional_group(&[state_break_body, state_expand_all]);
                }

                let state_inline = build_inline_args(d, callee, head_parts, last_arg_doc);

                return d.conditional_group(&[state_inline, state_break_body, state_expand_all]);
            }

            // Special case: expression arrow with object/array body
            // Prettier keeps preceding args inline and expands object/array internally
            // e.g., fn(arg, (x) => ({\n  a: x,\n}));
            if let Some(internal::Expression::ArrowFunctionExpression(arrow)) =
                call.arguments.last()
                && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
                && matches!(
                    &**body_expr,
                    internal::Expression::ObjectExpression(_)
                        | internal::Expression::ArrayExpression(_)
                )
                && !arrow_has_type_annotations(arrow)
            {
                let inline_sig = build_arrow_inline_signature(printer, arrow);
                let body_doc = d.parens(printer.build_expression_doc(body_expr));
                let body_doc =
                    prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);

                let state_inline = build_inline_args(d, callee, head_parts.clone(), last_arg_doc);

                let state_hug = d.concat(&[
                    callee,
                    d.text("("),
                    d.concat(&head_parts),
                    inline_sig,
                    d.text(" => "),
                    d.group_break(body_doc),
                    d.text(")"),
                ]);

                let state_expand_all = build_expand_all_args(d, callee, all_args_broken);

                return d.conditional_group(&[state_inline, state_hug, state_expand_all]);
            }

            // Remaining function/arrow last args: block-body arrows, expandable
            // arrow chains, and function expressions use inline-or-expand-all.
            //
            // The only case that skips expand-last: expression-body arrows with
            // non-expandable bodies (e.g., AwaitExpression). Prettier's couldExpandArg
            // returns false for these, so they fall through to the default group path.
            //
            // Arrow chains (`() => () => { block }`) are expandable when the terminal
            // body is Block/Object/Array (prettier's arrowChainRecursion=true check).
            let is_non_expandable_expr_arrow = matches!(
                call.arguments.last(),
                Some(internal::Expression::ArrowFunctionExpression(arrow))
                    if arrow.body.is_expression() && !could_expand_arrow_chain(arrow)
            );
            if !is_non_expandable_expr_arrow {
                return build_inline_or_expand_all(
                    d,
                    callee,
                    head_parts,
                    last_arg_doc,
                    all_args_broken,
                );
            }
        }
    }

    // "Expand last arg" pattern (Prettier's shouldExpandLastArg):
    // When last arg is array/object and preceding args allow it,
    // use different strategies based on whether last two args have same type.
    //
    // IMPORTANT: This must come BEFORE the comment handling path below, because
    // inline block comments before the first arg (e.g., `fn(/** @type {T} */ a, b, {...})`)
    // would otherwise trigger the comment path which doesn't support expand-last layout.
    // `build_args_split_last` already handles leading inline comments correctly.
    //
    // Prettier disables expand-last-arg when the last two arguments have the
    // same outer type (e.g., both arrays, both TSAsExpression). Uses expand-all instead.
    //
    // Prettier also excludes "concise" arrays (all numeric literals) — these use
    // fill layout which has different break characteristics and don't hug.
    //
    // Prettier also blocks expand-last when there are exactly 2 args, the penultimate
    // is an ArrowFunctionExpression, and the last is an array — this covers React hook
    // patterns like `useMemo(() => func(), [dep1, dep2])` where the deps array should
    // NOT be hugged (shouldExpandLastArg, callArguments.js:260-262).
    {
        if call.arguments.len() >= 2
            && last_arg_is_array_or_object(&call.arguments)
            && !call.arguments.last().is_some_and(is_concise_numeric_array)
            && preceding_args_allow_expand_last(&call.arguments, printer.line_breaks)
            && !any_comment_forces_expansion(call, printer, paren_open)
            && !last_arg_has_comments(&call.arguments, printer, call.span.end, paren_open)
            && !(call.arguments.len() == 2
                && matches!(
                    call.arguments.first(),
                    Some(internal::Expression::ArrowFunctionExpression(_))
                )
                && matches!(
                    call.arguments.last(),
                    Some(internal::Expression::ArrayExpression(_))
                ))
        {
            if last_two_args_same_type(&call.arguments) {
                // Same type: use 2-state conditional (inline, expand-all)
                // Don't use the "hug with bracket" state
                let (head_parts, last_arg_doc, all_args_broken) =
                    build_args_split_last(&call.arguments, printer, paren_open);

                // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
                if head_parts.iter().any(|&id| d.will_break(id)) {
                    return build_expand_all_args(d, callee, all_args_broken);
                }

                // Prettier: group(args, { shouldBreak: args.some(willBreak) })
                // When same-type args and any will break, force expand-all.
                // Note: will_break() catches both hardlines and group_break() (source-multiline
                // objects). The different-type path below uses has_forced_break() instead because
                // it has a hug state where group_break objects should still hug.
                if d.will_break(last_arg_doc) {
                    return build_expand_all_args(d, callee, all_args_broken);
                }

                return build_inline_or_expand_all(
                    d,
                    callee,
                    head_parts,
                    last_arg_doc,
                    all_args_broken,
                );
            }

            // Different types: check if last arg has hardlines (e.g., comments)
            // If it does, Prettier uses expand-all instead of hug
            let (head_parts, last_arg_doc, all_args_broken) =
                build_args_split_last(&call.arguments, printer, paren_open);

            // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
            if head_parts.iter().any(|&id| d.will_break(id)) {
                return build_expand_all_args(d, callee, all_args_broken);
            }

            // If last arg has forced breaks (hardlines), use expand-all instead of hug.
            // Note: Use has_forced_break() not will_break() - see comment above.
            if d.has_forced_break(last_arg_doc) {
                return build_inline_or_expand_all(
                    d,
                    callee,
                    head_parts,
                    last_arg_doc,
                    all_args_broken,
                );
            }

            // No hardlines: build 3-state conditional_group
            // State 0: inline - fn('x', [a, b])
            // State 1: hug - fn('x', [\n  a,\n  b,\n]) - head inline, last expands
            // State 2: expand all - fn(\n  'x',\n  [\n    a,\n  ],\n)
            //
            // This ensures:
            // - Short total: stays inline
            // - Long last arg content: head stays inline, last expands internally
            // - Long total due to many/long head args: expand all
            //
            // Key: In state_hug, wrap last_arg_doc in group_break() to force the array/object
            // to break. This makes fits() return true when it hits the first line inside,
            // allowing the hug state to be selected when head args + opening bracket fit.
            // Matches Prettier: group(lastArg, { shouldBreak: true })
            let state_inline = build_inline_args(d, callee, head_parts.clone(), last_arg_doc);
            let state_hug = d.concat(&[
                callee,
                d.text("("),
                d.concat(&head_parts),
                d.group_break(last_arg_doc),
                d.text(")"),
            ]);
            let state_expand_all = build_expand_all_args(d, callee, all_args_broken);
            return d.conditional_group(&[state_inline, state_hug, state_expand_all]);
        }
    }

    // Check for any comments in arguments (leading, inter-argument, or trailing)
    let has_leading_comments = !call.arguments.is_empty()
        && printer.has_comments_between(paren_open, call.arguments[0].span().start);
    let has_inter_arg_comments = has_inter_argument_comments(call, printer);
    let has_trailing_arg_comments = has_trailing_comments_on_args(call, printer);
    // Also check for trailing block comments (has_trailing_comments_on_args only checks line comments)
    let has_any_trailing_comments = has_trailing_arg_comments || has_trailing_block_comment;

    // Check for own-line block comments after the last arg (before closing paren).
    // These need per-element handling to emit after the trailing comma.
    // Also checks inside spread spans for comments from stripped parens.
    let has_own_line_trailing_block = call.arguments.last().is_some_and(|last_arg| {
        let search_start = printer.last_arg_comment_scan_start(last_arg);
        tsv_lang::comments_in_range(printer.comments, search_start, call.span.end).any(|c| {
            c.is_block
                && !tsv_lang::printing::is_same_line_fast(
                    printer.line_breaks,
                    search_start,
                    c.span.start,
                )
        })
    });

    if has_leading_comments
        || has_inter_arg_comments
        || has_any_trailing_comments
        || has_own_line_trailing_block
    {
        // Build arguments with leading and/or inter-argument comments
        let mut arg_parts = Vec::new();
        // Comments trailing the `(` on its own line, kept on the `(` line when the
        // call expands (divergence from prettier, which relocates them to their own
        // line). Injected after `(` in the force-expansion wrap below.
        let mut paren_line_prefix_parts: Vec<DocId> = Vec::new();
        let mut force_expansion = false;
        let mut has_trailing_comma_on_last = false;
        // Block comment trailing the last arg after the comma — preserved after the
        // (synthetic) trailing comma (prettier relocates before; see conformance_prettier.md).
        let mut last_after_comma: Vec<DocId> = Vec::new();

        for (i, arg) in call.arguments.iter().enumerate() {
            // Handle leading comments before first argument
            if i == 0 && has_leading_comments {
                let first_arg_start = arg.span().start;

                if should_force_expansion_for_comments(printer, paren_open, first_arg_start) {
                    force_expansion = true;
                }

                let gap_pc = PartitionedComments::new(
                    printer.comments,
                    printer.line_breaks,
                    paren_open,
                    first_arg_start,
                );
                let has_paren_line =
                    !gap_pc.trailing_block.is_empty() || !gap_pc.trailing_line.is_empty();

                if force_expansion && has_paren_line {
                    // Comments trailing the `(` stay on the `(` line; own-line
                    // comments stay on their own lines before the first arg.
                    // (Inline-collapse cases keep the old behavior below, so a
                    // block comment that hugs the arg — `fn(/* c */ a)` — is
                    // unchanged when the call doesn't expand.)
                    gap_pc.emit_trailing_comments(&mut paren_line_prefix_parts, printer);
                    for comment in &gap_pc.leading {
                        arg_parts.push(printer.build_comment_doc(comment));
                        arg_parts.push(d.hardline());
                    }
                } else {
                    // Build leading comments before first arg.
                    // Inline block comments use space between them (staying on one line).
                    // Line comments and standalone block comments use hardline.
                    let all_inline_block =
                        printer.all_comments_are_inline_block(paren_open, first_arg_start);
                    let mut has_prev_comment = false;
                    for comment in
                        tsv_lang::comments_in_range(printer.comments, paren_open, first_arg_start)
                    {
                        if has_prev_comment {
                            if all_inline_block {
                                arg_parts.push(d.text(" "));
                            } else {
                                arg_parts.push(d.hardline());
                            }
                        }
                        arg_parts.push(printer.build_comment_doc(comment));
                        has_prev_comment = true;
                    }
                    // After last comment: inline block comments use space, others use line.
                    if all_inline_block {
                        arg_parts.push(d.text(" "));
                    } else {
                        arg_parts.push(d.line());
                    }
                }
            }

            // Build the argument
            arg_parts.push(printer.build_expression_doc(arg));

            // Check for comments after this argument (before next arg or closing paren)
            if i < call.arguments.len() - 1 {
                let arg_end = arg.span().end;
                let next_arg_start = call.arguments[i + 1].span().start;

                // Own-line block comments from spread with stripped parens:
                // placed after the comma as siblings in the call.
                let spread_comments = printer.spread_own_line_block_comments(arg);
                if !spread_comments.is_empty() {
                    arg_parts.push(d.text(","));
                    for comment in &spread_comments {
                        arg_parts.push(d.hardline());
                        arg_parts.push(printer.build_comment_doc(comment));
                    }
                    force_expansion = true;
                    arg_parts.push(d.hardline());
                } else if printer.has_comments_between(arg_end, next_arg_start) {
                    if should_force_expansion_for_comments(printer, arg_end, next_arg_start) {
                        force_expansion = true;
                    }

                    let pc = PartitionedComments::new(
                        printer.comments,
                        printer.line_breaks,
                        arg_end,
                        next_arg_start,
                    );

                    let comma_pos = find_comma_pos(printer.source, arg_end, next_arg_start);

                    let has_blank_line = pc.has_blank_line_in_gap(
                        printer.source,
                        printer.line_breaks,
                        arg_end,
                        next_arg_start,
                    );
                    if has_blank_line {
                        force_expansion = true;
                    }

                    if pc.has_trailing_line() {
                        // Trailing line comments: comma, comment, hardline
                        force_expansion = true;
                        arg_parts.push(d.text(","));
                        for comment in &pc.trailing_line {
                            arg_parts.push(d.text(" "));
                            arg_parts.push(printer.build_comment_doc(comment));
                        }
                        if has_blank_line {
                            arg_parts.push(d.literalline());
                        }
                        arg_parts.push(d.hardline());
                    } else if pc.has_trailing_block() {
                        // Trailing block comments: place relative to comma based on source position
                        if let Some(cpos) = comma_pos {
                            for comment in &pc.trailing_block {
                                if is_comment_before_comma(comment, cpos) {
                                    arg_parts.push(d.text(" "));
                                    arg_parts.push(printer.build_comment_doc(comment));
                                }
                            }
                        }
                        arg_parts.push(d.text(","));
                        if has_blank_line {
                            arg_parts.push(d.literalline());
                        }
                        arg_parts.push(d.line());
                        // After-comma block comments (e.g., `arg1, /** @type {T} */ arg2`)
                        // go AFTER the line break so they stay with the next arg when breaking.
                        if let Some(cpos) = comma_pos {
                            for comment in &pc.trailing_block {
                                if is_comment_after_comma(comment, cpos) {
                                    arg_parts.push(printer.build_comment_doc(comment));
                                    arg_parts.push(d.text(" "));
                                }
                            }
                        }
                    } else {
                        // No trailing comments, add comma and line
                        arg_parts.push(d.text(","));
                        if has_blank_line {
                            arg_parts.push(d.literalline());
                        }
                        arg_parts.push(d.line());
                    }

                    // Add leading comments - inline with next arg if on same line
                    pc.emit_leading_comments_inline_aware(&mut arg_parts, printer, next_arg_start);
                } else {
                    let has_blank_line = has_blank_line_between_args(
                        printer.source,
                        printer.line_breaks,
                        arg_end,
                        next_arg_start,
                    );
                    if has_blank_line {
                        // No comments but blank line between args
                        force_expansion = true;
                        arg_parts.push(d.text(","));
                        arg_parts.push(d.literalline());
                        arg_parts.push(d.hardline());
                    } else {
                        // No comments, just comma and line
                        arg_parts.push(d.comma_line());
                    }
                }
            } else {
                // Last argument - check for trailing comments before closing paren.
                // For spread elements, scan inside the spread span for comments from
                // stripped parens (argument.end to spread.end).
                let effective_arg_end = printer.last_arg_comment_scan_start(arg);
                let paren_close = call.span.end;

                let pc = PartitionedComments::new(
                    printer.comments,
                    printer.line_breaks,
                    effective_arg_end,
                    paren_close,
                );

                // Own-line comments (block or line) after the last arg (before closing
                // paren). These appear as siblings after the trailing comma, forcing
                // expansion. Also handles spread with stripped parens via effective_arg_end.
                if !pc.leading.is_empty() {
                    force_expansion = true;
                    pc.emit_last_arg_dangling_comments(
                        &mut arg_parts,
                        printer,
                        &mut has_trailing_comma_on_last,
                    );
                }

                if pc.has_trailing_line() {
                    if !has_trailing_comma_on_last {
                        arg_parts.push(d.text(","));
                    }

                    // Build comment docs: " // comment" for each
                    let comment_docs: Vec<_> = pc
                        .trailing_line
                        .iter()
                        .flat_map(|c| [d.text(" "), printer.build_comment_doc(c)])
                        .collect();
                    let comments = d.concat(&comment_docs);

                    // Line comments always force the CALL to expand - the newline after the
                    // comment means the call must break to multiple lines.
                    force_expansion = true;

                    // Arrays/objects have their own groups that decide internal expansion.
                    // Use line_suffix to exclude the comment from width calculations, so
                    // the array/object can stay inline even when the comment exceeds print_width.
                    // The force_expansion above ensures the call itself expands.
                    if is_array_or_object_unwrapped(arg) {
                        arg_parts.push(d.line_suffix(comments));
                    } else {
                        arg_parts.push(comments);
                    }
                    has_trailing_comma_on_last = true;
                } else if pc.has_trailing_block() {
                    // Trailing block comments: place relative to the source comma.
                    // Before-comma stay after the arg; after-comma are preserved past
                    // the trailing comma (emitted by the wrappers below). Don't force
                    // expansion - let content decide based on width/source newlines.
                    // e.g., fn({short} /* c */) stays inline, fn({long...} /* c */) expands
                    let comma_pos = find_comma_pos(printer.source, effective_arg_end, paren_close);
                    for comment in &pc.trailing_block {
                        if comma_pos.is_some_and(|cp| is_comment_after_comma(comment, cp)) {
                            last_after_comma.push(d.text(" "));
                            last_after_comma.push(printer.build_comment_doc(comment));
                        } else {
                            arg_parts.push(d.text(" "));
                            arg_parts.push(printer.build_comment_doc(comment));
                        }
                    }
                }
            }
        }

        let arg_doc = d.concat(&arg_parts);

        // Force expansion if needed, otherwise allow collapsing.
        // Use a group with break_parent instead of literal hardlines to avoid
        // propagating breaks to parent (e.g., assignment) during fits().
        if force_expansion {
            // Build manually when we have trailing comments (we already added our commas)
            // Add trailing comma after last arg ONLY if we didn't already add one
            let trailing = if has_trailing_comma_on_last {
                d.empty()
            } else {
                d.text(",")
            };
            // Use hardlines for the expansion. The assignment should use NeverBreakAfterOperator
            // for calls since they handle their own expansion.
            // Wrap in group_break so line() separators between non-commented args
            // are forced to Break mode (newlines). Without this, when the call doc
            // is used as a body_doc inside chain_args or other contexts that render
            // in Flat mode, line() between args becomes a space instead of newline.
            return d.concat(&[
                callee,
                d.text("("),
                d.concat(&paren_line_prefix_parts),
                d.group_break(d.concat(&[
                    d.indent(d.concat(&[
                        d.hardline(),
                        arg_doc,
                        trailing,
                        d.concat(&last_after_comma),
                    ])),
                    d.hardline(),
                ])),
                d.text(")"),
            ]);
        }

        // When we have a trailing comment on the last arg, we already added the comma
        // before the comment. Use a custom soft-break structure that doesn't add
        // another trailing comma.
        if has_trailing_comma_on_last {
            return d.concat(&[
                callee,
                d.group(d.concat(&[
                    d.text("("),
                    d.indent_softline(d.concat(&[arg_doc, d.concat(&last_after_comma)])),
                    d.softline(),
                    d.text(")"),
                ])),
            ]);
        }

        // After-comma block comment on the last arg: preserve it past the trailing
        // comma (soft-break wrapper with the comment inserted after `trailing_comma`).
        if !last_after_comma.is_empty() {
            return d.concat(&[
                callee,
                d.group(d.concat(&[
                    d.text("("),
                    d.indent_softline(d.concat(&[
                        arg_doc,
                        d.trailing_comma(),
                        d.concat(&last_after_comma),
                    ])),
                    d.softline(),
                    d.text(")"),
                ])),
            ]);
        }

        return wrap_call_with_soft_breaks(d, callee, arg_doc);
    }

    // Check for blank lines between arguments (forces expansion and preservation).
    // NOTE: This path is only reached when has_inter_arg_comments is false (the
    // comment-handling path above returns early). No comment handling needed here.
    // Uses has_blank_line_between_args to skip stripped grouping paren span gaps.
    let has_blank_lines = call.arguments.windows(2).any(|window| {
        has_blank_line_between_args(
            printer.source,
            printer.line_breaks,
            window[0].span().end,
            window[1].span().start,
        )
    });

    if has_blank_lines {
        // Build arguments with blank line preservation (forced expansion).
        // The shared builder's comment branches never fire here: the comment
        // handling path above returns early when any inter-arg comments exist.
        let arg_doc = build_args_with_blank_lines(printer, &call.arguments);
        return wrap_call_with_hard_breaks(d, callee, arg_doc);
    }

    // Build args with line separators (one per line when broken)
    // Boolean() calls don't get extra indent on binary continuation lines
    let use_arg_indent = !is_boolean_call(call, printer);
    let arg_parts = d.join_doc(
        call.arguments.iter().map(|arg| {
            // For curried arrows (body is another arrow), skip chain detection so the
            // outer arrow hugs its body — matches prettier's expandLastArg behavior.
            let curried = is_curried_arrow(arg);
            if curried {
                printer.skip_arrow_chain.set(true);
            }
            let doc = if use_arg_indent {
                printer.build_arg_expression_doc(arg)
            } else {
                printer.build_expression_doc(arg)
            };
            if curried {
                printer.skip_arrow_chain.set(false);
            }
            doc
        }),
        d.comma_line(),
    );

    // Prettier: group(contents, { shouldBreak: printedArguments.some(willBreak) })
    // If any arg has hardlines (e.g., non-empty block body), force the group to break.
    // This handles block functions before the last arg (e.g., `fn((x) => { body }, aaa)`)
    // without the old has_block_function_before_last check, which was too aggressive —
    // it forced hardlines for empty block bodies like `async () => {}`, preventing
    // calls like `fn([], 3, async () => {}, aaa)` from staying on one line.
    if d.will_break(arg_parts) {
        d.concat(&[
            callee,
            d.group_break(d.concat(&[
                d.text("("),
                d.indent_softline(d.concat(&[arg_parts, d.trailing_comma()])),
                d.softline(),
                d.text(")"),
            ])),
        ])
    } else {
        wrap_call_with_soft_breaks(d, callee, arg_parts)
    }
}
