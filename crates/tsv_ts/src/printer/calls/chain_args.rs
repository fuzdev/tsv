// Chain-specific argument building for call expressions
//
// Handles building call arguments in chain contexts where the callee
// is handled separately by the chain printer.

use super::super::comments::{CommentFilter, CommentSpacing};
use super::super::{Printer, has_newline_before_position, is_multiline_template_expression};
use super::arg_comments::{
    PartitionedComments, any_comment_forces_expansion, build_after_comma_leading_comments,
    build_before_comma_trailing_comments, first_arg_has_any_comments, has_blank_line_between_args,
    has_inter_argument_comments, has_trailing_comments_on_args, is_comment_inline_with_next,
    last_arg_has_comments,
};
use super::arg_predicates::{
    arrow_body_is_call_through_non_null, arrow_has_trailing_param_comments, is_block_function,
    is_concise_numeric_array, is_curried_arrow, is_function_composition_args,
    is_short_second_arg_for_expand_first, is_ternary_arrow_body, last_arg_is_array_or_object,
    preceding_args_allow_expand_last,
};
use super::arg_wrapping::{
    ChainArgKind, build_args_split_last, build_arrow_sig_doc, build_break_body_state,
    build_chain_expand_all_args, classify_chain_arg, last_two_args_same_type,
    prebuild_expand_last_break_body, prebuild_expand_last_obj_array_body,
    prepend_arrow_body_comments, wrap_args_with_soft_breaks, wrap_huggable_arg,
};
use crate::ast::internal::{self, Expression};
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};

/// Prepend optional leading comments to a doc.
#[inline]
fn prepend_leading(d: &DocArena, leading: Option<DocId>, doc: DocId) -> DocId {
    match leading {
        Some(lc) => d.concat(&[lc, doc]),
        None => doc,
    }
}

/// Get type arguments for a call expression, checking both the call itself
/// and a TSInstantiationExpression callee.
///
/// Our parser produces `CallExpression { callee: TSInstantiationExpression { expr, <T> } }`
/// for `expr<T>(args)`, while the canonical parser puts `<T>` directly on
/// `CallExpression.typeArguments`. In chain context, the TSInstantiationExpression
/// is linearized away, so the Call node must recover type arguments from the callee.
fn get_call_type_arguments<'a>(
    call: &'a internal::CallExpression<'a>,
) -> Option<&'a internal::TSTypeParameterInstantiation<'a>> {
    call.type_arguments.as_ref().or({
        if let Expression::TSInstantiationExpression(inst) = call.callee {
            Some(&inst.type_arguments)
        } else {
            None
        }
    })
}

