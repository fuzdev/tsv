// Main call expression formatting logic
//
// Contains the primary `build_call_doc_with_wrapping` function that handles
// all the special cases for call expression formatting.

use super::super::{
    ParenContext, Printer, container_may_have_multiline_content, has_multiline_content,
};
use super::arg_comments::{
    PartitionedComments, any_arg_empty_line, first_arg_has_any_comments,
    has_inter_argument_comments, has_trailing_comments_on_args,
    should_force_expansion_for_comments,
};
use super::arg_predicates::{
    arrow_body_is_call_through_non_null, is_array_or_object_unwrapped,
    is_function_composition_args, is_ternary_arrow_body,
};
use super::arg_wrapping::{
    ArgOpener, append_type_args_with_gap_comments, arg_needs_soft_wrap,
    arrow_hug_refused_by_comments, build_arrow_call_body_states,
    build_arrow_gap_break_single_arg_doc, build_arrow_hug_printed_doc, build_arrow_sig_doc,
    build_call_args_expanded, build_call_args_with_blank_lines, build_empty_args_doc,
    build_expand_first_arg_doc, build_joined_argument_doc, build_printed_argument_doc,
    build_single_arrow_hug_doc, build_single_container_arg_doc, build_ternary_arrow_hug_ladder,
    could_expand_arrow_chain, first_arg_signature_refuses_expand_first, last_arg_arrow_gap_break,
    prepend_arrow_body_comments, should_expand_first_arg, try_hook_deps_args_doc,
    try_hug_multiline_template_arg, wrap_call_with_soft_breaks, wrap_call_with_will_break_guard,
};
use super::call_paren_open;
use super::expand_last::{ArgOwner, try_expand_last_arg};
use super::module_paths::{get_module_path_chain_break, is_boolean_call, is_module_path_no_break};
use super::test_patterns::{
    build_test_callee_flat_doc, is_test_call, test_call_flat_layout_applies,
};
use crate::ast::internal;
use crate::printer::CommentVec;
use crate::printer::expressions::functions::{
    arrow_signature_has_breaking_comments, function_signature_has_breaking_comments,
};
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

    // Test function calls (`it`, `test.only`, `describe`, …) stay on one line even past
    // the print width — unless an argument gap holds a comment the flat layout has no
    // emitter for, in which case the call expands like any other. Asked ONCE, here, and
    // re-read by the layout branch below: the flat CALLEE and the flat LAYOUT are two
    // halves of one decision, and a second `test_call_flat_layout_applies` call is a
    // second question that can answer differently.
    let test_call_flat = test_call_flat_layout_applies(call, printer);
    // The flat callee is a break-free doc built straight from the chain parts, so a long
    // `it.skip` never breaks at its `.`. It replaces ONLY the callee's own doc, which is why
    // it is built HERE rather than in the layout branch: everything the general path wraps
    // around a callee — the removed-paren comments (`(/* c */ it)(…)`), the type arguments
    // and the gap before them (`it/* c */ <T>(…)`) — applies to it below. A branch that
    // returns a doc assembled from its own callee skips every one of those, `<T>` included.
    let callee_doc = test_call_flat
        .then(|| build_test_callee_flat_doc(call.callee, printer))
        .flatten()
        .unwrap_or_else(|| printer.build_expression_doc(call.callee));

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

    // Handle optional chaining. With an empty argument list and no explicit type
    // arguments, `?.` fuses into the list's own `?.(` instead of gluing onto the
    // callee, so a comment in the callee→`(` gap lands BEFORE `?.` — the side
    // prettier picks, and the member-chain printer's answer to the same gap
    // (`build_chain_args_empty`). With type arguments `?.` precedes them
    // (`call?.<T>()`), and with arguments present it stays on the callee.
    let fuse_optional = call.optional && call.arguments.is_empty() && call.type_arguments.is_none();
    let callee = if call.optional && !fuse_optional {
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
        return build_empty_args_doc(
            printer,
            callee,
            after_type_args,
            call.span.end,
            fuse_optional,
        );
    }

    // Single-argument comment paths: leading line comments (multi-line expansion)
    // and inline block comments. Own-line trailing comments defer to the general
    // comment path, so this returns `None` and the caller falls through.
    if call.arguments.len() == 1
        && let Some(doc) = try_single_arg_comment_paths(printer, call, callee)
    {
        return doc;
    }

    // Position the `(` follows — every argument-gap window (the comment scans, and Rule A's
    // first-argument freeze) opens here.
    let paren_open = call_paren_open(call);

    // The test-call flat layout, whose break-free callee `callee` already carries (built
    // at the top, so the type arguments and the removed-paren comments wrap it there).
    if test_call_flat {
        // Check for trailing comments on last arg. The empty-args case returned
        // above, so `arguments` is non-empty here and `.last()` is always `Some`.
        #[allow(clippy::unreachable)] // empty args already returned above ⇒ last() is Some
        let Some(last_arg) = call.arguments.last() else {
            unreachable!("a test call requires arguments");
        };
        let paren_close = call.span.end;

        // The callback's own parameter list stays flat. Prettier reaches the same place from
        // the other side — its parameter printers ask `isTestCall` of the function's PARENT
        // (`isParametersInTestCall`) — and tsv's printer has no parent link, so the call sets
        // a one-shot flag on the way down instead. `is_test_call` has already established
        // that argument 1 is the callback; its type-parameter builders peek the flag and its
        // value-parameter builder spends it before building anything nested, so nothing below
        // the callback's own signature sees it.
        // The `set` per argument (rather than only at index 1) keeps the flag from surviving
        // an argument that never consumed it.
        //
        // Keyed on the flat LAYOUT — this branch — and not on `is_test_call`, deliberately: the
        // flat-parameter licence is the overrunning line's, so a call that DECLINES the flat
        // layout (an argument gap holds a comment) is an ordinary call, and its callback's
        // parameters break on width like any other's. Prettier keys the same rule on the callee
        // alone, but never reaches the expanded state to disagree — `printCallExpression` takes
        // its test-call branch unconditionally, so its `isParametersInTestCall` holds exactly
        // when the call already printed flat. Both boundaries are pinned by
        // `tests/fixtures/typescript/expressions/calls/test_call_expanded_params_long_prettier_divergence`;
        // reasoning in docs/conformance_prettier.md §Print Width Philosophy.
        let arg_docs: DocBuf = call
            .arguments
            .iter()
            .enumerate()
            .map(|(i, arg)| {
                printer.test_call_flat_params.set(i == 1);
                printer.build_expression_doc(arg)
            })
            .collect();
        printer.test_call_flat_params.set(false);

        let mut parts: DocBuf = smallvec![callee, d.text("("), d.join_doc(arg_docs, d.text(", "))];

        // The last-arg→`)` gap, per the canonical trailing-run rule: a block inline
        // before the `)`, a `//` deferred past it (the flat test-call quirk — prettier
        // relocates it to `}); // c` too, pinned by `calls/trailing_comment_edge_cases`),
        // an own-line comment keeping its own line inside the suffix. The returned
        // must-break is deliberately ignored: the deferred run escapes the flat call by
        // design here, and the flush at the statement's own break ends its line.
        printer.push_trailing_comments_in_range(&mut parts, last_arg.span().end, paren_close);
        parts.push(d.text(")"));

        return d.concat(&parts);
    }

    // Whole-call comment-presence gate: one binary search over the entire argument
    // window. Every per-argument comment sub-query below (the leading / inter-arg /
    // trailing predicates, each O(n) over args, plus the general comment path) lies
    // within [paren_open, call.span.end], so when the call has no comment they are
    // provably all false/empty — skip them. Canonical reference:
    // build_params_doc_with_comments.
    // Counts owned comments: this asks whether the argument window puts any comment text on
    // the page (a *layout* question), not who emits it — see `has_comments_on_page_between`.
    let call_has_comments = printer.has_comments_on_page_between(paren_open, call.span.end);

    // Prettier's React-hook deps-array layout — the FIRST thing `printCallArguments` asks,
    // above `anyArgEmptyLine` and every specialized layout, so an author blank between the
    // callback and the deps array collapses here rather than forcing the arguments out.
    if let Some(doc) = try_hook_deps_args_doc(
        printer,
        call.arguments,
        paren_open,
        call.span.end,
        call_has_comments,
        d.concat(&[callee, d.text("(")]),
    ) {
        return doc;
    }

    // A trailing LINE comment on an argument (`fn(a, // c⏎ b)`) rules out every layout
    // below that would have to place the arguments itself: each joins them with `", "` or
    // hugs them, and a comment running to EOL can't survive either — so they all decline
    // and the call falls through to the comment-aware paths. Computed once: it is pure in
    // `(call, printer)`, six arms ask it, and each ask walks the arguments with a binary
    // search per gap. (`build_call_with_arg_comments` hoists the same predicate for the
    // same reason.) The `call_has_comments` conjunct keeps a comment-free call at the one
    // window-wide binary search above.
    let arg_trailing_line_comment =
        call_has_comments && has_trailing_comments_on_args(call, printer);

    // Prettier's `anyArgEmptyLine` (`print/call-arguments.js`): an author blank line in ANY
    // inter-argument gap forces `allArgsBrokenOut()` — and that test sits ABOVE
    // `shouldExpandFirstArg` / `shouldExpandLastArg`, so the blank defeats every specialized
    // layout rather than being asked about after one has been chosen. tsv used to ask it only
    // at the bottom of this dispatcher, below every early return, so the blank survived a
    // plain argument list and was silently eaten by each hug / expand / composition path — the
    // blank-DROP class (docs/blank_audit.md), invisible to every gate but a prettier compare.
    // Hoisted here as a DECLINE conjunct on those arms rather than as an early return, so a
    // call whose gaps also hold comments still reaches `build_call_with_arg_comments` (which
    // owns the blank question for a commented gap) — the same shape as
    // `arg_trailing_line_comment` above, and what keeps `build_call_args_with_blank_lines`'s
    // "no inter-arg comments here" invariant true.
    //
    // ⚠️ A **test call has no `anyArgEmptyLine`**, and the predicate is `is_test_call` — the
    // CALLEE — not `test_call_flat_layout_applies`, the layout that fired above. Prettier's
    // `printCallExpression` takes its `isTestCall` branch unconditionally and joins the
    // arguments itself, so `printCallArguments` (and with it `anyArgEmptyLine`) is never
    // reached for one; tsv's flat layout additionally DECLINES when an argument gap holds a
    // comment it has no emitter for (the sanctioned divergence pinned by
    // `test_call_arg_comment_prettier_divergence`). Keying the blank on the layout would let
    // a declined test call keep the blank, and that is not idempotent: pass 1 glues the
    // own-line comment to its argument, which makes the flat layout apply on pass 2, which
    // drops the blank again. Keying it on the callee gives both passes the same answer —
    // prettier's — and `blanks:audit` caught exactly this as three NON-IDEMPOTENT shapes.
    let any_arg_empty_line =
        !is_test_call(call, printer) && any_arg_empty_line(call.arguments, printer);

    // A trailing BLOCK comment on the last argument — the block twin of the predicate above,
    // disqualifying the hug arms the same way. Computed here rather than at the first hug
    // because the blank gate below hands it to `build_call_with_arg_comments`; it is pure and
    // already short-circuited by `call_has_comments`, so hoisting it costs nothing.
    let has_trailing_block_comment = call_has_comments
        && call.arguments.last().is_some_and(|last_arg| {
            printer
                .comments_on_page_between(last_arg.span().end, call.span.end)
                .any(|c| c.is_block)
        });

    // Prettier's POSITION for `anyArgEmptyLine` — it short-circuits to `allArgsBrokenOut()`
    // above every specialized layout. ONE site rather than a decline conjunct on each arm, so
    // a layout added below cannot forget to ask; the two forms are equivalent because every
    // arm this jumps over is either single-argument (a blank needs two arguments to sit
    // between, so the question is vacuous there) or would have declined on the blank anyway.
    //
    // WHICH builder owns it is the gaps' answer, not this gate's: a commented gap belongs to
    // `build_call_with_arg_comments`, which carries the blank through its own per-gap
    // emitters, and only a comment-free one reaches `build_call_args_with_blank_lines` —
    // whose "no inter-argument comments here" invariant is exactly what this fallback states.
    if any_arg_empty_line {
        return build_call_with_arg_comments(
            printer,
            call,
            callee,
            paren_open,
            has_trailing_block_comment,
            call_has_comments,
        )
        .unwrap_or_else(|| {
            build_call_args_with_blank_lines(
                printer,
                callee,
                call.arguments,
                paren_open,
                call.span.end,
            )
        });
    }

    // Module path calls that should not break at arguments (e.g., require.resolve)
    // Keep the call on one line; let assignment/parent break instead
    if is_module_path_no_break(call, printer) && !arg_trailing_line_comment {
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
    if let Some((base_expr, method_name)) =
        get_module_path_chain_break(call, printer).filter(|_| !arg_trailing_line_comment)
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
    if call.arguments.len() == 1
        && !arg_trailing_line_comment
        && !has_trailing_block_comment
        && let Some(doc) = try_single_arg_hug(printer, call, callee)
    {
        return doc;
    }

    // Single template literal argument with embedded newlines on the same line
    // as `(` — hug it. A template on its own line falls through to
    // has_multiline_content, which produces the expanded form via
    // build_call_args_expanded.
    if let Some(doc) =
        try_hug_multiline_template_arg(printer, callee, call.arguments, call.span.end)
    {
        return doc;
    }

    // Check if any argument has multiline content (e.g., line continuation strings)
    // Prettier expands calls containing multiline strings (recursively)
    let has_multiline = container_may_have_multiline_content(call.span, printer.source)
        && call
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, printer.source));

    if has_multiline {
        // Force expansion with hardlines for multiline content
        return build_call_args_expanded(
            printer,
            ArgOpener::Callee(callee),
            call.arguments,
            paren_open,
            call.span.end,
        );
    }

    // Function composition pattern: when any argument is a call containing a callback
    // e.g., fn(arr.map((x) => x), b) → fn(\n\tarr.map((x) => x),\n\tb,\n)
    // Prettier's isFunctionCompositionArgs: 2+ args, any arg is call with function/arrow inside
    // Skip if there are trailing comments - let the comment handling code deal with expansion
    if is_function_composition_args(call.arguments) && !arg_trailing_line_comment {
        return build_call_args_expanded(
            printer,
            ArgOpener::Callee(callee),
            call.arguments,
            paren_open,
            call.span.end,
        );
    }

    // "Expand first arg" pattern: when first arg is a function with block body
    // and remaining args are short, hug the function and put tail args after closing }
    // e.g., setTimeout(() => { tick(); }, 100);
    // The inline tail can carry neither a comment running to EOL nor one leading the first
    // argument. Named rather than inlined, matching the `new` twin.
    let expand_first_blocked = arg_trailing_line_comment
        || (call_has_comments
            && (first_arg_has_any_comments(call.arguments, printer, paren_open)
                || first_arg_signature_refuses_expand_first(printer, call.arguments)));
    if should_expand_first_arg(printer, call.arguments) && !expand_first_blocked {
        return build_expand_first_arg_doc(
            printer,
            ArgOpener::Callee(callee),
            call.arguments,
            paren_open,
            call.span.end,
        );
    }

    // No all-arrows arm here: `is_function_composition_args` above already covers it.
    // 2+ arguments that are ALL arrows means `function_count > 1`, which that predicate
    // short-circuits `true` on, and both arms decline on the same
    // `arg_trailing_line_comment` — so an arrow-only arm below it can never be reached.
    //
    // Prettier's `shouldExpandLastArg`, shared with the `new` printer (one
    // `printCallArguments` prints both). Must come BEFORE the comment-handling path below:
    // an inline block comment ahead of the first argument (`fn(/* c */ a, b, {…})`) would
    // otherwise take that path, which has no expand-last layout, and `build_args_split_last`
    // already handles such a comment correctly. Returns `None` when the guard fails, so the
    // caller falls through.
    if let Some(doc) = try_expand_last_arg(
        printer,
        call.arguments,
        callee,
        paren_open,
        call.span.end,
        call_has_comments,
        ArgOwner::Call,
    ) {
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

    // Build args with line separators (one per line when broken)
    // Boolean() calls don't get extra indent on binary continuation lines
    let use_arg_indent = !is_boolean_call(call, printer);
    let arg_parts = d.join_doc(
        call.arguments.iter().map(|arg| {
            // This is prettier's `printedArguments` — printed with no `expandLastArg`, so a
            // curried chain takes the progressive layout (`build_printed_argument_doc`).
            build_printed_argument_doc(printer, arg, || {
                if use_arg_indent {
                    printer.build_arg_expression_doc(arg)
                } else {
                    printer.build_expression_doc(arg)
                }
            })
        }),
        d.comma_line(),
    );

    // Prettier: group(contents, { shouldBreak: printedArguments.some(willBreak) }).
    //
    // The explicit shouldBreak is NOT redundant, though it looks it: a forced break in
    // `arg_parts` does break this group at *render* (the plain-group arm keys on
    // `should_break || will_break(contents)`), but `arena_fits`' Group arm keys only on
    // `should_break` — tsv has no propagateBreaks. So inside an outer FLAT fits walk a
    // plain group is measured flat to its first hardline, where a `group_break` is
    // entered in Break mode and ends the line at its first softline. Prettier's
    // propagateBreaks makes its own fits see such a group broken, i.e. like the
    // `group_break` side. The two shapes therefore differ wherever an outer flat fits
    // measures this call — a `conditional_group` state, a fill part — and the angle-bracket
    // cast ladder is a live observer: without the guard, `<A>fn(a, (y) => { … }, c)` is
    // measured short enough to select the parenthesized state, printing `<A>(⏎fn(…)⏎)`
    // where prettier keeps the cast attached. See
    // `expressions/type_assertion_call_block_arg_long`.
    //
    // This still handles block functions before the last arg (`fn((x) => { body }, aaa)`)
    // without the old has_block_function_before_last check, which was too aggressive — it
    // forced hardlines for empty block bodies like `async () => {}`, preventing calls like
    // `fn([], 3, async () => {}, aaa)` from staying on one line.
    wrap_call_with_will_break_guard(d, callee, arg_parts)
}

