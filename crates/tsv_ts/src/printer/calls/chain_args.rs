// Chain-specific argument building for call expressions
//
// Handles building call arguments in chain contexts where the callee
// is handled separately by the chain printer.
//
// Every state LADDER here is the plain-call / `new` one read through `ArgOpener` — the chain
// spelling differs only in what a state opens with (its own `(` / `?.(` rather than a callee
// doc), so `arg_wrapping.rs` owns the layouts and this file owns the strategy selection. What
// is still this file's alone is the selection ORDER and the arms the other trees have no twin
// for (the force-expanded hugs, the classify-and-wrap fallback).

use super::super::comments::CommentSpacing;
use super::super::{Printer, is_curried_arrow_chain, is_multiline_template_expression};
use super::arg_comments::{
    PartitionedComments, any_arg_empty_line, any_comment_forces_expansion, build_arg_gap_docs,
    emit_last_arg_trailing_comments, first_arg_has_any_comments, has_inter_argument_comments,
    has_trailing_comments_on_args, last_arg_has_comments, push_empty_args,
};
use super::arg_predicates::{
    arrow_body_is_call_through_non_null, is_block_function, is_concise_numeric_array,
    is_function_composition_args, is_ternary_arrow_body, last_arg_is_array_or_object,
};
use super::arg_wrapping::{
    ArgOpener, ChainArgKind, arrow_body_expands_internally, arrow_body_tail_has_comments,
    arrow_hug_refused_by_comments, build_args_split_last, build_arrow_gap_break_multi_arg_doc,
    build_arrow_gap_break_single_arg_doc, build_arrow_sig_doc, build_break_body_ladder,
    build_expand_first_arg_doc, build_expand_last_obj_array_doc, build_flat_params_arg_doc,
    build_joined_argument_doc, build_printed_argument_doc, build_single_arrow_hug_doc,
    build_ternary_arrow_hug_ladder, classify_chain_arg, first_arg_signature_refuses_expand_first,
    last_arg_arrow_gap_break, last_two_args_same_type, prebuild_expand_last_break_body,
    prebuild_expand_last_obj_array_body, prepend_arrow_body_comments, should_expand_first_arg,
    try_hook_deps_args_doc,
};
use crate::ast::internal::{self, Expression};
use crate::printer::expressions::functions::{
    arrow_signature_has_breaking_comments, callback_signature_has_breaking_comments,
    function_signature_has_breaking_comments, prepend_leading,
};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::has_newline_before_position;

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

/// The `(`→first-argument gap for the chain's **body-line** paths, as a doc: the blocks the
/// author left on the `(`'s line, then the rest of the gap through the shared leading
/// emitter ([`PartitionedComments::emit_leading_comments_inline_aware`]).
///
/// The force-expanded builder takes it too, for the gap its own delimiter-line pull declined
/// ([`PartitionedComments::pulls_to_delimiter_line`]) — the whole gap leads the argument
/// there, which is exactly this doc.
///
/// ⚠️ **Emitting a FILTERED subset here is a DROP waiting for its gate to move.** The loop
/// this replaces kept only the comments an item-boundary predicate called inline with the
/// argument and discarded the rest, on the standing promise that anything else had already
/// forced the expanded path. The moment the expansion gate stopped calling a glued run given
/// its own line "own-line" — which is what prettier's own soft `line` says about it —
/// `obj.m1().m2(⏎/* c1 */ /* c2 */⏎a)` arrived here and BOTH comments vanished, with the
/// whole fixture suite and every audit green. A gap emitter that can only print PART of what
/// it is handed is a claim about its gate, and no gate stated that claim.
///
/// ⚠️ **It still cannot delegate to
/// [`super::arg_comments::emit_first_arg_leading_comments`]**, and the reason is
/// the shape rather than the rules: that emitter has a second output channel for the
/// `(`-**line** run (a `//` trailing the `(` stays on it), while this one returns a single
/// doc its callers splice into the argument body. Callers that can reach such a comment
/// place it themselves first (the body-line arms under `!comments_force_expansion`, the
/// force-expanded builder under a declined pull) and consume this doc for the rest — but
/// `build_call_args_doc_for_chain_impl` builds it EAGERLY, for arms that then never run, so
/// "no `(`-line comment reaches here" is false as an invariant on this function even though
/// it holds at every use. Asserting it fails on `calls/chain_open_paren_comment`, which is
/// how this note came to be measured rather than guessed.
fn build_inline_leading_comments(
    printer: &Printer<'_>,
    paren_open: u32,
    arg_start: u32,
) -> Option<DocId> {
    let d = printer.d();
    // A block-only run glued to `(` that the author broke after takes its
    // newline-after soft `line` — the same gate the non-chain seams ask
    // (`emit_first_arg_leading_comments`): own line when the argument layout
    // breaks, glued bytes when it collapses. An own-line-authored run declines
    // and keeps the emitters below.
    if let Some(run) = printer.opener_trailing_broke_after_run(paren_open, arg_start) {
        let mut parts = DocBuf::new();
        printer.push_leading_run_with_soft_line(&mut parts, &run);
        return Some(d.concat(&parts));
    }
    let pc = PartitionedComments::new(
        printer.comments,
        printer.comment_line_breaks,
        paren_open,
        arg_start,
    );

    let mut parts = DocBuf::new();
    for comment in &pc.trailing_block {
        parts.push(printer.build_comment_doc(comment));
        parts.push(d.text(" "));
    }
    pc.emit_leading_comments_inline_aware(&mut parts, printer);

    if parts.is_empty() {
        None
    } else {
        Some(d.concat(&parts))
    }
}