/// Build inline leading block comments for the first argument (non-expansion path).
///
/// Returns comments that should be emitted inline before the first arg:
/// - Block comments on the same line as paren_open (trailing_block)
/// - Block comments on the same line as the first arg (from leading)
fn build_inline_leading_comments(
    printer: &Printer<'_>,
    paren_open: u32,
    arg_start: u32,
) -> Option<DocId> {
    let d = printer.d();
    let pc = PartitionedComments::new(printer.comments, printer.line_breaks, paren_open, arg_start);

    let mut parts = DocBuf::new();

    // Block comments on same line as paren
    for comment in &pc.trailing_block {
        parts.push(printer.build_comment_doc(comment));
        parts.push(d.text(" "));
    }

    // Block comments effectively inline with first arg
    for comment in &pc.leading {
        if comment.is_block && is_comment_inline_with_next(printer, comment.span.end, arg_start) {
            parts.push(printer.build_comment_doc(comment));
            parts.push(d.text(" "));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(d.concat(&parts))
    }
}

/// Build inline trailing block comments for an argument (non-expansion path).
///
/// Used for the last arg (comments before closing paren) and single-arg paths
/// where there's no comma to split around.
fn build_inline_trailing_comments(
    printer: &Printer<'_>,
    arg_end: u32,
    next_boundary: u32,
) -> Option<DocId> {
    let d = printer.d();
    let pc = PartitionedComments::new(
        printer.comments,
        printer.line_breaks,
        arg_end,
        next_boundary,
    );

    if !pc.has_trailing_block() {
        return None;
    }

    let mut parts = DocBuf::new();
    for comment in &pc.trailing_block {
        parts.push(d.text(" "));
        parts.push(printer.build_comment_doc(comment));
    }
    Some(d.concat(&parts))
}

/// Build a Doc for call arguments only (for chain printing)
///
/// Uses proper group wrapping so args can break independently from the chain.
/// This allows the chain's conditionalGroup to try:
/// 1. Everything inline
/// 2. Args broken but chain inline (if args are in their own group)
/// 3. Chain broken (if args broken still doesn't fit)
pub(super) fn build_call_args_doc_for_chain(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    optional: bool,
) -> DocId {
    build_call_args_doc_for_chain_impl(printer, call, optional, false, false)
}

/// Check if a single argument is an arrow function with a breakable body
/// (call expression or ternary).
///
/// These patterns use the arrow-hugging layout `(sig =>\n body,\n)` even when
/// the chain forces expansion, matching prettier's behavior.
/// Array bodies are excluded — they use the self-expanding layout `(sig => [\n items\n])`.
fn is_single_arrow_with_breakable_body(arg: &Expression<'_>) -> bool {
    if let Expression::ArrowFunctionExpression(arrow) = arg
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
    {
        return arrow_body_is_call_through_non_null(body_expr) || is_ternary_arrow_body(body_expr);
    }
    false
}

/// Build a Doc for call arguments with forced expansion (hardlines instead of softlines)
///
/// Used for the "args expanded, chain inline" state in conditionalGroup.
pub(super) fn build_call_args_doc_for_chain_expanded(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    optional: bool,
) -> DocId {
    build_call_args_doc_for_chain_impl(printer, call, optional, true, false)
}

/// Build a Doc for call arguments with standard forced expansion
///
/// Like `build_call_args_doc_for_chain_expanded`, but always uses the standard
/// `(\n  args,\n)` form — never the arrow-hugging `(sig =>\n  body,\n)` form.
/// Used for the "first call inline, rest expanded" state in short chains where
/// the chain doesn't break between groups, so the arrow signature would add
/// too much to the first line.
pub(super) fn build_call_args_doc_for_chain_standard_expanded(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    optional: bool,
) -> DocId {
    build_call_args_doc_for_chain_impl(printer, call, optional, true, true)
}

/// Shared per-call state computed once in `build_call_args_doc_for_chain_impl`'s
/// prologue and threaded into the `build_chain_args_*` branch builders — the
/// `(`/`?.(` prefix, the opening-paren position, the precomputed comment flags,
/// and the inline leading-comment doc. Built once, then moved into whichever
/// branch runs (mirrors `ClassMemberHeader` in the class parser).
#[derive(Clone, Copy)]
#[allow(clippy::struct_excessive_bools)] // independent prologue flags, not a state machine
struct ChainArgsContext {
    paren_open: u32,
    prefix: &'static str,
    has_leading_comments: bool,
    has_any_comments: bool,
    has_trailing_block_comments: bool,
    comments_force_expansion: bool,
    standard_expansion: bool,
    leading_comment_doc: Option<DocId>,
}

/// Implementation for call args doc building
fn build_call_args_doc_for_chain_impl(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    optional: bool,
    force_expand: bool,
    standard_expansion: bool,
) -> DocId {
    let d = printer.d();
    // Build type arguments if present: `<T, U>`
    let type_args = get_call_type_arguments(call);
    let type_args_doc = type_args.map(|ta| printer.build_type_parameter_instantiation_doc(ta));

    // Check for blank lines between arguments (forces expansion)
    // Uses has_blank_line_between_args to skip stripped grouping paren span gaps.
    let has_blank_lines = call.arguments.windows(2).any(|window| {
        has_blank_line_between_args(
            printer.source,
            printer.line_breaks,
            window[0].span().end,
            window[1].span().start,
        )
    });

    // Multiple arrow function arguments: always expand to multiple lines
    // Prettier always expands 2+ arrow function arguments, regardless of source formatting.
    // This matches Prettier's behavior: fn(() => x, () => y) → fn(\n  () => x,\n  () => y,\n)
    let all_args_are_arrows = call.arguments.len() >= 2
        && call
            .arguments
            .iter()
            .all(|arg| matches!(arg, Expression::ArrowFunctionExpression(_)));

    // Get paren_open position (after type args if present, otherwise after callee)
    let paren_open = type_args.map_or_else(|| call.callee.span().end, |ta| ta.span.end);

    // Check for any comments in arguments (leading, inter-argument, or trailing)
    // Note: presence of comments doesn't necessarily mean expansion - only line comments
    // and block comments on their own line force expansion.
    //
    // Whole-call comment-presence gate (one binary search over [paren_open,
    // call.span.end]): every sub-scan below lies within that window, so with no
    // comment they are all provably false — skip them. Canonical reference:
    // build_params_doc_with_comments.
    let call_has_comments = printer.has_comments_between(paren_open, call.span.end);
    let has_leading_comments = call_has_comments
        && !call.arguments.is_empty()
        && printer.has_comments_between(paren_open, call.arguments[0].span().start);
    let has_inter_arg_comments = call_has_comments && has_inter_argument_comments(call, printer);
    let has_trailing_comments = call_has_comments && has_trailing_comments_on_args(call, printer);
    // Also check for trailing block comments on last arg (for inline handling)
    let has_trailing_block_comments = call_has_comments
        && call
            .arguments
            .last()
            .is_some_and(|last| printer.has_comments_between(last.span().end, call.span.end));
    let has_any_comments = has_leading_comments
        || has_inter_arg_comments
        || has_trailing_comments
        || has_trailing_block_comments;

    // Build leading comment doc once for reuse in single-arg arrow paths
    // (e.g., /** @param {any} x */ before arrow function parameters)
    let leading_comment_doc = if has_leading_comments && !call.arguments.is_empty() {
        build_inline_leading_comments(printer, paren_open, call.arguments[0].span().start)
    } else {
        None
    };

    // Check if any comments require expansion (line comments or block comments on own line)
    // Inline block comments don't force expansion. `has_any_comments` is a superset —
    // forced expansion needs a comment to exist — so gate this scan on it.
    let comments_force_expansion =
        has_any_comments && any_comment_forces_expansion(call, printer, paren_open);

    // Function composition: call arg contains a callback → expand all args
    // e.g., x.y(arr.map((e) => e[0]), ['foo']) — matches Prettier's isFunctionCompositionArgs
    let force_expand = force_expand
        || has_blank_lines
        || comments_force_expansion
        || all_args_are_arrows
        || is_function_composition_args(call.arguments);

    // `?.` precedes explicit type arguments (`a.fn?.<T>(b)`), so it only fuses
    // with the paren when there are none
    let prefix = if optional && type_args.is_none() {
        "?.("
    } else {
        "("
    };

    let mut parts = DocBuf::new();
    if optional && type_args.is_some() {
        parts.push(d.text("?."));
    }
    // Emit comments between callee and type args: `obj.fn/* c */ <string>()`
    // Uses build_name_to_type_params_comments for safe line comment handling
    if let Some(ta) = type_args {
        let gap_start = call.callee.span().end;
        let gap_end = ta.span.start;
        if let Some(doc) = printer.build_name_to_type_params_comments_opt(
            gap_start,
            gap_end,
            CommentSpacing::Trailing,
        ) {
            parts.push(doc);
        }
    }
    if let Some(ta_doc) = type_args_doc {
        parts.push(ta_doc);
    }

    let ctx = ChainArgsContext {
        paren_open,
        prefix,
        has_leading_comments,
        has_any_comments,
        has_trailing_block_comments,
        comments_force_expansion,
        standard_expansion,
        leading_comment_doc,
    };

    if call.arguments.is_empty() {
        build_chain_args_empty(printer, call, ctx, parts)
    } else if force_expand {
        build_chain_args_force_expand(printer, call, ctx, parts)
    } else if call.arguments.len() == 1 {
        build_chain_args_single(printer, call, ctx, parts)
    } else {
        build_chain_args_multi(printer, call, ctx, parts)
    }
}

/// Empty argument list (`()` / `?.()` / `<T>()`), preserving dangling comments
/// between the callee/type-args and the `(` and inside the parens.
fn build_chain_args_empty(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    ctx: ChainArgsContext,
    mut parts: DocBuf,
) -> DocId {
    let d = printer.d();
    let ChainArgsContext {
        paren_open, prefix, ..
    } = ctx;
    // `prefix` is `"("` or `"?.("` (the prologue's two literals), so the closed
    // empty-args form is one of two statics — no transient `format!` String.
    let empty_pair: &'static str = if prefix == "?.(" { "?.()" } else { "()" };

    // Separate pre-paren comments (between > and () from inside-paren comments
    let paren_close = call.span.end;
    let actual_paren = printer.find_char_outside_comments(paren_open, paren_close, b'(');
    if let Some(paren_pos) = actual_paren {
        let pre_paren_comments = printer.build_comments_between_filtered_opt(
            paren_open,
            paren_pos,
            CommentSpacing::Leading,
            CommentFilter::All,
        );
        let inside_paren_comments = printer
            .build_inline_comments_between_doc_no_leading_space_opt(paren_pos + 1, paren_close);
        if let Some(pre) = pre_paren_comments {
            parts.push(pre);
        }
        match inside_paren_comments {
            Some(inner) => {
                parts.push(d.text(prefix));
                parts.push(inner);
                parts.push(d.text(")"));
            }
            None => parts.push(d.text(empty_pair)),
        }
    } else {
        parts.push(d.text(empty_pair));
    }
    d.concat(&parts)
}

/// Forced-expansion argument layout (`force_expand` true): hardlines instead of
/// softlines. Single object/array and single expression-arrow args get hugged
/// special-cases; everything else uses the blank-line- and comment-aware loop.
fn build_chain_args_force_expand(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    ctx: ChainArgsContext,
    mut parts: DocBuf,
) -> DocId {
    let d = printer.d();
    let ChainArgsContext {
        paren_open,
        prefix,
        has_leading_comments,
        has_trailing_block_comments,
        comments_force_expansion,
        standard_expansion,
        leading_comment_doc,
        ..
    } = ctx;

    // Special case: single object/array arg should hug the parens
    // and expand internally with hardlines, not softlines around it.
    // e.g., `.push({\n  ...\n})` not `.push(\n  {...},\n)`
    //
    // We use build_arg_expression_doc_expanded which produces hardlines,
    // allowing fits() to correctly measure the first line: `chain.call({`
    // and return true (since hardlines end the fits check).
    //
    // Exception: when there are trailing comments, we use the full expansion
    // path which produces the extra-indented style that Prettier uses:
    // `fn(\n  {...} /* comment */,\n)` not `fn({...} /* comment */)`
    if call.arguments.len() == 1 && !has_trailing_block_comments && !comments_force_expansion {
        let arg = &call.arguments[0];
        if matches!(
            arg,
            Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
        ) {
            // Build the object/array with forced internal expansion (hardlines)
            let arg_doc = printer.build_arg_expression_doc_expanded(arg);
            parts.push(d.text(prefix));
            parts.push(arg_doc);
            parts.push(d.text(")"));
            return d.concat(&parts);
        }
    }

    // Special case: single arrow arg with array body — the array expands internally.
    // Layout: `(sig => [\n  items,\n])` — array content on new lines, bracket hugged.
    // Skip when comments_force_expansion — own-line comments would be lost since
    // leading_comment_doc only captures inline block comments.
    if call.arguments.len() == 1
        && !comments_force_expansion
        && let Expression::ArrowFunctionExpression(arrow) = &call.arguments[0]
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        && matches!(&**body_expr, Expression::ArrayExpression(_))
    {
        let body_doc = printer.build_arg_expression_doc_expanded(body_expr);
        let body_doc =
            prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
        let sig_doc = build_arrow_sig_doc(printer, arrow);
        let sig_doc = prepend_leading(d, leading_comment_doc, sig_doc);

        parts.push(d.text(prefix));
        parts.push(sig_doc);
        parts.push(d.text(" => "));
        parts.push(body_doc);
        parts.push(d.text(")"));
        return d.concat(&parts);
    }

    // Special case: single arrow arg with breakable body (call, ternary).
    // Use the arrow-hugging break state directly: `(sig =>\n  body,\n)`
    // This matches prettier which keeps the signature hugged even when forcing expansion.
    // couldExpandArg keys on the body type (looking through the return-type annotation),
    // so typed-return arrows hug too.
    // Skip when standard_expansion is requested — short chains where the chain
    // doesn't break between groups need the standard `(\n  args,\n)` form to keep
    // the first line short enough for fits().
    if !standard_expansion
        && !comments_force_expansion
        && call.arguments.len() == 1
        && is_single_arrow_with_breakable_body(&call.arguments[0])
    {
        let arg = &call.arguments[0];
        if let Expression::ArrowFunctionExpression(arrow) = arg
            && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        {
            let body_doc = printer.build_expression_doc(body_expr);
            let body_doc =
                prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
            let sig_doc = build_arrow_sig_doc(printer, arrow);
            let sig_doc = prepend_leading(d, leading_comment_doc, sig_doc);

            parts.push(d.text(prefix));
            parts.push(sig_doc);
            parts.push(d.text(" =>"));
            parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
            parts.push(d.hardline());
            parts.push(d.text(")"));
            return d.concat(&parts);
        }
    }

    // Forced expansion: use hardlines instead of softlines
    // Build arguments with blank line preservation and full comment handling
    let mut arg_parts = DocBuf::new();
    // Comments trailing the `(` on its own line, kept on the `(` line
    // (divergence from prettier, which relocates them to their own line).
    // Injected after the `(` in the wrap below.
    let mut paren_line_prefix_parts: DocBuf = DocBuf::new();

    for (i, arg) in call.arguments.iter().enumerate() {
        let arg_start = arg.span().start;

        // Handle leading comments before first argument
        if i == 0 && has_leading_comments {
            let first_pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                paren_open,
                arg_start,
            );

            let has_paren_line =
                !first_pc.trailing_block.is_empty() || !first_pc.trailing_line.is_empty();

            if has_paren_line {
                // Comments trailing the `(` stay on the `(` line; the own-line set
                // then leads the first arg (source order preserved — see
                // conformance_prettier.md §Comment relocation, Call open paren `(`).
                first_pc.emit_trailing_comments(&mut paren_line_prefix_parts, printer);
            }
            // The own-line leading comments (everything not on the `(` line): a block
            // hugging the arg stays inline (`/* b */ a`), an own-line block / line
            // comment takes its own line with author blanks preserved. The shared
            // emitter's backward-walk keeps a non-hugging own-line block on its own
            // line even when a later block hugs the arg.
            first_pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
        }

        // Check for blank line before this arg (from previous arg)
        // Only add blank line preservation when there are no comments between args,
        // since comments with blank lines are handled in the separator logic below.
        if i > 0 {
            let prev_end = call.arguments[i - 1].span().end;
            let has_comments_before = printer.has_comments_between(prev_end, arg_start);
            if !has_comments_before
                && has_blank_line_between_args(
                    printer.source,
                    printer.line_breaks,
                    prev_end,
                    arg_start,
                )
            {
                arg_parts.push(d.literalline());
                arg_parts.push(d.hardline());
            }
        }

        arg_parts.push(printer.build_arg_expression_doc(arg));

        // Handle trailing comments and comma placement
        let arg_end = arg.span().end;
        let next_boundary = if i < call.arguments.len() - 1 {
            call.arguments[i + 1].span().start
        } else {
            call.span.end
        };

        if i < call.arguments.len() - 1 {
            // Not the last argument
            let next_arg_start = call.arguments[i + 1].span().start;

            // Reclassify a hugging after-comma block as leading, emit the
            // before/after-comma trailing comments + comma; the separator + leading
            // comments below finish the gap.
            let pc = printer.open_inter_arg_gap(&mut arg_parts, arg_end, next_arg_start);

            // Skip hardline if next arg has blank line
            // (blank line preservation at the top of the loop handles the line break)
            let has_comments_before_next = printer.has_comments_between(arg_end, next_arg_start);
            let next_has_blank = if has_comments_before_next {
                pc.has_blank_line_in_gap(printer.source, printer.line_breaks)
            } else {
                has_blank_line_between_args(
                    printer.source,
                    printer.line_breaks,
                    arg_end,
                    next_arg_start,
                )
            };
            if next_has_blank && has_comments_before_next {
                // Blank line before next arg's leading comments — emit literalline
                // before the hardline separator. When there are no comments, the
                // blank line is handled at the top of the next iteration.
                arg_parts.push(d.literalline());
                arg_parts.push(d.hardline());
            } else if !next_has_blank {
                arg_parts.push(d.hardline());
            }
            // else: next_has_blank && !has_comments_before_next — skip hardline,
            // blank line preservation at top of next iteration adds literalline + hardline
            pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
        } else {
            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                arg_end,
                next_boundary,
            );
            // Last argument - same-line trailing comments trail the arg in source order
            // (a block that sat after the source comma just trails past where the comma
            // was; a line comment follows via `line_suffix`), then own-line dangling
            // comments. No trailing comma (trailingComma: 'none').
            pc.emit_last_arg_comments(&mut arg_parts, printer);
        }
    }

    parts.push(d.text(prefix));
    parts.push(d.concat(&paren_line_prefix_parts));
    // No trailing comma after the last arg (trailingComma: 'none') — the last-arg
    // comment emit trails same-line comments after the arg and emits no comma, so
    // nothing is appended here.
    parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&arg_parts)])));
    parts.push(d.hardline());
    parts.push(d.text(")"));
    d.concat(&parts)
}