/// Close the lone argument's `argument`→`)` gap.
///
/// Every branch of [`try_single_arg_comment_paths`] opens a gap *before* the argument, and
/// each one owes the gap *after* it — the branches are chosen on what leads the argument,
/// but they all print the whole `(…)`. `has_own_line_trailing_comment` has already routed
/// every own-line comment in this gap to the general path, so what reaches here is
/// same-line blocks, which stay on the argument's line.
///
/// One emitter because a branch that forgot it printed the gap it opened and dropped the
/// one it closed (`f(⏎// c⏎a /* t */)` → `f(⏎// c⏎a)`).
fn push_lone_arg_trailing_comments(
    printer: &Printer<'_>,
    parts: &mut DocBuf,
    arg_end: u32,
    paren_close: u32,
) {
    if let Some(trailing) = printer.build_inline_comments_between_doc_opt(arg_end, paren_close) {
        parts.push(trailing);
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

    // TODO: a lone argument answers the delimiter-line question differently from every
    // sibling (`docs/comments.md` §The delimiter-line question). With a block-only run in
    // the `(`→argument gap, the hug branch below joins the whole run with spaces, MERGING
    // comments the author put on separate lines (`fn(/* c1 */⏎/* c2 */⏎a)` →
    // `fn(⏎/* c1 */ /* c2 */⏎a⏎)`). This is an F1 IDEMPOTENCY break, not just a prettier
    // divergence: the merged form is not a fixed point, and pass 2 collapses the whole
    // call to `fn(/* c1 */ /* c2 */ a);`. Every other argument count, and `new`/member-chain at this
    // one, pull the `(`-line comment onto the `(` and give each own-line comment its own
    // line. Deferring the gap to the general comment path is NOT the fix on its own: it
    // regresses the blank-line-after-a-leading-comment shape
    // (`calls/first_arg_leading_comment_blank`, `calls/leading_arg_block_comment_newline`,
    // `syntax/comments/blank_line_after_value_leading_comment`), so the general path needs
    // the blank-preserving arm first. Needs a fixture.
    if has_line_comments && !has_own_line_trailing_comment {
        // Multi-line format: fn( // comment\n\targ,\n)
        // Comments trailing the `(` on its own line stay there (a divergence from
        // prettier, which relocates them to their own line); own-line comments
        // stay on their own lines before the arg. See conformance_prettier_ts_comments.md
        // §Comment relocation (Call open paren `(`).
        let gap_pc = PartitionedComments::new(
            printer.comments,
            printer.comment_line_breaks,
            paren_open,
            arg_start,
        );

        let mut paren_line_prefix = DocBuf::new();
        gap_pc.emit_delimiter_line_pull(&mut paren_line_prefix, printer);

        let mut inner = DocBuf::new();
        // Own-line comments each take their own line (author blanks preserved); a
        // block that hugs the arg stays inline with it (`/* b */ a`).
        gap_pc.emit_leading_comments_inline_aware(&mut inner, printer);
        // Use the argument-context builder so a binary/logical chain (or
        // conditional) gets its continuation indent — matching the no-leading-
        // comment path. `build_expression_doc` would emit the Grouped chain
        // (flush continuation), losing the indent prettier applies here.
        // An own-line directive in the gap freezes the argument verbatim (Rule A);
        // this branch already keeps such a comment on its own line.
        inner.push(build_joined_argument_doc(
            printer,
            paren_open,
            call.arguments,
            0,
        ));
        push_lone_arg_trailing_comments(printer, &mut inner, arg_end, paren_close);

        return Some(d.concat(&[
            callee,
            d.text("("),
            d.concat(&paren_line_prefix),
            d.indent_hardline(d.concat(&inner)),
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
        // The broke-after half of this gap first: a block-only run glued to `(`
        // whose last comment the author broke after rides its newline-after soft
        // `line` inside the same wrap group — own line when the argument breaks
        // the call open, glued bytes when everything collapses (prettier's
        // `printLeadingComment` `line`). An own-line-authored run declines the
        // gate and keeps the glued emitter below, whose glue answers preserve
        // its breaks; so does an owned comment, which rides the argument's doc.
        if let Some(run) = printer.opener_trailing_broke_after_run(paren_open, arg_start) {
            let mut parts: DocBuf = DocBuf::new();
            printer.push_leading_run_with_soft_line(&mut parts, &run);
            parts.push(build_joined_argument_doc(
                printer,
                paren_open,
                call.arguments,
                0,
            ));
            push_lone_arg_trailing_comments(printer, &mut parts, arg_end, paren_close);
            return Some(wrap_call_with_soft_breaks(d, callee, d.concat(&parts)));
        }
        let inline_comments = printer.build_rhs_comments_glued_opt(paren_open, arg_start);
        // Argument-context builder so a binary/logical chain gets its
        // continuation indent (matches the no-comment path); see the leading
        // line-comment branch above for the same reasoning. A directive alone on its
        // line freezes the argument (Rule A) — only the BLOCK spelling reaches here
        // (a line comment routes to the branch above).
        let arg_doc = build_joined_argument_doc(printer, paren_open, call.arguments, 0);

        // Leading run, argument, trailing run — `build_rhs_comments_glued_opt` already
        // adds the trailing space after each comment it emits.
        let mut parts: DocBuf = DocBuf::new();
        if let Some(inline) = inline_comments {
            parts.push(inline);
        }
        parts.push(arg_doc);
        push_lone_arg_trailing_comments(printer, &mut parts, arg_end, paren_close);
        let arg_with_comment = d.concat(&parts);

        // Soft-break wrapping so the outer call can expand when content exceeds print
        // width — e.g., fn(/** @type {T} */ call(long_args)) →
        // fn(\n\t/** @type {T} */ call(\n\t\tlong_args,\n\t),\n). An arg that breaks
        // internally (multiline content) breaks this group with it, which is already
        // the expanded form — no separate hard-break arm.
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

    // A broken `=>`→body gap keeps the arrow start on the `callee(` line and breaks the
    // closing paren onto its own. **Above the match**, because the rule is the gap's and not
    // the body's ([`last_arg_arrow_gap_break`]) — asking it
    // inside the block-arrow arm left every other body kind to the arms below, which are
    // body-keyed and answer this gap not at all: a call or ternary body took its own
    // reassembling layout and printed the signature past the print width with no state to
    // fall through to (`calls/arrow_object_body_own_line_comment_long`).
    //
    // A break forced inside the SIGNATURE still wins, as it did when this lived one level
    // down: that layout renders the signature's head on the callee's line, which the break
    // makes impossible, so the block-arrow arm's own refusal keeps it.
    if let internal::Expression::ArrowFunctionExpression(arrow) = arg
        && !arrow_signature_has_breaking_comments(printer, arrow)
        && let Some(gap_break) = last_arg_arrow_gap_break(printer, arg)
    {
        return Some(build_arrow_gap_break_single_arg_doc(
            printer,
            ArgOpener::Callee(callee),
            arg,
            &gap_break,
            None,
            || printer.build_expression_doc(arg),
        ));
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

        // Regular function expression: keep hugged (block body handles own formatting),
        // unless a comment forces its parameter list multiline — the same refusal the block
        // arrow one arm up makes (`build_block_arrow_hug_states`). The hug renders the
        // callee and the signature's head on one line; a forced break inside that signature
        // invalidates it, so the call expands instead.
        internal::Expression::FunctionExpression(func) => {
            let arg_doc = printer.build_expression_doc(arg);
            if function_signature_has_breaking_comments(printer, func) {
                return Some(d.concat(&[
                    callee,
                    d.text("("),
                    d.indent(d.concat(&[d.softline(), arg_doc])),
                    d.softline(),
                    d.text(")"),
                ]));
            }
            return Some(d.concat(&[callee, d.text("("), arg_doc, d.text(")")]));
        }

        // Object/array literals, and type assertions wrapping them — the shared arm, so the
        // `new` twin cannot answer either half differently
        // ([`build_single_container_arg_doc`]).
        _ if is_array_or_object_unwrapped(arg) => {
            return Some(build_single_container_arg_doc(printer, callee, arg));
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

        // Expression arrow whose body is neither a block nor expandable — an object or array
        // body cannot reach here, since [`could_expand_arrow_chain`] claims it for the first
        // arm above. (Probed: 0 hits over 15.5k real files and the whole fixture suite, which
        // is what the guard's complement already says.)
        internal::Expression::ArrowFunctionExpression(arrow) => {
            if let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body {
                // Expandable body (ternary): use conditional parens
                // Prettier's "expand last arg" pattern:
                // - Flat: `map((x) => (x ? y : z))` - parens prevent `<=` ambiguity
                // - Break: `map((x) =>\n  x ? y : z,)` - no parens, indented
                // Prettier's couldExpandArg keys on the body type and looks
                // through the return-type annotation, so typed-return arrows
                // (`(x): T => …`) are eligible too.
                // The reassembling arm's refusal pair (`arrow_hug_refused_by_comments`): a
                // break forced inside the signature this conditional-group cannot honor, and
                // a comment on the body's tail — every state below reassembles the argument
                // from a signature and a body doc, synthesizing its own parens around the
                // ternary, so such a comment reaches no emitter.
                if is_ternary_arrow_body(body_expr)
                    && !arrow_hug_refused_by_comments(printer, arrow, body_expr)
                {
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
            if arrow_body_is_call_through_non_null(body_expr)
                // The reassembling arm's refusal pair — a break forced inside the signature
                // invalidates this hug exactly as it does the object/array one above, and so
                // does a comment on the body's own tail, which the states this builds
                // reassemble past (`arrow_hug_refused_by_comments`).
                && !arrow_hug_refused_by_comments(printer, arrow, body_expr)
            {
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
        // Block arrow or non-call expression body: standard wrapping.
        // Nothing here hugs, so this is prettier's `printedArguments` shape — a curried
        // chain takes the progressive layout (`build_printed_argument_doc`).
        let arg = &call.arguments[0];
        let arg_doc =
            build_printed_argument_doc(printer, arg, || printer.build_expression_doc(arg));
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
    let build = || printer.build_expression_doc(arg);

    // ⚠️ Both comment refusals are asked BEFORE the pair below and read only the
    // `printedArguments` printing — the pair is a second build of the argument, which
    // recurses into any call nested in its body ([`build_arrow_hug_printed_doc`]).

    // If the arrow has trailing param comments, the params will be multiline,
    // so we should force the wrapped state (prettier behavior)
    if arrow_signature_has_breaking_comments(printer, arrow) {
        // Force wrapped state when arrow has trailing param comments
        let printed = build_arrow_hug_printed_doc(printer, arg, arrow, build);
        return d.concat(&[
            callee,
            d.text("("),
            d.indent(d.concat(&[d.softline(), printed])),
            d.softline(),
            d.text(")"),
        ]);
    }

    // ⚠️ An own-line comment between `=>` and the body is NOT asked here — `try_single_arg_hug`
    // answers it above its whole body-kind match, because the rule is the gap's and not the
    // body's and the arms past this builder never asked it at all. The only way such a comment
    // reaches here is behind the signature-break refusal above, which returns first.

    // The two printings and the state ladder they feed, shared with the `new` and
    // member-chain single-argument arms — an object/array terminal gets the middle state that
    // expands it internally while the arrow stays hugged.
    build_single_arrow_hug_doc(printer, ArgOpener::Callee(callee), arg, arrow, None, build)
}

/// Build the 3-state expand-last layout for a single expression arrow with a
/// ternary body: `map((x) => (cond ? a : b))`. Flat keeps the parens; the break
/// states drop them and indent the body after `=>` (no trailing comma).
///
/// The ladder itself is [`build_ternary_arrow_hug_ladder`], shared with the `new` and
/// member-chain spellings of this same layout.
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

    // A forced break collapses the ladder to its break state alone — asked here of the BODY,
    // where the member-chain spelling asks it of the whole arrow.
    build_ternary_arrow_hug_ladder(d, ArgOpener::Callee(callee), sig_doc, body_doc, body_doc)
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
    // A spread's stripped parens can leave one *before* the argument's own end, where no
    // scan here can reach it — that share is asked for by name, of every argument.
    let has_spread_paren_comments =
        printer.any_spread_paren_comment_forces_expansion(call.arguments);
    let has_own_line_trailing_block = call.arguments.last().is_some_and(|last_arg| {
        let search_start = last_arg.span().end;
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
        || has_own_line_trailing_block
        || has_spread_paren_comments)
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
            let has_paren_line = gap_pc.has_trailing_comments();

            // `PartitionedComments::pulls_to_delimiter_line`, spelled with the flag this
            // builder independently needs (it must record the forced expansion even where
            // nothing sits on the `(` line).
            if force_expansion && has_paren_line {
                // Comments trailing the `(` stay on the `(` line, author blank included;
                // the own-line set then leads the first arg via the shared emitter (a block
                // hugging the arg stays inline, own-line/line comments break, author blanks
                // preserved).
                gap_pc.emit_delimiter_line_pull(&mut paren_line_prefix_parts, printer);
                gap_pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
            } else if !has_paren_line {
                // No comment on the `(` line → every gap comment leads the first
                // arg. Same shared emitter; it ends with the right separator before
                // the arg (space after a hug, hardline after an own-line comment).
                gap_pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
            } else if let Some(run) =
                printer.opener_trailing_broke_after_run(paren_open, first_arg_start)
            {
                // A block-only run glued to `(` that the author broke after: its
                // newline-after soft `line` rides the argument group — own line
                // when the list breaks (a breaking first argument, a sibling, or
                // width), glued bytes when the call collapses. The glue loop
                // below would weld the run to the argument in both renderings.
                printer.push_leading_run_with_soft_line(&mut arg_parts, &run);
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
        // build_arg_expression_doc). An own-line format-ignore directive in this
        // argument's gap freezes it verbatim (Rule A).
        arg_parts.push(build_joined_argument_doc(
            printer,
            paren_open,
            call.arguments,
            i,
        ));

        // Check for comments after this argument (before next arg or closing paren)
        if i < call.arguments.len() - 1 {
            let arg_end = arg.span().end;
            let next_arg_start = call.arguments[i + 1].span().start;

            // The gap after this argument, in the two regions that partition it: the
            // parent's share of a spread's stripped-paren interior and the ordinary
            // `[arg_end, next_arg_start)` scan. `open_inter_arg_gap` owns both — asking
            // only one of them (as an `if`/`else` over the two) drops the other.
            if printer.inter_arg_gap_has_comments(arg, next_arg_start) {
                if should_force_expansion_for_comments(printer, arg_end, next_arg_start) {
                    force_expansion = true;
                }

                // Open the gap (reclassify hugging blocks, emit before/after-comma
                // trailing comments + the comma, then the interior's own-line blocks);
                // the separator + leading comments below finish it.
                let gap = printer.open_inter_arg_gap(&mut arg_parts, arg, next_arg_start);
                let pc = gap.comments;
                if gap.forces_expansion {
                    force_expansion = true;
                }

                let has_blank_line = pc.has_blank_line_in_gap(printer);
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
            // Last argument - trailing comments before the closing paren, in two
            // regions that partition it (see `emit_last_arg_trailing_comments`, which
            // states the same split for the builders that need no force-expansion
            // feedback). First the parent's share of a spread's stripped-paren
            // interior: own-line comments the spread's own doc deliberately leaves
            // behind, each a sibling line the call cannot stay collapsed around —
            // deferred past the ordinary gap when it ends in a `//`
            // (`spread_share_ends_in_line_comment`, the ordering rule).
            // A same-line `//` the spread's own doc defers must flush INSIDE the call:
            // on a collapsed list the buffer drains past the `)` and the `;`,
            // re-binding the comment to the statement. Also feeds the demotion below.
            let arg_defers_line = printer.defers_trailing_line_comment(arg);
            if arg_defers_line {
                force_expansion = true;
            }
            let share_ends_in_line = printer.spread_share_ends_in_line_comment(arg);
            if !share_ends_in_line && printer.push_spread_own_line_comments(&mut arg_parts, arg) {
                force_expansion = true;
            }

            // Then the ordinary gap, on the argument's own end. Widening this anchor to
            // reach the interior claims the spread's share a second time — the same-line
            // blocks and interior `//`s print twice.
            let arg_end = arg.span().end;
            let paren_close = call.span.end;

            // The last-argument gap holds the list's own comma (`for_closer_gap`, never the
            // delimiter reading) — see `emit_last_arg_trailing_comments`, the shared path
            // this loop mirrors so it can feed `force_expansion`.
            let mut pc = PartitionedComments::for_closer_gap(printer, arg_end, paren_close);
            // The argument's own doc may already end in a deferred `//` (a spread whose
            // stripped parens held one); a second one may not join that line.
            pc.demote_trailing_line_after_deferred(arg_defers_line);

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

            // (2) Same-line line comment after the last arg, via `line_suffix`. At most
            // one — the trailing run ends at the first `//`
            // (`Printer::closer_trailing_comment_run`); anything past it is own-line and
            // sits in `pc.leading` below. No trailing comma precedes it
            // (trailingComma: 'none').
            if pc.has_trailing_line() {
                // Line comments always force the CALL to expand - the newline after the
                // comment means the call must break to multiple lines. A trailing line
                // comment never counts toward width (prettier's `lineSuffix`), so the
                // argument's own group (array/object, binary, conditional, …) can stay
                // inline even when the comment exceeds print_width; force_expansion
                // ensures the call expands.
                force_expansion = true;
                if let Some(comment) = pc.trailing_line {
                    arg_parts.push(printer.build_trailing_line_comment_doc(comment));
                }
            }

            // (3) Own-line comments (block or line) after the last arg, before the
            // closing paren — emitted each on its own line, with no trailing comma
            // (trailingComma: 'none').
            if !pc.leading.is_empty() {
                force_expansion = true;
                pc.emit_dangling_comments(&mut arg_parts, printer);
            }

            // The `//`-ending share, deferred past the ordinary gap (the ordering rule
            // above).
            if share_ends_in_line && printer.push_spread_own_line_comments(&mut arg_parts, arg) {
                force_expansion = true;
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
            d.group_break(d.concat(&[d.indent_hardline(arg_doc), d.hardline()])),
            d.text(")"),
        ]));
    }

    Some(wrap_call_with_soft_breaks(d, callee, arg_doc))
}