/// Build inline trailing block comments for an argument (non-expansion path).
///
/// Used for the last arg (comments before closing paren) and single-arg paths
/// where there's no comma to split around — i.e. always a **closer** gap, which is why it
/// takes [`PartitionedComments::for_closer_gap`] rather than the inter-item walk. The two
/// differ only in their claim on a **line** comment, and this emitter reads only
/// `trailing_block`, so the constructor is output-neutral here today; it is the contract
/// that matters, since the inter-item walk's rule is wrong for a gap holding the comma
/// `trailingComma: 'none'` deletes and would start mattering the moment this emitted more.
fn build_inline_trailing_comments(
    printer: &Printer<'_>,
    arg_end: u32,
    next_boundary: u32,
) -> Option<DocId> {
    let d = printer.d();
    let pc = PartitionedComments::for_closer_gap(printer, arg_end, next_boundary);

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

/// A lone arrow argument with an EXPRESSION body — the shape every hug arm in
/// [`build_chain_args_force_expand`] shares, before each asks its own body kind
/// (an expandable literal, or the breakable call / ternary that takes the
/// `(sig =>\n body,\n)` layout even when the chain forces expansion).
fn single_arrow_expression_body<'a>(
    call: &'a internal::CallExpression<'a>,
) -> Option<(
    &'a internal::ArrowFunctionExpression<'a>,
    &'a Expression<'a>,
)> {
    if call.arguments.len() != 1 {
        return None;
    }
    let Expression::ArrowFunctionExpression(arrow) = &call.arguments[0] else {
        return None;
    };
    match &arrow.body {
        internal::ArrowFunctionBody::Expression(body) => Some((arrow, body)),
        internal::ArrowFunctionBody::BlockStatement(_) => None,
    }
}