/// Single non-force-expand argument: arrow special-cases (call/ternary/object/array
/// expression bodies, block arrows, multiline templates) and the general
/// classify-and-wrap path. Always returns.
fn build_chain_args_single(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    ctx: ChainArgsContext,
    mut parts: DocBuf,
) -> DocId {
    let d = printer.d();
    let ChainArgsContext {
        paren_open,
        prefix,
        has_leading_comments,
        has_any_comments,
        leading_comment_doc,
        ..
    } = ctx;

    let arg = &call.arguments[0];

    // Special case: arrow function with call expression body
    // Prettier keeps `(sig =>` hugged, breaking after `=>` to the body.
    // Structure: `(sig =>\n  body\n)` instead of `(\n  sig =>\n    body\n)`
    //
    // couldExpandArg keys on the body type and looks through the return-type
    // annotation plus a trailing non-null `!` (its `stripChainElementWrappers`),
    // so typed-return and `=> call()!` arrows are call-body arrows too.
    //
    // Leading comments on the arg block expand-last (prettier's shouldExpandLastArg
    // returns false when hasComment(lastArg, Leading)).
    if let Expression::ArrowFunctionExpression(arrow) = arg
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        && arrow_body_is_call_through_non_null(body_expr)
        && !(has_any_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
    {
        let arrow_doc = printer.build_arg_expression_doc(arg);
        let arrow_doc = prepend_leading(d, leading_comment_doc, arrow_doc);
        let body_doc = printer.build_expression_doc(body_expr);
        let body_doc =
            prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
        let sig_doc = build_arrow_sig_doc(printer, arrow);
        let sig_doc = prepend_leading(d, leading_comment_doc, sig_doc);

        // State 1: sig hugged, body indented — (sig =>\n  body\n)
        let break_state = d.concat(&[
            d.text(prefix),
            sig_doc,
            d.text(" =>"),
            d.indent(d.concat(&[d.hardline(), body_doc])),
            d.hardline(),
            d.text(")"),
        ]);

        // State 2: all args broken out — (\n  sig => body,\n)
        // Matches prettier's allArgsBrokenOut(): a group with shouldBreak
        // that puts a line right after "(" so fits() returns true early
        // when evaluated in Break mode during look-ahead.
        let all_broken_state = d.group_break(d.concat(&[
            d.text(prefix),
            d.indent(d.concat(&[d.line(), arrow_doc])),
            d.line(),
            d.text(")"),
        ]));

        // If body will break (multiline content), use break state directly
        // so the hugged-signature layout is preserved when content is multiline
        if d.will_break(body_doc) {
            parts.push(break_state);
        } else {
            parts.push(d.conditional_group(&[
                // State 0: flat — (arrow)
                d.concat(&[d.text(prefix), arrow_doc, d.text(")")]),
                // State 1: body breaks
                break_state,
                // State 2: all broken out
                all_broken_state,
            ]));
        }
        return d.concat(&parts);
    }

    // Special case: arrow function with ternary body
    // Prettier uses conditional parens:
    // - Flat: `map((x) => (x ? y : z))` - with parens
    // - Break: `map((x) =>\n  x ? y : z)` - no parens, body indented
    // couldExpandArg keys on the body type (looking through the return-type
    // annotation), so typed-return arrows are eligible.
    // Leading comments block expand-last (prettier's shouldExpandLastArg).
    if let Expression::ArrowFunctionExpression(arrow) = arg
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        && is_ternary_arrow_body(body_expr)
        && !(has_any_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
    {
        let arrow_doc = printer.build_arg_expression_doc(arg);
        let arrow_doc = prepend_leading(d, leading_comment_doc, arrow_doc);
        let body_doc = printer.build_expression_doc(body_expr);
        let body_doc =
            prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
        let sig_doc = build_arrow_sig_doc(printer, arrow);
        let sig_doc = prepend_leading(d, leading_comment_doc, sig_doc);

        // State 0: Flat - with parens around ternary
        let state_flat = d.concat(&[
            d.text(prefix),
            sig_doc,
            d.text(" => ("),
            body_doc,
            d.text("))"),
        ]);

        // State 1: Break - no parens, body indented
        let state_break = d.concat(&[
            d.text(prefix),
            sig_doc,
            d.text(" =>"),
            d.indent(d.concat(&[d.hardline(), body_doc])),
            d.hardline(),
            d.text(")"),
        ]);

        // State 2: All broken - signature and body both indented
        let state_all_broken = d.concat(&[
            d.text(prefix),
            d.indent(d.concat(&[
                d.hardline(),
                sig_doc,
                d.text(" =>"),
                d.indent(d.concat(&[d.hardline(), body_doc])),
            ])),
            d.hardline(),
            d.text(")"),
        ]);

        // If arrow is already flat (no breaking content), try all states
        // If it has breaking content, use state_break directly
        if d.will_break(arrow_doc) {
            parts.push(state_break);
        } else {
            parts.push(d.conditional_group(&[state_flat, state_break, state_all_broken]));
        }
        return d.concat(&parts);
    }

    // Special case: arrow function with object/array expression body
    // Prettier's shouldExpandLastArg path: produces a 3-state conditional_group
    // so fluid assignments can expand call args instead of breaking after =.
    // couldExpandArg keys on the body type (looking through the return-type
    // annotation), so typed-return arrows are eligible.
    // Leading comments block expand-last (prettier's shouldExpandLastArg).
    // See also: call_formatting.rs's parallel non-chain implementation.
    if let Expression::ArrowFunctionExpression(arrow) = arg
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        && matches!(
            &**body_expr,
            Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
        )
        && !(has_any_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
    {
        // Render the arrow with flat params (prettier's expandLastArg
        // `removeLines`) so the force-broken state breaks the body, not the
        // destructuring param — letting it fall through to all-args-broken-out.
        printer.expand_last_arg_flat_params.set(true);
        let arrow_doc = printer.build_arg_expression_doc(arg);
        printer.expand_last_arg_flat_params.set(false);
        let arrow_doc = prepend_leading(d, leading_comment_doc, arrow_doc);

        // State 0: hugged flat — (arrow_doc)
        let state_hug = d.concat(&[d.text(prefix), arrow_doc, d.text(")")]);

        // State 1: arrow forced to break — (group_break(arrow_doc))
        let state_arrow_break = d.concat(&[d.text(prefix), d.group_break(arrow_doc), d.text(")")]);

        // State 2: all args broken out — (\n  arrow_doc,\n)
        let state_all_broken = d.group_break(d.concat(&[
            d.text(prefix),
            d.indent(d.concat(&[d.line(), arrow_doc])),
            d.line(),
            d.text(")"),
        ]));

        parts.push(d.conditional_group(&[state_hug, state_arrow_break, state_all_broken]));
        return d.concat(&parts);
    }

    // Build arg doc, wrapping certain expressions in isolated_group to prevent
    // internal breaks from propagating to parent groups (enables call hugging).
    // For curried arrows (body is another arrow), skip chain detection so the
    // outer arrow hugs its body — matches prettier's expandLastArg behavior.
    let curried = is_curried_arrow(arg);
    if curried {
        printer.skip_arrow_chain.set(true);
    }
    let arg_doc = printer.build_huggable_expression_doc(arg);
    if curried {
        printer.skip_arrow_chain.set(false);
    }
    let arg_start = arg.span().start;
    let arg_end = arg.span().end;

    // Check for leading inline block comments before the arg
    let leading_comments_doc = if has_leading_comments {
        build_inline_leading_comments(printer, paren_open, arg_start)
    } else {
        None
    };

    // Check for trailing inline block comments (don't force expansion)
    let trailing_comments_doc = if has_any_comments {
        build_inline_trailing_comments(printer, arg_end, call.span.end)
    } else {
        None
    };

    // Build combined arg doc with leading/trailing comments
    let arg_with_comments = match (leading_comments_doc, trailing_comments_doc) {
        (Some(leading), Some(trailing)) => d.concat(&[leading, arg_doc, trailing]),
        (Some(leading), None) => d.concat(&[leading, arg_doc]),
        (None, Some(trailing)) => d.concat(&[arg_doc, trailing]),
        (None, None) => arg_doc,
    };

    // Check if it's a block arrow with trailing param comments
    // These need soft-break wrapping to expand the call
    let block_arrow_has_trailing_param_comments = if let Expression::ArrowFunctionExpression(arrow) =
        arg
        && !arrow.body.is_expression()
    {
        let arrow_token = printer.find_arrow_token_for(arrow);
        arrow_has_trailing_param_comments(arrow, arrow_token, |start, end| {
            printer.has_comments_between(start, end)
        })
    } else {
        false
    };

    if block_arrow_has_trailing_param_comments {
        // Block arrow with trailing param comments - force expansion
        parts.push(wrap_args_with_soft_breaks(d, prefix, arg_with_comments));
        return d.concat(&parts);
    }

    // Single multiline template literal on its own line — preserve expanded form.
    // Mirrors Prettier's isTemplateOnItsOwnLine: walks backwards from the
    // template backtick to check if the author placed it on a new line.
    let template_on_own_line = is_multiline_template_expression(arg)
        && has_newline_before_position(printer.source, arg_start);

    if template_on_own_line {
        let arg_doc = printer.build_expression_doc(arg);
        parts.push(d.text(prefix));
        parts.push(d.indent(d.concat(&[d.hardline(), arg_doc])));
        parts.push(d.hardline());
        parts.push(d.text(")"));
        return d.concat(&parts);
    }

    // Multiline template literal on same line as ( — hug it.
    // Mirrors call_formatting.rs's isTemplateOnItsOwnLine handling:
    // when the template starts on the same line as the opening paren,
    // prettier hugs it (no break between `(` and the backtick).
    if is_multiline_template_expression(arg) {
        parts.push(d.text(prefix));
        parts.push(arg_with_comments);
        parts.push(d.text(")"));
        return d.concat(&parts);
    }

    // Block-body arrows: use conditional_group to try hug first, then expand.
    // Cannot use wrap_args_with_soft_breaks (regular group) because will_break()
    // recurses into the block body's hardlines and forces break without trying
    // fits(). conditional_group uses fits() directly, correctly measuring whether
    // the hugged first line (e.g., `fn((params) => {`) fits.
    //
    // Exception: when the arg has leading comments, force expansion.
    // Prettier's shouldExpandLastArg returns false for args with leading comments,
    // and the default path forces expansion via shouldBreak: printedArguments.some(willBreak).
    if let Expression::ArrowFunctionExpression(arrow) = arg
        && matches!(arrow.body, internal::ArrowFunctionBody::BlockStatement(_))
    {
        let state_expand = d.concat(&[
            d.text(prefix),
            d.indent(d.concat(&[d.hardline(), arg_with_comments])),
            d.hardline(),
            d.text(")"),
        ]);
        if has_leading_comments {
            // Leading comments prevent hugging — force expansion
            parts.push(state_expand);
        } else {
            // No leading comments — try hug first, then expand
            let state_hug = d.concat(&[d.text(prefix), arg_with_comments, d.text(")")]);
            parts.push(d.conditional_group(&[state_hug, state_expand]));
        }
        return d.concat(&parts);
    }

    // Leading comments prevent hugging — prettier's shouldExpandLastArg
    // returns false when hasComment(lastArg, Leading), so the default
    // expansion path is used instead of expand-last hugging.
    let kind = if has_leading_comments {
        ChainArgKind::NeedsSoftWrap
    } else {
        classify_chain_arg(arg)
    };
    match kind {
        ChainArgKind::NeedsSoftWrap => {
            // Needs soft-break wrapping - e.g., long strings
            parts.push(wrap_args_with_soft_breaks(d, prefix, arg_with_comments));
        }
        ChainArgKind::NeedsWrapper => {
            // Huggable with internal break points (ternary, etc.)
            // Hugs opening paren; breaks the closing paren onto its own line
            // when content breaks (no trailing comma; trailingComma: 'none').
            parts.push(wrap_huggable_arg(d, prefix, arg_with_comments));
        }
        ChainArgKind::HugsNaturally => {
            // Objects/arrays/blocks that hug naturally
            parts.push(d.text(prefix));
            parts.push(arg_with_comments);
            parts.push(d.text(")"));
        }
    }
    d.concat(&parts)
}

/// Multiple non-force-expand arguments: the expand-last/expand-first strategy
/// trees (block-function last, expression-arrow last, expand-first, array/object
/// last) and the default soft-break-wrapped argument list. Always returns.
fn build_chain_args_multi(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    ctx: ChainArgsContext,
    mut parts: DocBuf,
) -> DocId {
    let d = printer.d();
    let ChainArgsContext {
        paren_open,
        prefix,
        has_any_comments,
        comments_force_expansion,
        ..
    } = ctx;

    // Multiple arguments with block-body callback:
    // Use conditional_group to try inline first, then expand-all.
    // fits() checks actual width, handling both short and non-short preceding args.
    //
    // IMPORTANT: Cannot use wrap_args_with_soft_breaks (regular group) because
    // will_break() recurses into the block body's hardlines and forces break
    // without trying fits(). conditional_group uses fits() directly.
    if call.arguments.len() >= 2
        && call.arguments.last().is_some_and(is_block_function)
        && preceding_args_allow_expand_last(call.arguments, printer.line_breaks)
        && !comments_force_expansion
        && !(has_any_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
    {
        let (head_parts, last_arg_doc, all_args_broken) =
            build_args_split_last(call.arguments, printer, paren_open, has_any_comments);

        // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
        if head_parts.iter().any(|&id| d.will_break(id)) {
            parts.push(build_chain_expand_all_args(d, prefix, all_args_broken));
            return d.concat(&parts);
        }

        let state_inline = d.concat(&[
            d.text(prefix),
            d.concat(&head_parts),
            last_arg_doc,
            d.text(")"),
        ]);
        let state_expand_all = build_chain_expand_all_args(d, prefix, all_args_broken);

        parts.push(d.conditional_group(&[state_inline, state_expand_all]));
        return d.concat(&parts);
    }

    // Expression arrow with call/conditional expression body
    // Prettier keeps preceding args inline and breaks after =>
    // e.g., `a.b(c, (x) =>\n  fn(x, ...),\n);`
    // couldExpandArg keys only on the body type — param/return type annotations
    // don't disable the hug, so a typed arrow expands the same way (its full
    // signature is emitted via build_arrow_sig_doc).
    if call.arguments.len() >= 2
        && preceding_args_allow_expand_last(call.arguments, printer.line_breaks)
        && !comments_force_expansion
        && !(has_any_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
        && let Some(Expression::ArrowFunctionExpression(arrow)) = call.arguments.last()
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        && matches!(
            &**body_expr,
            Expression::CallExpression(_) | Expression::ConditionalExpression(_)
        )
    {
        // Expand-last arrow with a call body: build the body ONCE and inject it so the
        // whole-arrow arg doc reuses it (the break-body state below reuses it too) —
        // building it in both places recurses into itself → O(2^depth).
        let body_reuse =
            prebuild_expand_last_break_body(printer, call.arguments.last(), has_any_comments);
        let inject_prev = body_reuse.map(|(span, doc)| printer.inject_arrow_body(span, doc));

        let (head_parts, last_arg_doc, all_args_broken) =
            build_args_split_last(call.arguments, printer, paren_open, has_any_comments);

        if let Some(prev) = inject_prev {
            printer.restore_arrow_body_inject(prev);
        }

        // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
        if head_parts.iter().any(|&id| d.will_break(id)) {
            parts.push(build_chain_expand_all_args(d, prefix, all_args_broken));
            return d.concat(&parts);
        }

        let sig_doc = build_arrow_sig_doc(printer, arrow);
        // Reuse the pre-built call body (see above); conditional bodies build fresh.
        let body_doc =
            body_reuse.map_or_else(|| printer.build_expression_doc(body_expr), |(_, doc)| doc);
        let body_doc =
            prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);

        // State 1: hug - head inline, arrow body breaks after =>
        let prefix_doc = d.text(prefix);
        let state_break_body =
            build_break_body_state(d, prefix_doc, &head_parts, sig_doc, body_doc);

        // State 2: expand all args
        let state_expand_all = build_chain_expand_all_args(d, prefix, all_args_broken);

        // Prettier: when willBreak(lastArg) is true, skip flat state.
        // The flat state would be selected by fits() but produces wrong
        // closing brackets (e.g., `}));` instead of `}),\n)`).
        if d.will_break(last_arg_doc) {
            parts.push(d.conditional_group(&[state_break_body, state_expand_all]));
            return d.concat(&parts);
        }

        // State 0: all inline
        let state_inline = d.concat(&[
            d.text(prefix),
            d.concat(&head_parts),
            last_arg_doc,
            d.text(")"),
        ]);

        parts.push(d.conditional_group(&[state_inline, state_break_body, state_expand_all]));
        return d.concat(&parts);
    }

    // Expression arrow with object/array body
    // Prettier keeps preceding args inline and expands object/array internally
    // e.g., `a.b(c, (x) => ({\n  y: x,\n}));`
    // couldExpandArg keys only on the body type — a typed arrow expands the same
    // way (its full signature is emitted via build_arrow_sig_doc).
    if call.arguments.len() >= 2
        && preceding_args_allow_expand_last(call.arguments, printer.line_breaks)
        && !comments_force_expansion
        && !(has_any_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
        && let Some(Expression::ArrowFunctionExpression(arrow)) = call.arguments.last()
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        && matches!(
            &**body_expr,
            Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
        )
    {
        // Expand-last arrow with an object/array body: build the body ONCE and inject it so
        // the whole-arrow arg doc reuses it (the hug state below reuses it too) — building it
        // in both places recurses into itself → O(2^depth).
        let obj_reuse =
            prebuild_expand_last_obj_array_body(printer, call.arguments.last(), has_any_comments);
        let inject_prev =
            obj_reuse.map(|(span, inject_doc, _)| printer.inject_arrow_body(span, inject_doc));

        let (head_parts, last_arg_doc, all_args_broken) =
            build_args_split_last(call.arguments, printer, paren_open, has_any_comments);

        if let Some(prev) = inject_prev {
            printer.restore_arrow_body_inject(prev);
        }

        // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
        if head_parts.iter().any(|&id| d.will_break(id)) {
            parts.push(build_chain_expand_all_args(d, prefix, all_args_broken));
            return d.concat(&parts);
        }

        let sig_doc = build_arrow_sig_doc(printer, arrow);
        // Reuse the pre-built object/array body (see above); `(x) => ({ ... })` parens included.
        let body_doc = obj_reuse.map_or_else(
            || d.parens(printer.build_expression_doc(body_expr)),
            |(_, _, hug)| hug,
        );
        let body_doc =
            prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);

        // State 0: all inline
        let state_inline = d.concat(&[
            d.text(prefix),
            d.concat(&head_parts),
            last_arg_doc,
            d.text(")"),
        ]);

        // State 1: hug - head inline, object/array expands internally
        let state_hug = d.concat(&[
            d.text(prefix),
            d.concat(&head_parts),
            sig_doc,
            d.text(" => "),
            d.group_break(body_doc),
            d.text(")"),
        ]);

        // State 2: expand all args
        let state_expand_all = build_chain_expand_all_args(d, prefix, all_args_broken);

        parts.push(d.conditional_group(&[state_inline, state_hug, state_expand_all]));
        return d.concat(&parts);
    }

    // "Expand first arg" pattern: first arg is block function, rest are short
    // e.g., `.reduce((acc, item) => { ... }, {})` - callback hugs, tail args stay inline
    // Matches prettier's shouldExpandFirstArg behavior
    // NOTE: Must come before expand-last-array/object to match Prettier's ordering —
    // shouldExpandFirstArg is checked before shouldExpandLastArg for arrays/objects.
    if call.arguments.len() == 2
        && is_block_function(&call.arguments[0])
        && !comments_force_expansion
        && !(has_any_comments && first_arg_has_any_comments(call.arguments, printer, paren_open))
        // Prettier's shouldExpandFirstArg checks !couldExpandArg(secondArg).
        // couldExpandArg returns true for a bare object/array with a leading comment
        // (hasComment(node)), so prettier breaks all args; tsv matches by blocking
        // expand-first. A cast-wrapped collection is NOT blocked — prettier expand-firsts
        // it and the inter-arg comment is carried inline below.
        && !(matches!(
            &call.arguments[1],
            Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
        ) && printer.has_comments_between(
            call.arguments[0].span().end,
            call.arguments[1].span().start,
        ))
        && is_short_second_arg_for_expand_first(&call.arguments[1], |start, end| {
            printer.has_comments_between(start, end)
        })
    {
        // First arg (callback) expands, tail args stay inline
        let first_arg_doc = printer.build_arg_expression_doc(&call.arguments[0]);
        let second_arg_doc = printer.build_arg_expression_doc(&call.arguments[1]);

        // Inter-arg comment handling (e.g., `a.b((x) => { ... }, /** @type {T} */ c)`)
        let first_end = call.arguments[0].span().end;
        let second_start = call.arguments[1].span().start;
        let inter_leading = build_after_comma_leading_comments(printer, first_end, second_start);
        let inter_trailing = build_before_comma_trailing_comments(printer, first_end, second_start);

        // Prettier: if (tailArgs.some(willBreak)) return allArgsBrokenOut()
        if d.will_break(second_arg_doc) {
            let mut all_parts: DocBuf = smallvec![first_arg_doc];
            if let Some(t) = inter_trailing {
                all_parts.push(t);
            }
            all_parts.push(d.comma_hardline());
            if let Some(l) = inter_leading {
                all_parts.push(l);
            }
            all_parts.push(second_arg_doc);
            parts.push(d.text(prefix));
            parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&all_parts)])));
            parts.push(d.hardline());
            parts.push(d.text(")"));
            return d.concat(&parts);
        }

        parts.push(d.text(prefix));
        parts.push(first_arg_doc);
        if let Some(t) = inter_trailing {
            parts.push(t);
        }
        parts.push(d.text(", "));
        if let Some(l) = inter_leading {
            parts.push(l);
        }
        parts.push(second_arg_doc);
        parts.push(d.text(")"));
        return d.concat(&parts);
    }

    // "Expand last arg" pattern for arrays/objects:
    // Keep preceding args inline, only expand the last array/object arg.
    // e.g., `assert.deepEqual(parse('/foo'), [{...}, {...}])` keeps parse('/foo') inline
    // Matches prettier's shouldExpandLastArg for array/object arguments.
    //
    // Skip when last two args have the same outer type - use expand-all instead.
    if call.arguments.len() >= 2
        && last_arg_is_array_or_object(call.arguments)
        && !call.arguments.last().is_some_and(is_concise_numeric_array)
        && preceding_args_allow_expand_last(call.arguments, printer.line_breaks)
        && !comments_force_expansion
        && !(has_any_comments
            && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open))
        // Prettier blocks expand-last for 2-arg arrow+array (React hook pattern)
        && !(call.arguments.len() == 2
            && matches!(
                call.arguments.first(),
                Some(Expression::ArrowFunctionExpression(_))
            )
            && matches!(
                call.arguments.last(),
                Some(Expression::ArrayExpression(_))
            ))
        && !last_two_args_same_type(call.arguments)
    {
        let (head_parts, last_arg_doc, all_args_broken) =
            build_args_split_last(call.arguments, printer, paren_open, has_any_comments);

        // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
        if head_parts.iter().any(|&id| d.will_break(id)) {
            parts.push(build_chain_expand_all_args(d, prefix, all_args_broken));
            return d.concat(&parts);
        }

        // State 0: inline - all args on one line
        let state_inline = d.concat(&[
            d.text(prefix),
            d.concat(&head_parts),
            last_arg_doc,
            d.text(")"),
        ]);

        // State 1: hug - head inline, last expands with group_break
        // group_break forces the array/object to break internally
        let state_hug = d.concat(&[
            d.text(prefix),
            d.concat(&head_parts),
            d.group_break(last_arg_doc),
            d.text(")"),
        ]);

        // State 2: expand all - all args on separate lines
        let state_expand_all = build_chain_expand_all_args(d, prefix, all_args_broken);

        parts.push(d.conditional_group(&[state_inline, state_hug, state_expand_all]));
        return d.concat(&parts);
    }

    // Multiple arguments: wrap in group with softlines so they can break. Each gap's
    // after-comma block comment follows the respect-the-newline rule — hugging the next
    // arg → leads it (`C`); stranded on the comma line → stays there (`A`) — via the same
    // shared emit_* helpers the force-expanded paths use. A comment-free gap takes the
    // cheap `comma_line()` separator (no per-gap comment scan).
    let mut arg_parts = DocBuf::new();
    for (i, arg) in call.arguments.iter().enumerate() {
        let arg_start = arg.span().start;
        let arg_end = arg.span().end;
        let is_first = i == 0;
        let is_last = i == call.arguments.len() - 1;

        // Leading inline block comments before the first arg (paren → arg gap).
        if is_first
            && has_any_comments
            && let Some(l) = build_inline_leading_comments(printer, paren_open, arg_start)
        {
            arg_parts.push(l);
        }

        arg_parts.push(printer.build_arg_expression_doc(arg));

        if is_last {
            // Trailing inline block comments after the last arg (before `)`).
            if has_any_comments
                && let Some(t) = build_inline_trailing_comments(printer, arg_end, call.span.end)
            {
                arg_parts.push(t);
            }
        } else {
            let next_arg_start = call.arguments[i + 1].span().start;
            if has_any_comments && printer.has_comments_between(arg_end, next_arg_start) {
                let mut pc = PartitionedComments::new(
                    printer.comments,
                    printer.line_breaks,
                    arg_end,
                    next_arg_start,
                );
                pc.route_after_comma_hugging_to_leading(printer);
                // before-comma blocks trail the arg, the comma, stranded after-comma
                // blocks (`A`).
                pc.emit_trailing_comments_around_comma(&mut arg_parts, printer);
                arg_parts.push(d.line());
                // hugging after-comma + own-line comments lead the next arg (`C`).
                pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
            } else {
                arg_parts.push(d.comma_line());
            }
        }
    }
    parts.push(wrap_args_with_soft_breaks(d, prefix, d.concat(&arg_parts)));
    d.concat(&parts)
}
