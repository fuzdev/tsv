// Main call expression formatting logic
//
// Contains the primary `build_call_doc_with_wrapping` function that handles
// all the special cases for call expression formatting.

use super::super::{ParenContext, Printer, has_multiline_content};
use super::arg_comments::{
    PartitionedComments, any_comment_forces_expansion, build_after_comma_leading_comments,
    first_arg_has_any_comments, has_inter_argument_comments, has_trailing_comments_on_args,
    last_arg_has_comments, should_force_expansion_for_comments,
};
use super::arg_predicates::{
    arrow_body_is_call_through_non_null, arrow_has_trailing_param_comments,
    is_array_or_object_unwrapped, is_concise_numeric_array, is_curried_arrow,
    is_function_composition_args, is_ternary_arrow_body, last_arg_is_array_or_object,
};
use super::arg_wrapping::{
    append_type_args_with_gap_comments, arg_needs_soft_wrap, build_args_joined_with_comments,
    build_args_split_last, build_args_with_blank_lines, build_arrow_call_body_states,
    build_arrow_sig_doc, build_break_body_state, build_empty_args_doc, build_expand_all_args,
    build_inline_args, build_inline_or_expand_all, could_expand_arrow_chain,
    last_two_args_same_type, prebuild_expand_last_break_body, prebuild_expand_last_obj_array_body,
    prepend_arrow_body_comments, should_expand_first_arg, try_hug_multiline_template_arg,
    wrap_call_with_hard_breaks, wrap_call_with_soft_breaks,
};
use super::module_paths::{get_module_path_chain_break, is_boolean_call, is_module_path_no_break};
use super::test_patterns::{build_test_callee_flat_doc, is_test_call};
use crate::ast::internal;
use crate::printer::CommentVec;
use crate::printer::expressions::functions::arrow_signature_has_breaking_comments;
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
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
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
) -> DocId {
    let d = printer.d();
    let callee_doc = printer.build_expression_doc(call.callee);

    // Wrap callee in parens if needed (e.g., ternary: `(a ? b : c)()`)
    // This must happen BEFORE adding removed-paren comments so comments stay outside
    let callee_doc = if printer.needs_parens(call.callee, ParenContext::Callee) {
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

    // Single-argument comment paths: leading line comments (multi-line expansion)
    // and inline block comments. Own-line trailing comments defer to the general
    // comment path, so this returns `None` and the caller falls through.
    if call.arguments.len() == 1
        && let Some(doc) = try_single_arg_comment_paths(printer, call, callee)
    {
        return doc;
    }

    // Test function calls (it, test, describe, etc.) stay on one line
    // even if they exceed print width
    if is_test_call(call, printer) {
        // Build callee as a flat doc (no conditionalGroup) straight from the
        // interned chain parts — this prevents breaking at `.skip` etc. even when
        // very long, without materializing a throwaway callee `String`.
        let flat_callee = build_test_callee_flat_doc(call.callee, printer).unwrap_or(callee);

        // Check for trailing comments on last arg. The empty-args case returned
        // above, so `arguments` is non-empty here and `.last()` is always `Some`.
        #[allow(clippy::unreachable)] // empty args already returned above ⇒ last() is Some
        let Some(last_arg) = call.arguments.last() else {
            unreachable!("is_test_call requires arguments");
        };
        let paren_close = call.span.end;
        let mut parts: DocBuf = smallvec![
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

    // Position after type args (or callee if no type args) — the `(` follows this.
    let paren_open = call
        .type_arguments
        .as_ref()
        .map_or_else(|| call.callee.span().end, |ta| ta.span.end);

    // Whole-call comment-presence gate: one binary search over the entire argument
    // window. Every per-argument comment sub-query below (the leading / inter-arg /
    // trailing predicates, each O(n) over args, plus the general comment path) lies
    // within [paren_open, call.span.end], so when the call has no comment they are
    // provably all false/empty — skip them. Canonical reference:
    // build_params_doc_with_comments.
    // Counts owned comments: this asks whether the argument window puts any comment text on
    // the page (a *layout* question), not who emits it — see `has_comments_on_page_between`.
    let call_has_comments = printer.has_comments_on_page_between(paren_open, call.span.end);

    // Module path calls that should not break at arguments (e.g., require.resolve)
    // Keep the call on one line; let assignment/parent break instead
    if is_module_path_no_break(call, printer)
        && !(call_has_comments && has_trailing_comments_on_args(call, printer))
    {
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
        .filter(|_| !(call_has_comments && has_trailing_comments_on_args(call, printer)))
    {
        let base_doc = printer.build_expression_doc(base_expr);
        let method_doc = printer.identifier_name_doc(method_name);
        let arg_doc = printer.build_expression_doc(&call.arguments[0]);

        // Format: base\n\t.method(arg)
        // When it fits on one line, don't break
        return d.group(d.concat(&[
            base_doc,
            d.indent_softline(d.concat(&[
                d.text("."),
                method_doc,
                d.text("("),
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
    let has_trailing_block_comment = call_has_comments
        && call.arguments.last().is_some_and(|last_arg| {
            printer
                .comments_on_page_between(last_arg.span().end, call.span.end)
                .any(|c| c.is_block)
        });
    if call.arguments.len() == 1
        && !(call_has_comments && has_trailing_comments_on_args(call, printer))
        && !has_trailing_block_comment
        && let Some(doc) = try_single_arg_hug(printer, call, callee)
    {
        return doc;
    }

    // Single template literal argument with embedded newlines on the same line
    // as `(` — hug it. A template on its own line falls through to
    // has_multiline_content, which produces the expanded form via
    // wrap_call_with_hard_breaks.
    if let Some(doc) =
        try_hug_multiline_template_arg(printer, callee, call.arguments, call.span.end)
    {
        return doc;
    }

    // Check if any argument has multiline content (e.g., line continuation strings)
    // Prettier expands calls containing multiline strings (recursively)
    let has_multiline = call
        .arguments
        .iter()
        .any(|arg| has_multiline_content(arg, printer.source));

    if has_multiline {
        // Force expansion with hardlines for multiline content
        let arg_parts =
            build_args_joined_with_comments(printer, call.arguments, paren_open, true, |p, a| {
                p.build_arg_expression_doc(a)
            });
        return wrap_call_with_hard_breaks(d, callee, arg_parts);
    }

    // Function composition pattern: when any argument is a call containing a callback
    // e.g., fn(arr.map((x) => x), b) → fn(\n\tarr.map((x) => x),\n\tb,\n)
    // Prettier's isFunctionCompositionArgs: 2+ args, any arg is call with function/arrow inside
    // Skip if there are trailing comments - let the comment handling code deal with expansion
    if is_function_composition_args(call.arguments)
        && !(call_has_comments && has_trailing_comments_on_args(call, printer))
    {
        let arg_parts =
            build_args_joined_with_comments(printer, call.arguments, paren_open, true, |p, a| {
                p.build_arg_expression_doc(a)
            });
        return wrap_call_with_hard_breaks(d, callee, arg_parts);
    }

    // "Expand first arg" pattern: when first arg is a function with block body
    // and remaining args are short, hug the function and put tail args after closing }
    // e.g., setTimeout(() => { tick(); }, 100);
    if should_expand_first_arg(printer, call.arguments)
        && !(call_has_comments && has_trailing_comments_on_args(call, printer))
        && !(call_has_comments && first_arg_has_any_comments(call.arguments, printer, paren_open))
    {
        let first_arg_doc = printer.build_expression_doc(&call.arguments[0]);

        // Build tail args (everything after first), carrying any inline block comment
        // that leads a tail arg after its comma (`}, /* c */ arg`) so it isn't dropped —
        // matching prettier's expand-first, which keeps the comment inline.
        let mut tail_parts = DocBuf::new();
        let mut prev_end = call.arguments[0].span().end;
        for arg in call.arguments.iter().skip(1) {
            tail_parts.push(d.text(", "));
            if let Some(leading) =
                build_after_comma_leading_comments(printer, prev_end, arg.span().start)
            {
                tail_parts.push(leading);
            }
            tail_parts.push(printer.build_expression_doc(arg));
            prev_end = arg.span().end;
        }

        // Prettier: if (tailArgs.some(willBreak)) return allArgsBrokenOut()
        // When any tail arg's doc will break, the inline tail won't work.
        if tail_parts.iter().any(|&id| d.will_break(id)) {
            let arg_parts = build_args_joined_with_comments(
                printer,
                call.arguments,
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

    if all_args_are_arrows && !(call_has_comments && has_trailing_comments_on_args(call, printer)) {
        let arg_parts =
            build_args_joined_with_comments(printer, call.arguments, paren_open, true, |p, a| {
                p.build_expression_doc(a)
            });
        return wrap_call_with_hard_breaks(d, callee, arg_parts);
    }

    // Expand-last pattern for function/arrow last arguments. Returns `None` when
    // there are fewer than 2 args, the guard fails, or the last arg is a
    // non-expandable expression-body arrow (which falls through to the default path).
    if let Some(doc) =
        try_expand_last_function_arg(printer, call, callee, paren_open, call_has_comments)
    {
        return doc;
    }

    // Expand-last pattern for array/object last arguments (Prettier's
    // shouldExpandLastArg). Must come BEFORE the comment-handling path below.
    // Returns `None` when the guard fails, so the caller falls through.
    if let Some(doc) =
        try_expand_last_array_object_arg(printer, call, callee, paren_open, call_has_comments)
    {
        return doc;
    }

    // Comment-handling path: leading, inter-argument, or trailing comments on the
    // arguments. Returns `None` when there are no such comments, so the caller
    // falls through to the blank-line / default layout below.
    if let Some(doc) = build_call_with_arg_comments(
        printer,
        call,
        callee,
        paren_open,
        has_trailing_block_comment,
        call_has_comments,
    ) {
        return doc;
    }

    // Check for blank lines between arguments (forces expansion and preservation).
    // NOTE: This path is only reached when has_inter_arg_comments is false (the
    // comment-handling path above returns early). No comment handling needed here.
    let has_blank_lines = call
        .arguments
        .windows(2)
        .any(|window| printer.is_next_line_empty(window[0].span().end, window[1].span().start));

    if has_blank_lines {
        // Build arguments with blank line preservation (forced expansion).
        // The shared builder's comment branches never fire here: the comment
        // handling path above returns early when any inter-arg comments exist.
        let arg_doc = build_args_with_blank_lines(printer, call.arguments);
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
                d.indent_softline(arg_parts),
                d.softline(),
                d.text(")"),
            ])),
        ])
    } else {
        wrap_call_with_soft_breaks(d, callee, arg_parts)
    }
}

/// Single-argument comment paths: leading line comments (multi-line expansion)
/// and inline block comments before the lone argument. Returns `None` when the
/// argument has no such comments — or has own-line trailing comments, which
/// defer to the general comment path — so the caller falls through.
fn try_single_arg_comment_paths(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    callee: DocId,
) -> Option<DocId> {
    let d = printer.d();
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
    // after the last arg, no trailing comma). Same-line inline trailing block
    // comments (e.g. `fn(/* c */ a /* t */)`) stay on this fast path.
    let has_own_line_trailing_comment =
        printer
            .comments_on_page_between(arg_end, paren_close)
            .any(|c| {
                !c.is_block
                    || !tsv_lang::printing::is_same_line_fast(
                        printer.comment_line_breaks,
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
            printer.comment_line_breaks,
            paren_open,
            arg_start,
        );

        let mut paren_line_prefix = DocBuf::new();
        gap_pc.emit_trailing_comments(&mut paren_line_prefix, printer);

        let mut inner = DocBuf::new();
        // Own-line comments each take their own line (author blanks preserved); a
        // block that hugs the arg stays inline with it (`/* b */ a`).
        gap_pc.emit_leading_comments_inline_aware(&mut inner, printer);
        // Use the argument-context builder so a binary/logical chain (or
        // conditional) gets its continuation indent — matching the no-leading-
        // comment path. `build_expression_doc` would emit the Grouped chain
        // (flush continuation), losing the indent prettier applies here.
        inner.push(printer.build_arg_expression_doc(first_arg));

        return Some(d.concat(&[
            callee,
            d.text("("),
            d.concat(&paren_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&inner)])),
            d.hardline(),
            d.text(")"),
        ]));
    }

    // A block comment before the lone argument — **owned or not** — defeats the
    // argument hug, exactly as prettier's `couldExpandArg` refuses to hug an arg
    // whose leading comment sits before it. This is an **on-page** question (does a
    // comment occupy the page here), not a *to-emit* one: an owned comment (a JSDoc
    // cast / any glued block comment) travels inside the argument's own doc, so it
    // isn't emitted here, but it still forces the expansion — a to-emit gate would
    // go blind to it and wrongly hug.
    //
    // `build_rhs_comments_glued_opt` emits only the non-owned comments (with spaces
    // between consecutive blocks: `fn(/** @type {A} */ /** @type {B} */ expr)`); an
    // owned one is `None` here and rides on `arg_doc`.
    if printer.has_comments_on_page_between(paren_open, arg_start) && !has_own_line_trailing_comment
    {
        let inline_comments = printer.build_rhs_comments_glued_opt(paren_open, arg_start);
        // Argument-context builder so a binary/logical chain gets its
        // continuation indent (matches the no-comment path); see the leading
        // line-comment branch above for the same reasoning.
        let arg_doc = printer.build_arg_expression_doc(first_arg);

        // Build comment + arg, including any trailing comments after the arg
        // Note: build_rhs_comments_opt already adds trailing space after each comment
        let mut parts: DocBuf = DocBuf::new();
        if let Some(inline) = inline_comments {
            parts.push(inline);
        }
        parts.push(arg_doc);
        if let Some(trailing) = printer.build_inline_comments_between_doc_opt(arg_end, paren_close)
        {
            parts.push(trailing);
        }
        let arg_with_comment = d.concat(&parts);

        // If the arg will break internally (multiline content), use expanded format
        // e.g., fn(/* c */ {\n  prop,\n}) → fn(\n  /* c */ {\n    prop,\n  },\n)
        if d.will_break(arg_doc) {
            return Some(wrap_call_with_hard_breaks(d, callee, arg_with_comment));
        }

        // Use soft-break wrapping so outer call can expand when content exceeds print width
        // e.g., fn(/** @type {T} */ call(long_args)) → fn(\n\t/** @type {T} */ call(\n\t\tlong_args,\n\t),\n)
        return Some(wrap_call_with_soft_breaks(d, callee, arg_with_comment));
    }

    None
}

/// Single huggable argument: the "hug" layout cascade (block/expression arrows,
/// function expressions, object/array literals, short literals). Returns `None`
/// for long/multiline literals and other non-arrow arguments that should fall
/// through to standard wrapping.
fn try_single_arg_hug(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    callee: DocId,
) -> Option<DocId> {
    let d = printer.d();
    let arg = &call.arguments[0];

    // Non-huggable arguments: use soft-break wrapping so outer call can break first
    // (call expressions, member expressions, new expressions, identifiers, conditionals)
    if arg_needs_soft_wrap(arg) {
        let arg_doc = printer.build_arg_expression_doc(arg);
        return Some(wrap_call_with_soft_breaks(d, callee, arg_doc));
    }

    match arg {
        // Block arrow (or expandable arrow chain): use conditional_group to let Doc decide hug vs wrap
        //
        // Expandable arrow chains: `() => () => { block }`, `() => () => ({obj})`
        // are treated identically to block-body arrows. Matches prettier's
        // couldExpandArg recursive check with arrowChainRecursion=true.
        internal::Expression::ArrowFunctionExpression(arrow)
            if !arrow.body.is_expression() || could_expand_arrow_chain(arrow) =>
        {
            return Some(build_block_arrow_hug_states(printer, callee, arrow, arg));
        }

        // Regular function expression: keep hugged (block body handles own formatting)
        internal::Expression::FunctionExpression(_) => {
            return Some(d.concat(&[
                callee,
                d.text("("),
                printer.build_expression_doc(arg),
                d.text(")"),
            ]));
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
                        && !printer.has_comments_to_emit_between(arr.span.start, arr.span.end)
                }
                internal::Expression::ObjectExpression(obj) => {
                    obj.properties.is_empty()
                        && !printer.has_comments_to_emit_between(obj.span.start, obj.span.end)
                }
                _ => false,
            };
            let arg_doc = printer.build_expression_doc(arg);
            if is_truly_empty {
                return Some(wrap_call_with_soft_breaks(d, callee, arg_doc));
            }
            return Some(d.concat(&[callee, d.text("("), arg_doc, d.text(")")]));
        }

        // Short literals (non-string or short string): hug them
        // Long string literals and multiline strings should use standard wrapping
        internal::Expression::Literal(lit) => {
            let span_len = (lit.span.end - lit.span.start) as usize;
            let raw = lit.span.extract(printer.source);
            let is_multiline = raw.contains('\n');
            // Hug short, single-line literals (<=25 chars)
            if span_len <= 25 && !is_multiline {
                return Some(d.concat(&[
                    callee,
                    d.text("("),
                    printer.build_expression_doc(arg),
                    d.text(")"),
                ]));
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
                    // A break forced inside the signature invalidates the hug — see
                    // `arrow_signature_has_breaking_comments`. Route to the broken-out
                    // layout instead, which is where prettier's conditionalGroup lands.
                    // Only a FORCED break counts: a merely long body array breaks on fits,
                    // not on a hard line, so it still hugs and expands internally — the
                    // whole point of this arm.
                    let arg_doc = printer.build_expression_doc(arg);
                    if arrow_signature_has_breaking_comments(printer, arrow) {
                        return Some(wrap_call_with_soft_breaks(d, callee, arg_doc));
                    }
                    return Some(d.concat(&[callee, d.text("("), arg_doc, d.text(")")]));
                }

                // Expandable body (ternary): use conditional parens
                // Prettier's "expand last arg" pattern:
                // - Flat: `map((x) => (x ? y : z))` - parens prevent `<=` ambiguity
                // - Break: `map((x) =>\n  x ? y : z,)` - no parens, indented
                // Prettier's couldExpandArg keys on the body type and looks
                // through the return-type annotation, so typed-return arrows
                // (`(x): T => …`) are eligible too.
                if is_ternary_arrow_body(body_expr) {
                    return Some(build_ternary_arrow_hug_states(
                        printer, callee, arrow, body_expr,
                    ));
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
            // Prettier keeps `fn((x) =>` together (sig on opening line) when the
            // body is a call expression (looking through a trailing non-null `!`,
            // per prettier's `stripChainElementWrappers`). couldExpandArg keys on
            // the body type and ignores the return-type annotation, so typed-return
            // arrows (`(x): T => call()`) hug too.
            if arrow_body_is_call_through_non_null(body_expr) {
                // Build the body ONCE and compose both hug/wrap states from it; building
                // the whole arrow separately for the flat state re-built this same body
                // and recursed into itself → O(2^depth) for `a(x => b(y => …))`.
                let body_doc = printer.build_expression_doc(body_expr);
                let body_doc =
                    prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
                let sig_doc = build_arrow_sig_doc(printer, arrow);

                return Some(build_arrow_call_body_states(d, callee, sig_doc, body_doc));
            }
            // Other expression types: fall through to standard wrapping
        }
        // Block arrow or non-call expression body: standard wrapping
        let arg_doc = printer.build_expression_doc(&call.arguments[0]);
        return Some(wrap_call_with_soft_breaks(d, callee, arg_doc));
    }

    None
}

/// Build the hug/wrap states for a single block-body arrow (or expandable arrow
/// chain) argument: `callee((x) => { ... })`. Handles trailing-param-comment
/// forcing, object/array expression bodies, and the default 2-state hug/wrap.
fn build_block_arrow_hug_states(
    printer: &Printer<'_>,
    callee: DocId,
    arrow: &internal::ArrowFunctionExpression<'_>,
    arg: &internal::Expression<'_>,
) -> DocId {
    let d = printer.d();

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
    let arrow_token = arrow.arrow_token;
    let has_trailing_param_comments =
        arrow_has_trailing_param_comments(arrow, arrow_token, |start, end| {
            printer.has_comments_to_emit_between(start, end)
        });
    if has_trailing_param_comments {
        // Force wrapped state when arrow has trailing param comments
        return d.concat(&[
            callee,
            d.text("("),
            d.indent(d.concat(&[d.softline(), arrow_doc])),
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
            internal::Expression::ObjectExpression(_) | internal::Expression::ArrayExpression(_)
        )
    {
        // When the arrow has own-line comments between => and body,
        // keep the arrow start on the same line as callee(, and break the
        // closing paren to its own line (no trailing comma; trailingComma:
        // 'none'). Matches Prettier's expandLastArg behavior where the arrow
        // is reprinted with a softline appended.
        let body_start = body_expr.span().start;
        let arrow_token = arrow.arrow_token;
        if printer.has_own_line_post_arrow_comment(arrow_token, body_start) {
            // group_break forces the arrow to break. The softline after it
            // causes `\n)` when the group breaks.
            let inner = d.concat(&[
                d.text("("),
                d.group_break(arrow_doc),
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
                d.indent(d.concat(&[d.line(), arrow_doc])),
                d.line(),
                d.text(")"),
            ])),
        ]);
        return d.conditional_group(&[state_hug, state_arrow_break, state_all_broken]);
    }

    d.conditional_group(&[
        // State 1: hugged - callee((arrow) => { body })
        d.concat(&[callee, d.text("("), arrow_doc, d.text(")")]),
        // State 2: wrapped - callee(\n\t(arrow) => { body },\n)
        d.concat(&[
            callee,
            d.text("("),
            d.indent(d.concat(&[d.softline(), arrow_doc])),
            d.softline(),
            d.text(")"),
        ]),
    ])
}

/// Build the 3-state expand-last layout for a single expression arrow with a
/// ternary body: `map((x) => (cond ? a : b))`. Flat keeps the parens; the break
/// states drop them and indent the body after `=>` (no trailing comma).
fn build_ternary_arrow_hug_states(
    printer: &Printer<'_>,
    callee: DocId,
    arrow: &internal::ArrowFunctionExpression<'_>,
    body_expr: &internal::Expression<'_>,
) -> DocId {
    let d = printer.d();
    let sig_doc = build_arrow_sig_doc(printer, arrow);

    // Build body expression with comments between `=>` and body
    let body_doc = printer.build_expression_doc(body_expr);
    let body_doc = prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);

    // Build state 1: break version with params on call line, body breaks
    // Structure: callee + "(" + sig + " =>" + indent([hardline, body]) + hardline + ")"
    // First hardline: breaks after "=>"
    // Second hardline: breaks before ")" to put closing paren on its own line
    // No trailing comma (trailingComma: 'none').
    let state_break = d.concat(&[
        callee,
        d.text("("),
        sig_doc,
        d.text(" =>"),
        d.indent(d.concat(&[d.hardline(), body_doc])),
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
            d.indent(d.concat(&[d.hardline(), body_doc])),
        ])),
        d.hardline(),
        d.text(")"),
    ]);

    // Use conditional_group with 3 states to match Prettier
    // State 0: fully flat
    // State 1: arrow breaks (checked during fits())
    // State 2: all broken (only used in Break mode)
    d.conditional_group(&[state_flat, state_break, state_all_broken])
}

/// Expand-last pattern for function/arrow last arguments (the `len >= 2` branch).
///
/// Expression-body arrows get special hug/break-body states (call body, object body).
/// Block-body arrows and function expressions use conditional_group (inline vs expand-all).
///
/// IMPORTANT: Block-body functions CANNOT use the normal group path (wrap_call_with_soft_breaks)
/// because will_break() recurses into nested groups and finds hardlines in the block body,
/// forcing the parent group to break without trying fits(). conditional_group uses fits()
/// directly, correctly measuring the first line including `(x) => {`.
///
/// Returns `None` when there are fewer than 2 args, the guard fails, or the last
/// arg is a non-expandable expression-body arrow (which falls through to the default path).
fn try_expand_last_function_arg(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    callee: DocId,
    paren_open: u32,
    call_has_comments: bool,
) -> Option<DocId> {
    let d = printer.d();
    if call.arguments.len() < 2 {
        return None;
    }

    let last_is_function = matches!(
        call.arguments.last(),
        Some(
            internal::Expression::ArrowFunctionExpression(_)
                | internal::Expression::FunctionExpression(_)
        )
    );

    if last_is_function
        && !(call_has_comments && any_comment_forces_expansion(call, printer, paren_open))
        && !(call_has_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
    {
        // Expand-last arrow whose body is a call / object / array: build the body ONCE and
        // inject it so the whole-arrow arg doc reuses it (the break-body / hug state below
        // reuses it too). Building it in both places recurses into itself → O(2^depth).
        let body_reuse =
            prebuild_expand_last_break_body(printer, call.arguments.last(), call_has_comments);
        let obj_reuse = if body_reuse.is_none() {
            prebuild_expand_last_obj_array_body(printer, call.arguments.last(), call_has_comments)
        } else {
            None
        };
        let inject =
            body_reuse.or_else(|| obj_reuse.map(|(span, inject_doc, _)| (span, inject_doc)));
        let inject_prev = inject.map(|(span, doc)| printer.inject_arrow_body(span, doc));

        let (head_parts, last_arg_doc, all_args_broken) =
            build_args_split_last(call.arguments, printer, paren_open, call_has_comments);

        if let Some(prev) = inject_prev {
            printer.restore_arrow_body_inject(prev);
        }

        // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
        // When any head arg's doc will break (e.g., new Map([...multiline...])),
        // the hug/inline states won't work — fall through to expand-all.
        if head_parts.iter().any(|&id| d.will_break(id)) {
            return Some(build_expand_all_args(d, callee, all_args_broken));
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
        if let Some(internal::Expression::ArrowFunctionExpression(arrow)) = call.arguments.last()
            && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
            && (arrow_body_is_call_through_non_null(body_expr)
                || matches!(&**body_expr, internal::Expression::ConditionalExpression(_)))
        {
            let sig_doc = build_arrow_sig_doc(printer, arrow);
            // Reuse the pre-built call body (see above); conditional bodies build fresh.
            let body_doc =
                body_reuse.map_or_else(|| printer.build_expression_doc(body_expr), |(_, doc)| doc);
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
                return Some(d.conditional_group(&[state_break_body, state_expand_all]));
            }

            let state_inline = build_inline_args(d, callee, &head_parts, last_arg_doc);

            return Some(d.conditional_group(&[state_inline, state_break_body, state_expand_all]));
        }

        // Special case: expression arrow with object/array body
        // Prettier keeps preceding args inline and expands object/array internally
        // e.g., fn(arg, (x) => ({\n  a: x,\n}));
        // couldExpandArg keys only on the body type — a typed arrow expands the
        // same way (its full signature is emitted via build_arrow_sig_doc).
        if let Some(internal::Expression::ArrowFunctionExpression(arrow)) = call.arguments.last()
            && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
            && matches!(
                &**body_expr,
                internal::Expression::ObjectExpression(_)
                    | internal::Expression::ArrayExpression(_)
            )
        {
            let sig_doc = build_arrow_sig_doc(printer, arrow);
            // Reuse the pre-built object/array body (see above).
            let body_doc = obj_reuse.map_or_else(
                || d.parens(printer.build_expression_doc(body_expr)),
                |(_, _, hug)| hug,
            );
            let body_doc =
                prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);

            let state_inline = build_inline_args(d, callee, &head_parts, last_arg_doc);

            let state_hug = d.concat(&[
                callee,
                d.text("("),
                d.concat(&head_parts),
                sig_doc,
                d.text(" => "),
                d.group_break(body_doc),
                d.text(")"),
            ]);

            let state_expand_all = build_expand_all_args(d, callee, all_args_broken);

            return Some(d.conditional_group(&[state_inline, state_hug, state_expand_all]));
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
            return Some(build_inline_or_expand_all(
                d,
                callee,
                &head_parts,
                last_arg_doc,
                all_args_broken,
            ));
        }
    }

    None
}

/// Expand-last pattern for array/object last arguments (Prettier's shouldExpandLastArg).
///
/// Must come BEFORE the comment-handling path, because inline block comments
/// before the first arg (e.g., `fn(/** @type {T} */ a, b, {...})`) would otherwise
/// trigger the comment path which doesn't support expand-last layout;
/// `build_args_split_last` already handles leading inline comments correctly.
///
/// Prettier disables expand-last-arg when the last two arguments have the
/// same outer type (e.g., both arrays, both TSAsExpression). Uses expand-all instead.
///
/// Prettier also excludes "concise" arrays (all numeric literals) — these use
/// fill layout which has different break characteristics and don't hug.
///
/// Prettier also blocks expand-last when there are exactly 2 args, the penultimate
/// is an ArrowFunctionExpression, and the last is an array — this covers React hook
/// patterns like `useMemo(() => func(), [dep1, dep2])` where the deps array should
/// NOT be hugged (shouldExpandLastArg, callArguments.js:260-262).
///
/// Returns `None` when the guard fails, so the caller falls through.
fn try_expand_last_array_object_arg(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    callee: DocId,
    paren_open: u32,
    call_has_comments: bool,
) -> Option<DocId> {
    let d = printer.d();
    if call.arguments.len() >= 2
        && last_arg_is_array_or_object(call.arguments)
        && !call.arguments.last().is_some_and(is_concise_numeric_array)
        && !(call_has_comments && any_comment_forces_expansion(call, printer, paren_open))
        && !(call_has_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
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
        if last_two_args_same_type(call.arguments) {
            // Same type: use 2-state conditional (inline, expand-all)
            // Don't use the "hug with bracket" state
            let (head_parts, last_arg_doc, all_args_broken) =
                build_args_split_last(call.arguments, printer, paren_open, call_has_comments);

            // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
            if head_parts.iter().any(|&id| d.will_break(id)) {
                return Some(build_expand_all_args(d, callee, all_args_broken));
            }

            // Prettier: group(args, { shouldBreak: args.some(willBreak) })
            // When same-type args and any will break, force expand-all.
            // Note: will_break() catches both hardlines and group_break() (source-multiline
            // objects). The different-type path below uses has_forced_break() instead because
            // it has a hug state where group_break objects should still hug.
            if d.will_break(last_arg_doc) {
                return Some(build_expand_all_args(d, callee, all_args_broken));
            }

            return Some(build_inline_or_expand_all(
                d,
                callee,
                &head_parts,
                last_arg_doc,
                all_args_broken,
            ));
        }

        // Different types: check if last arg has hardlines (e.g., comments)
        // If it does, Prettier uses expand-all instead of hug
        let (head_parts, last_arg_doc, all_args_broken) =
            build_args_split_last(call.arguments, printer, paren_open, call_has_comments);

        // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
        if head_parts.iter().any(|&id| d.will_break(id)) {
            return Some(build_expand_all_args(d, callee, all_args_broken));
        }

        // If last arg has forced breaks (hardlines), use expand-all instead of hug.
        // Note: Use has_forced_break() not will_break() - see comment above.
        if d.has_forced_break(last_arg_doc) {
            return Some(build_inline_or_expand_all(
                d,
                callee,
                &head_parts,
                last_arg_doc,
                all_args_broken,
            ));
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
        let state_inline = build_inline_args(d, callee, &head_parts, last_arg_doc);
        let state_hug = d.concat(&[
            callee,
            d.text("("),
            d.concat(&head_parts),
            d.group_break(last_arg_doc),
            d.text(")"),
        ]);
        let state_expand_all = build_expand_all_args(d, callee, all_args_broken);
        return Some(d.conditional_group(&[state_inline, state_hug, state_expand_all]));
    }

    None
}

/// Build the argument-list doc when the arguments carry comments (leading,
/// inter-argument, or trailing). Returns `None` when there are no such comments,
/// so the caller falls through to the blank-line / default layout.
fn build_call_with_arg_comments(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    callee: DocId,
    paren_open: u32,
    has_trailing_block_comment: bool,
    call_has_comments: bool,
) -> Option<DocId> {
    let d = printer.d();

    // Zero-comment fast gate: this path only fires when the argument list has
    // comments; with none, every check below is false and it returns None anyway,
    // so skip them (canonical reference: build_params_doc_with_comments).
    if !call_has_comments {
        return None;
    }

    // Check for any comments in arguments (leading, inter-argument, or trailing)
    let has_leading_comments = !call.arguments.is_empty()
        && printer.has_comments_to_emit_between(paren_open, call.arguments[0].span().start);
    let has_inter_arg_comments = has_inter_argument_comments(call, printer);
    let has_trailing_arg_comments = has_trailing_comments_on_args(call, printer);
    // Also check for trailing block comments (has_trailing_comments_on_args only checks line comments)
    let has_any_trailing_comments = has_trailing_arg_comments || has_trailing_block_comment;

    // Check for own-line block comments after the last arg (before closing paren).
    // These need per-element handling to emit after the trailing comma.
    // Also checks inside spread spans for comments from stripped parens.
    let has_own_line_trailing_block = call.arguments.last().is_some_and(|last_arg| {
        let search_start = printer.last_arg_comment_scan_start(last_arg);
        printer
            .comments_on_page_between(search_start, call.span.end)
            .any(|c| {
                c.is_block
                    && !tsv_lang::printing::is_same_line_fast(
                        printer.comment_line_breaks,
                        search_start,
                        c.span.start,
                    )
            })
    });

    if !(has_leading_comments
        || has_inter_arg_comments
        || has_any_trailing_comments
        || has_own_line_trailing_block)
    {
        return None;
    }

    // Build arguments with leading and/or inter-argument comments
    let mut arg_parts = DocBuf::new();
    // Comments trailing the `(` on its own line, kept on the `(` line when the
    // call expands (divergence from prettier, which relocates them to their own
    // line). Injected after `(` in the force-expansion wrap below.
    let mut paren_line_prefix_parts: DocBuf = DocBuf::new();
    let mut force_expansion = false;

    for (i, arg) in call.arguments.iter().enumerate() {
        // Handle leading comments before first argument
        if i == 0 && has_leading_comments {
            let first_arg_start = arg.span().start;

            if should_force_expansion_for_comments(printer, paren_open, first_arg_start) {
                force_expansion = true;
            }

            let gap_pc = PartitionedComments::new(
                printer.comments,
                printer.comment_line_breaks,
                paren_open,
                first_arg_start,
            );
            let has_paren_line =
                !gap_pc.trailing_block.is_empty() || !gap_pc.trailing_line.is_empty();

            if force_expansion && has_paren_line {
                // Comments trailing the `(` stay on the `(` line; the own-line set
                // then leads the first arg via the shared emitter (a block hugging
                // the arg stays inline, own-line/line comments break, author blanks
                // preserved).
                gap_pc.emit_trailing_comments(&mut paren_line_prefix_parts, printer);
                gap_pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
            } else if !has_paren_line {
                // No comment on the `(` line → every gap comment leads the first
                // arg. Same shared emitter; it ends with the right separator before
                // the arg (space after a hug, hardline after an own-line comment).
                gap_pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
            } else {
                // A block trails the `(` but nothing forces expansion. Every comment
                // in this gap is a block (no line comment reaches here) that is
                // paren-trailing or hugs an arg — all collapsible.
                // Prettier joins consecutive blocks (and the hugged arg) onto one
                // line; an author blank line in the gap breaks the run and is
                // preserved (and forces the call open). A space keeps a block glued
                // to its arg, so a hug (`/* c */ a`) stays inline.
                let comments: CommentVec<'_> =
                    comments_to_emit_in_range(printer.comments, paren_open, first_arg_start)
                        .collect();
                // A blank between two comments, or between the last and the arg, expands
                // the call (the comment interiors are skipped — only the gaps matter).
                let blank_in_gap = comments
                    .windows(2)
                    .any(|w| printer.has_blank_line_between(w[0].span.end, w[1].span.start))
                    || comments.last().is_some_and(|last| {
                        printer.has_blank_line_between(last.span.end, first_arg_start)
                    });
                if blank_in_gap {
                    force_expansion = true;
                }
                let mut prev_end: Option<u32> = None;
                let push_sep = |arg_parts: &mut DocBuf, from: u32, to: u32| {
                    if printer.has_blank_line_between(from, to) {
                        arg_parts.push(d.literalline());
                        arg_parts.push(d.hardline());
                    } else {
                        arg_parts.push(d.text(" "));
                    }
                };
                for comment in &comments {
                    if let Some(pe) = prev_end {
                        push_sep(&mut arg_parts, pe, comment.span.start);
                    }
                    arg_parts.push(printer.build_comment_doc(comment));
                    prev_end = Some(comment.span.end);
                }
                // Separator to the first arg: a space keeps a hugging block glued to
                // it; an author blank line breaks (and is preserved).
                if let Some(pe) = prev_end {
                    push_sep(&mut arg_parts, pe, first_arg_start);
                }
            }
        }

        // Build the argument with the argument-context builder so a binary/logical
        // chain (or conditional) keeps its continuation indent — matching the
        // no-comment path (the single-arg comment path does the same via
        // build_arg_expression_doc).
        arg_parts.push(printer.build_arg_expression_doc(arg));

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
            } else if printer.has_comments_to_emit_between(arg_end, next_arg_start) {
                if should_force_expansion_for_comments(printer, arg_end, next_arg_start) {
                    force_expansion = true;
                }

                // Open the gap (reclassify hugging blocks, emit before/after-comma
                // trailing comments + the comma); the separator + leading comments
                // below finish it.
                let pc = printer.open_inter_arg_gap(&mut arg_parts, arg_end, next_arg_start);

                let has_blank_line =
                    pc.has_blank_line_in_gap(printer.source, printer.layout_line_breaks);
                if has_blank_line || pc.has_trailing_line() {
                    force_expansion = true;
                }
                if has_blank_line {
                    arg_parts.push(d.literalline());
                }
                // A line comment runs to EOL → hard-break; otherwise a soft line so a
                // block-only arg can still collapse inline.
                arg_parts.push(if pc.has_trailing_line() {
                    d.hardline()
                } else {
                    d.line()
                });

                // Leading: own-line comments + after-comma comments that hug the next arg
                // (`C`), emitted inline with it.
                pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
            } else {
                let has_blank_line = printer.is_next_line_empty(arg_end, next_arg_start);
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
                printer.comment_line_breaks,
                effective_arg_end,
                paren_close,
            );

            // Trailing comments after the last arg, before the closing paren, in
            // source order: same-line block comments first, then the same-line line
            // comment (via `line_suffix`), then own-line comments (each on its own
            // line). Emitting same-line comments before own-line ones — and never
            // dropping a block — avoids merging consecutive comments onto one line
            // (which reverses their order) and content loss.

            // (1) Same-line block comments trail the arg in source order. With no
            // trailing comma emitted (trailingComma: 'none'), a block that sat after
            // the source comma simply trails the arg past where the comma was — no
            // split around the never-emitted comma. Don't force expansion on their own
            // — let width/source newlines decide: fn({short} /* c */) stays inline,
            // fn({long...} /* c */) expands.
            for comment in &pc.trailing_block {
                arg_parts.push(d.text(" "));
                arg_parts.push(printer.build_comment_doc(comment));
            }

            // (2) Same-line line comment after the last arg, via `line_suffix`.
            // No trailing comma precedes it (trailingComma: 'none').
            if pc.has_trailing_line() {
                // Build comment docs: " // comment" for each
                let comment_docs: DocBuf = pc
                    .trailing_line
                    .iter()
                    .flat_map(|c| [d.text(" "), printer.build_comment_doc(c)])
                    .collect();

                // Line comments always force the CALL to expand - the newline after the
                // comment means the call must break to multiple lines. A trailing line
                // comment never counts toward width (prettier's `lineSuffix`), so the
                // argument's own group (array/object, binary, conditional, …) can stay
                // inline even when the comment exceeds print_width; force_expansion
                // ensures the call expands.
                force_expansion = true;
                arg_parts.push(d.line_suffix(d.concat(&comment_docs)));
            }

            // (3) Own-line comments (block or line) after the last arg, before the
            // closing paren — emitted each on its own line, with no trailing comma
            // (trailingComma: 'none'). Also handles spread with stripped parens via
            // effective_arg_end.
            if !pc.leading.is_empty() {
                force_expansion = true;
                pc.emit_dangling_comments(&mut arg_parts, printer);
            }
        }
    }

    let arg_doc = d.concat(&arg_parts);

    // Force expansion if needed, otherwise allow collapsing.
    // Use a group with break_parent instead of literal hardlines to avoid
    // propagating breaks to parent (e.g., assignment) during fits().
    if force_expansion {
        // No trailing comma after the last arg (trailingComma: 'none').
        // Use hardlines for the expansion. The assignment should use NeverBreakAfterOperator
        // for calls since they handle their own expansion.
        // Wrap in group_break so line() separators between non-commented args
        // are forced to Break mode (newlines). Without this, when the call doc
        // is used as a body_doc inside chain_args or other contexts that render
        // in Flat mode, line() between args becomes a space instead of newline.
        return Some(d.concat(&[
            callee,
            d.text("("),
            d.concat(&paren_line_prefix_parts),
            d.group_break(d.concat(&[d.indent(d.concat(&[d.hardline(), arg_doc])), d.hardline()])),
            d.text(")"),
        ]));
    }

    Some(wrap_call_with_soft_breaks(d, callee, arg_doc))
}