/// The hugged-arrow layout both expandable-literal arms emit: `(sig => <body>)`,
/// the signature hugged to the call paren and the body expanding internally behind
/// its own delimiters. `body_doc` arrives already built — the object arm's
/// grammar-required parens are the only thing the two assemble differently.
fn build_hugged_arrow_arg_doc(
    printer: &Printer<'_>,
    mut parts: DocBuf,
    ctx: ChainArgsContext,
    arrow: &internal::ArrowFunctionExpression<'_>,
    body_expr: &Expression<'_>,
    body_doc: DocId,
) -> DocId {
    let d = printer.d();
    let body_doc = prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
    let sig_doc = prepend_leading(
        d,
        ctx.leading_comment_doc,
        build_arrow_sig_doc(printer, arrow),
    );
    parts.push(d.text(ctx.prefix));
    parts.push(sig_doc);
    parts.push(d.text(" => "));
    parts.push(body_doc);
    parts.push(d.text(")"));
    d.concat(&parts)
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
    /// **to emit**: a non-owned leading comment before the first argument (emitted here).
    has_leading_comments: bool,
    /// **on page**: any leading comment before the first argument, **owned or not** — an
    /// owned comment rides inside the argument's own doc but still defeats the argument
    /// hug (prettier's `shouldExpandLastArg` returns false for a leading-commented arg). A
    /// to-emit gate would go blind to it and wrongly hug.
    has_leading_comment_on_page: bool,
    has_any_comments: bool,
    /// Whether the argument window puts any comment text on the page, **including** a
    /// comment a node owns and prints itself. The superset of `has_any_comments`, which is
    /// built from emit-keyed scans and so cannot see an owned comment. Layout gates take
    /// this one — an owned annotation on the last argument must refuse the expand-last hug
    /// exactly as an ordinary leading comment does.
    has_any_comment_text: bool,
    /// **on page**: a comment attaches to the last argument — leading it or trailing it
    /// before the `)`. The one predicate every argument-hug arm in all three branch
    /// builders asks, because prettier's `shouldExpandLastArg` returns false for a
    /// commented argument and because the hug layouts reassemble the argument from its
    /// signature and body docs, leaving its trailing gap with no emitter at all. Computed
    /// here rather than per builder: it was three identical re-derivations, and an arm that
    /// asked its own narrower question is exactly how the argument→`)` gap came to be
    /// DROPPED under forced expansion.
    last_arg_commented: bool,
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

    // Prettier's `anyArgEmptyLine` — the shared predicate, so a member chain's arguments
    // answer the blank question exactly as a plain call's do.
    let any_arg_empty_line = any_arg_empty_line(call.arguments, printer);

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
    // Counts owned comments — it only short-circuits the sub-scans below, and an owned
    // comment still puts text on the page. The sub-scans themselves stay emit-keyed
    // (`has_comments_to_emit_between`): the owning node prints those, so there is nothing to emit.
    let call_has_comments = printer.has_comments_on_page_between(paren_open, call.span.end);
    let has_leading_comments = call_has_comments
        && !call.arguments.is_empty()
        && printer.has_comments_to_emit_between(paren_open, call.arguments[0].span().start);
    // Layout counterpart of `has_leading_comments`: counts an owned leading comment too.
    let has_leading_comment_on_page = call_has_comments
        && !call.arguments.is_empty()
        && printer.has_comments_on_page_between(paren_open, call.arguments[0].span().start);
    let has_inter_arg_comments = call_has_comments && has_inter_argument_comments(call, printer);
    let has_trailing_comments = call_has_comments && has_trailing_comments_on_args(call, printer);
    // Also check for trailing block comments on last arg (for inline handling).
    let has_trailing_block_comments = call_has_comments
        && call.arguments.last().is_some_and(|last| {
            printer.has_comments_to_emit_between(last.span().end, call.span.end)
        });
    // A spread's stripped parens can hide a comment *before* the argument's own end,
    // where no scan above reaches it and the collapsing paths below drop it.
    let has_spread_paren_comments =
        call_has_comments && printer.any_spread_paren_comment_forces_expansion(call.arguments);
    let has_any_comments = has_leading_comments
        || has_inter_arg_comments
        || has_trailing_comments
        || has_trailing_block_comments
        || has_spread_paren_comments;
    // The argument-hug refusal, asked by every branch builder — see `ChainArgsContext`.
    // Guarded by the same window search, so a comment-free call pays nothing for it.
    let last_arg_commented = call_has_comments
        && last_arg_has_comments(call.arguments, printer, call.span.end, paren_open);

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
    // e.g., x.y(arr.map((e) => e[0]), ['foo']) — matches Prettier's isFunctionCompositionArgs.
    // That predicate also subsumes the all-arrows case (2+ arguments that are all arrows means
    // `function_count > 1`, which it short-circuits `true` on), so there is no separate
    // arrow-only disjunct here — the twin in `call_formatting.rs` states the same.
    let force_expand = force_expand
        || any_arg_empty_line
        || comments_force_expansion
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
        has_leading_comment_on_page,
        has_any_comments,
        has_any_comment_text: call_has_comments,
        last_arg_commented,
        has_trailing_block_comments,
        comments_force_expansion,
        standard_expansion,
        leading_comment_doc,
    };

    // Prettier's React-hook deps-array layout — the FIRST thing `printCallArguments` asks,
    // above `anyArgEmptyLine` and every specialized layout, and above the chain's own
    // forced expansion (prettier's member-chain printer reprints the group's arguments
    // through the same function, so the shape wins there too).
    if let Some(doc) = try_hook_deps_args_doc(
        printer,
        call.arguments,
        paren_open,
        call.span.end,
        call_has_comments,
        printer.d().text(prefix),
    ) {
        parts.push(doc);
        return printer.d().concat(&parts);
    }

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
    let ChainArgsContext {
        paren_open, prefix, ..
    } = ctx;
    push_empty_args(printer, &mut parts, paren_open, call.span.end, prefix);
    printer.d().concat(&parts)
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
        has_leading_comment_on_page,
        last_arg_commented,
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
    //
    // …and the leading half of that same refusal, which this arm is the last builder in the
    // family to ask. It reassembles the call from `prefix` + the argument's doc + `)`, so the
    // whole `callee`→argument gap reaches no emitter here (`docs/comments.md` hazard 4) — and
    // the hug is not tsv's to keep anyway: prettier's `shouldExpandLastArg` refuses on
    // `hasComment(lastArg, Leading)`, which `call_formatting.rs`'s `try_single_arg_comment_paths`
    // and `new_expression.rs` both honor. **on page**, not to-emit: a comment the argument owns
    // and prints itself still defeats the hug, and the gate that cannot see it hugs blind.
    if call.arguments.len() == 1
        && !has_leading_comment_on_page
        && !has_trailing_block_comments
        && !comments_force_expansion
    {
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

    // Special case: a single arrow arg the signature can hug to the call paren (prettier's
    // couldExpandArg) — three body kinds, each with its own layout below. For the two
    // expandable LITERAL bodies fits() measures only `chain.map((sig) => [` / `=> ({` and
    // truncates at the body's first hardline; the breakable ones break after `=>` instead.
    // Skip when comments_force_expansion — these layouts have no line to put a comment that
    // must own one on, and the `(`-line channel `leading_comment_doc` cannot carry has no
    // home here either.
    // Skip on a commented last argument — see `last_arg_commented` above.
    if !last_arg_commented
        && !comments_force_expansion
        && let Some((arrow, body_expr)) = single_arrow_expression_body(call)
    {
        // ARRAY body: `(sig => [\n  items,\n])` — array content on new lines, bracket
        // hugged.
        //
        // TODO: this is the one reassembling arm that asks half the refusal pair — the
        // body-tail question alone, where its OBJECT twin below asks
        // `arrow_hug_refused_by_comments` (and `!standard_expansion`). Instrumenting the arm's
        // entry over ~23k files (`tests/fixtures` + the prettier / svelte / kit / zzz / gro
        // trees) reaches it 6 times, all of them inside `tests/fixtures`: the signature
        // question is false at EVERY hit, so adding it cannot fire, and two of the hits DO
        // carry `standard_expansion` yet adding that half moves zero bytes (the state it
        // would change is not the one selected there). So no input separates either half —
        // hence no fixture, hence no change. Close it the moment one turns up.
        if matches!(body_expr, Expression::ArrayExpression(_))
            && !arrow_body_tail_has_comments(printer, arrow, body_expr)
        {
            let body_doc = printer.build_arg_expression_doc_expanded(body_expr);
            return build_hugged_arrow_arg_doc(printer, parts, ctx, arrow, body_expr, body_doc);
        }

        // OBJECT body: `(sig => ({\n  props\n}))` — the object expands behind its
        // grammar-required parens, synthesized here. Skipped under standard expansion —
        // prettier's all-broken fallback prints the argument normally (`(\n  sig => ({ … })
        // \n)`), the object breaking by width alone — and when a break forced inside the
        // signature invalidates the hug (`arrow_signature_has_breaking_comments`).
        if matches!(body_expr, Expression::ObjectExpression(_))
            && !standard_expansion
            && !arrow_hug_refused_by_comments(printer, arrow, body_expr)
        {
            let body_doc = d.parens(printer.build_arg_expression_doc_expanded(body_expr));
            return build_hugged_arrow_arg_doc(printer, parts, ctx, arrow, body_expr, body_doc);
        }

        // BREAKABLE body (call, ternary): the arrow-hugging break state directly,
        // `(sig =>\n  body,\n)` — prettier keeps the signature hugged even when forcing
        // expansion. couldExpandArg keys on the body type (looking through the return-type
        // annotation and a trailing `!`), so typed-return arrows hug too.
        // Skipped under standard expansion — short chains where the chain doesn't break
        // between groups need the standard `(\n  args,\n)` form to keep the first line short
        // enough for fits(); the standard form routes the argument→`)` gap through
        // `emit_last_arg_trailing_comments`, which is also prettier's layout for a commented
        // argument. Skipped on a signature break and on a body-tail comment for the same
        // reasons as the object arm above.
        if !standard_expansion
            && (arrow_body_is_call_through_non_null(body_expr) || is_ternary_arrow_body(body_expr))
            && !arrow_hug_refused_by_comments(printer, arrow, body_expr)
        {
            let body_doc = printer.build_expression_doc(body_expr);
            let body_doc =
                prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
            let sig_doc = build_arrow_sig_doc(printer, arrow);
            let sig_doc = prepend_leading(d, leading_comment_doc, sig_doc);

            parts.push(d.text(prefix));
            parts.push(sig_doc);
            parts.push(d.text(" =>"));
            parts.push(d.indent_hardline(body_doc));
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
    // Whether the gap just closed — between `arguments[i - 1]` and `arguments[i]` —
    // carries an author blank line. Computed once at the bottom of the previous
    // iteration (the no-comment branch) and reused here, since the top of this
    // iteration and that bottom look at the same gap under the same no-comment guard.
    let mut prev_gap_has_blank = false;

    for (i, arg) in call.arguments.iter().enumerate() {
        let arg_start = arg.span().start;

        // Handle leading comments before first argument
        if i == 0 && has_leading_comments {
            let first_pc = PartitionedComments::new(
                printer.comments,
                printer.comment_line_breaks,
                paren_open,
                arg_start,
            );

            // The delimiter-line question, in the conjunction the force-expanded builders
            // spell (`docs/comments.md`; `emit_first_arg_leading_comments`' rustdoc names
            // this caller): pull onto the `(` line only when a comment in the gap is what
            // forces the container open. A block-only run forces nothing, so it leads the
            // argument — which is prettier's answer and the plain call's
            // (`build_call_with_arg_comments` asks the same pair). Asking
            // `has_trailing_comments()` alone made this the one builder in the family that
            // pulled a lone block, and a `(`-line block is not even a fixed point where the
            // argument's own layout is what expanded the list.
            if first_pc.pulls_to_delimiter_line(printer) {
                // Comments trailing the `(` stay on the `(` line, author blank included;
                // the own-line set then leads the first arg (source order preserved — see
                // conformance_prettier_ts_comments.md §Comment relocation, Call open paren `(`).
                // A block hugging the arg stays inline (`/* b */ a`), an own-line block /
                // line comment takes its own line with author blanks preserved.
                first_pc.emit_delimiter_line_pull(&mut paren_line_prefix_parts, printer);
                first_pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
            } else if let Some(doc) = build_inline_leading_comments(printer, paren_open, arg_start)
            {
                // Nothing pulled, so the WHOLE gap leads the first argument — which is the
                // chain's own body-line emitter, asked here rather than re-derived: a
                // broke-after run takes its soft `line`, anything else glues to the
                // argument. Emitting only part of the gap is the DROP its doc warns about.
                arg_parts.push(doc);
            }
        }

        // Check for blank line before this arg (from previous arg)
        // Only add blank line preservation when there are no comments between args,
        // since comments with blank lines are handled in the separator logic below.
        if i > 0 {
            let prev_end = call.arguments[i - 1].span().end;
            let has_comments_before = printer.has_comments_to_emit_between(prev_end, arg_start);
            // Nothing to emit in the gap, but an owned annotation can still physically be
            // there — `prev_gap_has_blank` was measured with `blank_scan_end`, so its own
            // newlines don't read as a blank line yet an authored blank *before* it is kept.
            if !has_comments_before && prev_gap_has_blank {
                arg_parts.push(d.literalline());
                arg_parts.push(d.hardline());
            }
        }

        // An own-line format-ignore directive in this argument's gap freezes it verbatim
        // (Rule A). The three hugging special cases above all decline once a comment
        // forces expansion, so this is the only argument builder a directive reaches in
        // the chain family.
        //
        // A lone expression-body arrow renders its signature flat here, the same way the
        // expand-last arm does (prettier's `expandLastArg` `removeLines`): the argument is
        // already broken out onto its own line, so the remaining break belongs after `=>`,
        // on the body — never inside the parameter list. Without this the signature group
        // is the first break candidate this hardline-wrapped layout offers, and a
        // one-parameter arrow shatters into three lines. The non-chain path keeps the
        // signature intact by construction (`call_formatting.rs`'s `try_single_arg_hug`);
        // this is the chain path answering the question the same way rather than
        // differently.
        let flat_sig = call.arguments.len() == 1
            && matches!(arg, Expression::ArrowFunctionExpression(arrow)
                if arrow.body.is_expression() && !is_curried_arrow_chain(arg));
        let build_arg = || build_joined_argument_doc(printer, paren_open, call.arguments, i);
        arg_parts.push(if flat_sig {
            build_flat_params_arg_doc(printer, build_arg)
        } else {
            build_arg()
        });

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
            // before/after-comma trailing comments + comma, then the spread interior's
            // own-line share; the separator + leading comments below finish the gap.
            // This layout is hardline-joined throughout, so the gap's `forces_expansion`
            // obligation is already met and nothing reads it.
            let pc = printer
                .open_inter_arg_gap(&mut arg_parts, arg, next_arg_start)
                .comments;

            // Skip hardline if next arg has blank line
            // (blank line preservation at the top of the loop handles the line break)
            let has_comments_before_next =
                printer.has_comments_to_emit_between(arg_end, next_arg_start);
            let next_has_blank = if has_comments_before_next {
                pc.has_blank_line_in_gap(printer)
            } else {
                // Measure the no-comment gap's blank once; the top of the next iteration
                // reuses it (same gap, same guard) instead of re-scanning the window.
                printer.is_next_line_empty(arg_end, printer.blank_scan_end(arg_end, next_arg_start))
            };
            // Carry the no-comment gap's blank forward; a comment gap's blank is emitted
            // here, and the next iteration's top guard skips the reuse, so leave it false.
            prev_gap_has_blank = next_has_blank && !has_comments_before_next;
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
            // Last argument — the parent's share of a spread's stripped-paren interior,
            // then same-line trailing comments in source order (a block that sat after
            // the source comma just trails past where the comma was; a line comment
            // follows via `line_suffix`), then own-line dangling comments. No trailing
            // comma (trailingComma: 'none').
            emit_last_arg_trailing_comments(printer, &mut arg_parts, arg, next_boundary);
        }
    }

    parts.push(d.text(prefix));
    parts.push(d.concat(&paren_line_prefix_parts));
    // No trailing comma after the last arg (trailingComma: 'none') — the last-arg
    // comment emit trails same-line comments after the arg and emits no comma, so
    // nothing is appended here.
    parts.push(d.indent_hardline(d.concat(&arg_parts)));
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
        has_leading_comment_on_page,
        has_any_comments,
        has_any_comment_text,
        last_arg_commented,
        leading_comment_doc,
        ..
    } = ctx;
    // The chain's spelling of what a call-argument state opens with — its own `(` / `?.(`,
    // the callee being a separate group the chain printer places. Every ladder below is the
    // plain-call / `new` one read through it.
    let opener = ArgOpener::ChainPrefix(prefix);

    let arg = &call.arguments[0];

    // A broken `=>`→body gap forces the closing paren onto its own
    // line. Above every body-kind arm below, because the rule is the gap's and not the
    // body's — see [`last_arg_arrow_gap_break`]. The argument doc is built
    // here rather than reused from an arm because the arms below never run for this shape;
    // asking the cheap comment question first keeps that build off every other path.
    if has_any_comment_text
        && !last_arg_commented
        && let Some(gap_break) = last_arg_arrow_gap_break(printer, arg)
    {
        // ⚠️ This arm used to build ONE doc with flat parameters, which is right for a state
        // that can fall through and wrong for one that cannot: with no `allArgsBrokenOut()`
        // behind it, a flattened signature too wide for the line simply overflowed. The two
        // printings and the ladder go together — the shared entry point owns that pairing.
        parts.push(build_arrow_gap_break_single_arg_doc(
            printer,
            opener,
            arg,
            &gap_break,
            leading_comment_doc,
            || printer.build_arg_expression_doc(arg),
        ));
        return d.concat(&parts);
    }

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
        // `has_any_comment_text`: refusing the hug is a LAYOUT decision, so it must see a
        // comment the argument owns and prints itself (see `build_chain_args_multi`).
        && !last_arg_commented
        // …and the reassembling arm's refusal pair, the shared rule the object/array arm
        // below and the general gate already ask (`arrow_hug_refused_by_comments`). Its
        // body-tail half bites even though the flat state prints the whole arrow and would
        // keep the comment: a state set that is only conditionally lossless is one width away
        // from dropping it.
        && !(has_any_comment_text && arrow_hug_refused_by_comments(printer, arrow, body_expr))
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
            d.indent_hardline(body_doc),
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
        // `has_any_comment_text`: see above — a layout gate counts an owned comment.
        && !last_arg_commented
        // …and the same refusal pair as above — here its body-tail half bites at every state,
        // since the flat one synthesizes its own `(`/`)` around the body rather than printing
        // the arrow (`arrow_hug_refused_by_comments`).
        && !(has_any_comment_text && arrow_hug_refused_by_comments(printer, arrow, body_expr))
    {
        let arrow_doc = printer.build_arg_expression_doc(arg);
        let arrow_doc = prepend_leading(d, leading_comment_doc, arrow_doc);
        let body_doc = printer.build_expression_doc(body_expr);
        let body_doc =
            prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
        let sig_doc = build_arrow_sig_doc(printer, arrow);
        let sig_doc = prepend_leading(d, leading_comment_doc, sig_doc);

        // The ladder, shared with the plain-call and `new` spellings
        // ([`build_ternary_arrow_hug_ladder`]) — but the chain collapses it on a break
        // anywhere in the whole ARROW, where the other two ask only the body. Kept as
        // authored: the two questions differ exactly when the signature breaks on its own.
        //
        // TODO: `arrow_doc` is built for NOTHING ELSE — a whole second build of the argument
        // (its body included) that only answers `will_break`. Swapping it for `body_doc`
        // moves zero bytes over ~23k files, so the two agree on every shape real code and the
        // fixture suite write; but that is a render-side reading, and a `conditional_group`
        // state measured under an outer FLAT `fits` is exactly where such a claim has been
        // wrong before (TODO.md §The fits flat-walk is not render). Collapsing it is a
        // fanout win with its own probe to run, not part of this dedup.
        parts.push(build_ternary_arrow_hug_ladder(
            d, opener, sig_doc, body_doc, arrow_doc,
        ));
        return d.concat(&parts);
    }

    // Special case: arrow function with object/array expression body
    // Prettier's shouldExpandLastArg path: produces a 3-state conditional_group
    // so fluid assignments can expand call args instead of breaking after =.
    // couldExpandArg keys on the body type (looking through the return-type
    // annotation), so typed-return arrows are eligible.
    // Leading comments block expand-last (prettier's shouldExpandLastArg).
    // The layout itself is shared with the non-chain and `new` spellings — see
    // [`build_single_arrow_hug_doc`].
    if let Expression::ArrowFunctionExpression(arrow) = arg
        && arrow_body_expands_internally(arrow)
        // `has_any_comment_text` (on page), not `has_any_comments` (to emit): an owned
        // leading comment defeats the expand-last hug just like an ordinary one, so the
        // hug must refuse it too — a to-emit gate would hug blind.
        && !last_arg_commented
        // A comment LEADING the arg isn't the only shape that defeats the hug: one INSIDE
        // the signature forces a break the one-line hug can't honor. Same predicate as the
        // non-chain path — see `arrow_signature_has_breaking_comments`.
        && !(has_any_comment_text
            && arrow_signature_has_breaking_comments(printer, arrow))
    {
        // The two printings and the state ladder they feed — the same entry point the
        // non-chain and `new` sites take, differing only in the chain's argument builder and
        // its leading run ([`build_single_arrow_hug_doc`]). A curried chain's terminal
        // object/array is reached here too ([`arrow_body_expands_internally`], the guard
        // above and the ladder's own body-kind key), and for it the two printings genuinely
        // differ: the `expandLastArg` print has no chain layout, so a run of heads too wide
        // to hug fails on width instead of breaking its own heads and fitting.
        parts.push(build_single_arrow_hug_doc(
            printer,
            opener,
            arg,
            arrow,
            leading_comment_doc,
            || printer.build_arg_expression_doc(arg),
        ));
        return d.concat(&parts);
    }

    // Build arg doc in argument context. This is prettier's `printedArguments` — printed
    // with no `expandLastArg`, so a curried chain takes the progressive layout
    // (`build_printed_argument_doc`); the expand-last hug states above build their own.
    let arg_doc =
        build_printed_argument_doc(printer, arg, || printer.build_arg_expression_doc(arg));
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

    // Build combined arg doc with leading/trailing comments.
    //
    // ⚠️ Every layout arm below must splice THIS doc, never `arg_doc`. An arm that composes a
    // state out of the bare argument drops the whole gap run whenever that state wins — the
    // `})⟨⟩)` gap-injection shape, and hazard 4 in docs/comments.md. It is invisible to the
    // fixture suite until some document puts a comment there, and it is exactly what the
    // measured hug below reintroduced the first time it was written.
    let arg_with_comments = match (leading_comments_doc, trailing_comments_doc) {
        (Some(leading), Some(trailing)) => d.concat(&[leading, arg_doc, trailing]),
        (Some(leading), None) => d.concat(&[leading, arg_doc]),
        (None, Some(trailing)) => d.concat(&[arg_doc, trailing]),
        (None, None) => arg_doc,
    };

    // Check if it's a block-bodied callback whose parameter list a comment forces multiline.
    // These need soft-break wrapping to expand the call. A `function` expression is always
    // block-bodied, so it asks the question unconditionally — the arrow's `is_expression`
    // guard is about the arrow's *body* kind, not about the callee, and reading it as the
    // whole gate is what left the `function` twin ungated here.
    let callback_param_comment_forces_break = match arg {
        Expression::ArrowFunctionExpression(arrow) => {
            !arrow.body.is_expression() && arrow_signature_has_breaking_comments(printer, arrow)
        }
        Expression::FunctionExpression(func) => {
            function_signature_has_breaking_comments(printer, func)
        }
        _ => false,
    };

    if callback_param_comment_forces_break {
        // Callback with a forced-multiline parameter list - force expansion
        parts.push(opener.wrap_soft(d, arg_with_comments));
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
        parts.push(d.indent_hardline(arg_doc));
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
    // Cannot use `ArgOpener::wrap_soft` (a plain group) because will_break()
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
            d.indent_hardline(arg_with_comments),
            d.hardline(),
            d.text(")"),
        ]);
        if has_leading_comment_on_page || last_arg_commented {
            // A comment anywhere on the argument prevents hugging — force expansion, the
            // same `shouldExpandLastArg` rule the arms above ask, and prettier's layout for
            // a trailing one too (`(\n  (x) => {…} /* c */\n)`). An owned comment (on page
            // but not in `has_leading_comments`) rides inside `arg_with_comments` via
            // `arg_doc`, so it still defeats the hug.
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
    //
    // A comment that FORCES a break inside an arrow's signature does the same, and for the
    // same reason: hugging would strand the break mid-signature. This is the last of the
    // three places that must agree on when a hug is legal (the expand-last gate above, and
    // `call_formatting.rs`'s non-chain arm) — all asking the one predicate, because the
    // hug that mangled `arr.map(…)` while `fn(…)` stayed correct was precisely two of them
    // disagreeing. See `arrow_signature_has_breaking_comments`.
    let signature_forces_break = has_any_comment_text
        && matches!(arg, Expression::ArrowFunctionExpression(arrow)
            if arrow_signature_has_breaking_comments(printer, arrow));
    let kind = if has_leading_comment_on_page || signature_forces_break {
        ChainArgKind::NeedsSoftWrap
    } else {
        classify_chain_arg(arg)
    };
    match kind {
        ChainArgKind::NeedsSoftWrap => {
            // Needs soft-break wrapping - e.g., long strings
            parts.push(opener.wrap_soft(d, arg_with_comments));
        }
        ChainArgKind::NeedsWrapper => {
            // Huggable with internal break points (ternary, etc.)
            // Hugs opening paren; breaks the closing paren onto its own line
            // when content breaks (no trailing comma; trailingComma: 'none').
            parts.push(opener.hug_arg(d, arg_with_comments));
        }
        ChainArgKind::HugsNaturally if trailing_comments_doc.is_some() => {
            // A comment TRAILING the last argument defeats the hug, exactly as the leading
            // one two arms up does: prettier's `shouldExpandLastArg` refuses on both
            // (`!hasComment(lastArg, Leading) && !hasComment(lastArg, Trailing)`), so the
            // call takes the default broken-out layout. Only the two shapes that reach this
            // arm *with* a trailing comment were hugging — a `function` expression and a
            // block-terminal curried chain; every other trailing-comment argument
            // (object, array, block arrow, ternary, cast) is routed by an earlier path.
            parts.push(opener.wrap_soft(d, arg_with_comments));
        }
        ChainArgKind::HugsNaturally => {
            // A curried arrow chain is the one shape here whose hug has to be MEASURED.
            // Prettier reaches it through `shouldExpandLastArg`'s conditionalGroup, whose
            // hug states print the argument with `expandLastArg: true` — heads welded onto
            // the callee's line, which is what has to fit — and whose last state is
            // `allArgsBrokenOut()`, printed from `printedArguments`. An unconditional hug
            // measures the wrong thing: `arg_with_comments` already carries the progressive
            // layout, whose short first line always fits, so the call hugs where prettier
            // breaks out. Everything else here (objects, arrays, blocks, `function`s) hugs
            // unconditionally in prettier too, so it keeps the single state.
            if is_curried_arrow_chain(arg) {
                let state_hug = d.concat(&[d.text(prefix), arg_with_comments, d.text(")")]);
                parts.push(
                    d.conditional_group(&[state_hug, opener.wrap_soft(d, arg_with_comments)]),
                );
            } else {
                // Objects/arrays/blocks that hug naturally
                parts.push(d.text(prefix));
                parts.push(arg_with_comments);
                parts.push(d.text(")"));
            }
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
        has_any_comment_text,
        last_arg_commented,
        comments_force_expansion,
        ..
    } = ctx;
    // See [`build_chain_args_single`] — the chain's opener, through which every ladder below
    // is the plain-call / `new` one.
    let opener = ArgOpener::ChainPrefix(prefix);

    // A broken `=>`→body gap on the last argument forces the closing
    // paren onto its own line. Above every body-kind arm below, because the rule is the
    // gap's and not the body's — see [`last_arg_arrow_gap_break`]. The
    // cheap comment question gates the split-last build, so no other path pays for it.
    if call.arguments.len() >= 2
        && has_any_comment_text
        && !comments_force_expansion
        && !last_arg_commented
        && let Some(last) = call.arguments.last()
        && let Some(gap_break) = last_arg_arrow_gap_break(printer, last)
    {
        // The body both printings share, armed around each — built by the gate above.
        let body_reuse = gap_break.inject;
        let (head_parts, printed_last_arg_doc, all_args_broken) = printer
            .with_arrow_body_inject(body_reuse, || {
                build_args_split_last(call.arguments, printer, paren_open, has_any_comments)
            });
        parts.push(build_arrow_gap_break_multi_arg_doc(
            printer,
            opener,
            body_reuse,
            &head_parts,
            printed_last_arg_doc,
            all_args_broken,
            || printer.build_arg_expression_doc(last),
        ));
        return d.concat(&parts);
    }

    // Multiple arguments with block-body callback:
    // Use conditional_group to try inline first, then expand-all.
    // fits() checks actual width, handling both short and non-short preceding args.
    //
    // IMPORTANT: Cannot use `ArgOpener::wrap_soft` (a plain group) because
    // will_break() recurses into the block body's hardlines and forces break
    // without trying fits(). conditional_group uses fits() directly.
    if call.arguments.len() >= 2
        && call.arguments.last().is_some_and(is_block_function)
        && !comments_force_expansion
        // `has_any_comment_text`, not `has_any_comments`: refusing the expand-last hug is a
        // LAYOUT decision, so it must see a comment the last argument owns and prints
        // itself (a bundler annotation) — prettier's `shouldExpandLastArg` sees it too.
        && !last_arg_commented
    {
        let (head_parts, last_arg_doc, all_args_broken) =
            build_args_split_last(call.arguments, printer, paren_open, has_any_comments);

        // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
        if let Some(bail) = opener.expand_all_if_head_breaks(d, &head_parts, all_args_broken) {
            parts.push(bail);
            return d.concat(&parts);
        }

        // The same refusal one argument over: a break forced inside the LAST argument's own
        // signature invalidates the inline state just as a breaking head argument does. The
        // expression-body arms below have always asked it; this one — the block-bodied
        // callbacks, both spellings — never did.
        if call
            .arguments
            .last()
            .is_some_and(|arg| callback_signature_has_breaking_comments(printer, arg))
        {
            parts.push(opener.expand_all(d, all_args_broken));
            return d.concat(&parts);
        }

        parts.push(opener.inline_or_expand_all(d, &head_parts, last_arg_doc, all_args_broken));
        return d.concat(&parts);
    }

    // Expression arrow with call/conditional expression body
    // Prettier keeps preceding args inline and breaks after =>
    // e.g., `a.b(c, (x) =>\n  fn(x, ...),\n);`
    // couldExpandArg keys only on the body — param/return type annotations don't disable
    // the hug, so a typed arrow expands the same way (its full signature is emitted via
    // build_arrow_sig_doc).
    if call.arguments.len() >= 2
        && !comments_force_expansion
        && !(has_any_comments && last_arg_commented)
        && let Some(Expression::ArrowFunctionExpression(arrow)) = call.arguments.last()
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        // …and it reads the body through prettier's `stripChainElementWrappers`, so a call
        // reached through a trailing `!` is expandable too — the same pair the plain-call /
        // `new` twin asks ([`try_expand_last_arg`]). Asking the bare node kind here declined
        // the ladder for `(x) => fn(x)!` and broke every argument out, where both other
        // spellings — and prettier — keep the head inline.
        && (arrow_body_is_call_through_non_null(body_expr) || is_ternary_arrow_body(body_expr))
        // The reassembling arm's refusal pair, which bites here exactly as it does in the
        // single-argument arms above (`arrow_hug_refused_by_comments`).
        && !arrow_hug_refused_by_comments(printer, arrow, body_expr)
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
        if let Some(bail) = opener.expand_all_if_head_breaks(d, &head_parts, all_args_broken) {
            parts.push(bail);
            return d.concat(&parts);
        }

        let sig_doc = build_arrow_sig_doc(printer, arrow);
        // Reuse the pre-built call body (see above); conditional bodies build fresh.
        let body_doc =
            body_reuse.map_or_else(|| printer.build_expression_doc(body_expr), |(_, doc)| doc);
        let body_doc =
            prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);

        // The ladder — inline → break body → expand all — shared with the plain-call /
        // `new` spelling of this same layout ([`build_break_body_ladder`]).
        parts.push(build_break_body_ladder(
            d,
            opener,
            &head_parts,
            sig_doc,
            body_doc,
            last_arg_doc,
            all_args_broken,
        ));
        return d.concat(&parts);
    }

    // Expression arrow with an object/array terminal
    // Prettier keeps preceding args inline and expands object/array internally
    // e.g., `a.b(c, (x) => ({\n  y: x,\n}));`
    // couldExpandArg keys only on the terminal's type — a typed arrow expands the same
    // way, and so does a curried run of heads, since both states below render the
    // argument's own `expandLastArg` printing rather than a reassembled signature.
    if call.arguments.len() >= 2
        && !comments_force_expansion
        && !(has_any_comments && last_arg_commented)
        && let Some(last_arg) = call.arguments.last()
        && let Expression::ArrowFunctionExpression(arrow) = last_arg
        && arrow_body_expands_internally(arrow)
        // A break forced inside the signature — the hug renders the prefix and the signature
        // run on one line. NOT the body-tail question its reassembling siblings ask
        // ([`arrow_hug_refused_by_comments`]): these states render the argument's own
        // `expandLastArg` printing, so the body-end→arrow-end gap has an emitter here. See
        // [`arrow_body_tail_has_comments`] for why refusing it was a bug rather than
        // conservatism.
        && !arrow_signature_has_breaking_comments(printer, arrow)
    {
        // Expand-last arrow with an object/array terminal: build the terminal body ONCE and
        // inject it so BOTH printings of the argument reuse it — building it per printing
        // recurses into itself → O(2^depth). Re-armed around each printing, the same shape
        // the non-chain twin uses.
        let obj_reuse = prebuild_expand_last_obj_array_body(printer, Some(last_arg));

        let (head_parts, _printed_last_arg_doc, all_args_broken) = printer
            .with_arrow_body_inject(obj_reuse, || {
                build_args_split_last(call.arguments, printer, paren_open, has_any_comments)
            });

        // The willBreak bail and the three states, shared with the non-chain twin.
        parts.push(build_expand_last_obj_array_doc(
            printer,
            opener,
            obj_reuse,
            &head_parts,
            all_args_broken,
            || printer.build_arg_expression_doc(last_arg),
        ));
        return d.concat(&parts);
    }

    // "Expand first arg" pattern: first arg is block function, rest are short
    // e.g., `.reduce((acc, item) => { ... }, {})` - callback hugs, tail args stay inline
    // Matches prettier's shouldExpandFirstArg behavior
    // NOTE: Must come before expand-last-array/object to match Prettier's ordering —
    // shouldExpandFirstArg is checked before shouldExpandLastArg for arrays/objects.
    //
    // The shape test (two args, block-function first, short/comment-free second) is the
    // shared `should_expand_first_arg` — the chain used to inline a copy of it, which is how
    // one of the two gets a fix and the other doesn't. Only the chain-specific refusals are
    // spelled out here.
    // The refusals are one negated disjunction rather than a run of `&& !`: the shape test
    // says the layout applies, and everything inside the parens is a reason it cannot.
    if should_expand_first_arg(printer, call.arguments)
        && !(comments_force_expansion
            // `has_any_comment_text`, not `has_any_comments`: refusing the expand-FIRST hug is
            // a LAYOUT decision, so it must see a comment the first argument owns and prints
            // itself — the twin of the expand-last gate above.
            || (has_any_comment_text
                && first_arg_has_any_comments(call.arguments, printer, paren_open))
            // Prettier's `ArgExpansionBailout` reached through `expandFirstArg`; the plain-call
            // and `new` twins fold the same call into their `expand_first_blocked`.
            || first_arg_signature_refuses_expand_first(printer, call.arguments))
    {
        // The expand-first ladder, shared with the plain-call and `new` twins — including
        // the tail's willBreak bail and the broken-out fallback the chain used to hand-roll
        // beside it (`docs/comments.md` hazard 2 is answered there, once).
        parts.push(build_expand_first_arg_doc(
            printer,
            opener,
            call.arguments,
            paren_open,
            call.span.end,
        ));
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
        && !comments_force_expansion
        // `has_any_comment_text` (on page), not `has_any_comments` (to emit): an owned
        // comment leading the last array/object argument defeats the expand-last hug just
        // like an ordinary one — a to-emit gate would hug blind.
        && !last_arg_commented
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
        if let Some(bail) = opener.expand_all_if_head_breaks(d, &head_parts, all_args_broken) {
            parts.push(bail);
            return d.concat(&parts);
        }

        // The three-state ladder — inline → hug the last argument open → every argument on
        // its own line — shared with the plain-call / `new` spelling.
        parts.push(opener.inline_hug_or_expand_all(d, &head_parts, last_arg_doc, all_args_broken));
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

        // Broken-out layout, so prettier's `printedArguments` shape — a curried chain takes
        // the progressive layout (`build_printed_argument_doc`), as on the no-comment path.
        arg_parts.push(build_printed_argument_doc(printer, arg, || {
            printer.build_arg_expression_doc(arg)
        }));

        if is_last {
            // Trailing inline block comments after the last arg (before `)`).
            if has_any_comments
                && let Some(t) = build_inline_trailing_comments(printer, arg_end, call.span.end)
            {
                arg_parts.push(t);
            }
        } else {
            let next_arg_start = call.arguments[i + 1].span().start;
            if has_any_comments && printer.has_comments_to_emit_between(arg_end, next_arg_start) {
                // The shared argument-gap seam: everything through the comma
                // (before-comma blocks, the comma, a stranded after-comma block `A`), the
                // break, then the next argument's leading run (hugging after-comma `C` +
                // own-line comments).
                let (through_comma, leading) = build_arg_gap_docs(printer, arg_end, next_arg_start);
                arg_parts.extend(through_comma);
                arg_parts.push(d.line());
                arg_parts.extend(leading);
            } else {
                arg_parts.push(d.comma_line());
            }
        }
    }
    parts.push(opener.wrap_soft(d, d.concat(&arg_parts)));
    d.concat(&parts)
}
