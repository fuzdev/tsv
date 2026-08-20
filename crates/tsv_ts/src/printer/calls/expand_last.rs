// Prettier's `shouldExpandLastArg` layout, shared by the plain-call and `new` printers.
//
// Prettier prints both through ONE `printCallArguments` (its header comment lists
// `NewExpression` beside `CallExpression`), so the two must answer this question with one
// body. They did not: `new`'s spelling was a ~140-line near-copy that had drifted — it never
// gained the `useMemo` exclusion or the object/array arrow-body hug state, and the copy's
// existence is why the missing `couldExpandArg` classification survived as long as it did.

use super::arg_comments::{any_comment_forces_expansion_slice, last_arg_has_comments};
use super::arg_predicates::{
    arrow_body_is_call_through_non_null, is_array_or_object_unwrapped, is_concise_numeric_array,
    is_ternary_arrow_body,
};
use super::arg_wrapping::{
    ArgOpener, arrow_body_expands_internally, arrow_hug_refused_by_comments, build_args_split_last,
    build_arrow_gap_break_multi_arg_doc, build_arrow_sig_doc, build_break_body_ladder,
    build_expand_last_obj_array_doc, could_expand_arrow_chain, last_arg_arrow_gap_break,
    last_two_args_same_type, prebuild_expand_last_break_body, prebuild_expand_last_obj_array_body,
    prepend_arrow_body_comments,
};
use crate::ast::internal;
use crate::printer::Printer;
use crate::printer::expressions::functions::{
    arrow_signature_has_breaking_comments, callback_signature_has_breaking_comments,
};
use tsv_lang::doc::arena::DocId;

/// Which call-like node the arguments belong to.
///
/// The layout is otherwise identical — prettier shares one `printCallArguments` — and this
/// carries the **one** place the two genuinely diverge, which is a mechanism rather than a
/// taste call: `printFunction` gates its `shouldExpandParameters` on
/// `isCallExpression(parent)`, and `isCallExpression` is `CallExpression |
/// OptionalCallExpression` — **not** `NewExpression`. So a `function` last argument never
/// reaches the `ArgExpansionBailout` under `new` and keeps its hug even when a comment forces
/// its signature open, while the same callback under a plain call expands. The arrow printer
/// has no such parent check (`arrow-function.js` reads `args.expandLastArg` directly), which
/// is why the arrow refusal applies under both.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArgOwner {
    Call,
    New,
}

impl ArgOwner {
    /// Whether a break forced inside the last argument's signature invalidates the hug states,
    /// which render that signature's head on the callee's line.
    ///
    /// See the type's doc for why the `function` half is asked only for a plain call.
    fn callback_signature_refuses_hug(
        self,
        printer: &Printer<'_>,
        arg: &internal::Expression<'_>,
    ) -> bool {
        match self {
            Self::Call => callback_signature_has_breaking_comments(printer, arg),
            Self::New => match arg {
                internal::Expression::ArrowFunctionExpression(arrow) => {
                    arrow_signature_has_breaking_comments(printer, arrow)
                }
                _ => false,
            },
        }
    }
}

/// Prettier's `shouldExpandLastArg` (`print/call-arguments.js`) plus the layouts it selects.
///
/// Returns `None` when the guard fails, so the caller falls through to its own comment /
/// blank-line / default paths.
///
/// `callee` is the whole head the `(` follows — for `new` that includes the `new ` keyword and
/// the type arguments, which is the ONLY thing the two callers pass differently.
///
/// **The `couldExpandArg` classification is asked HERE, above the argument doc builds**, not
/// after them. Reaching it late and then declining builds every argument twice — once for this
/// layout and once on the caller's general path, which recurses into any call nested in the
/// body: `f(lead, p => f(lead, q => …))` went 2^depth doc nodes (`fanout:audit`'s
/// `ts_nested_arrow_multiarg_new`). Every predicate the gate asks is a pure AST/comment
/// question, so none of them needs a doc.
///
/// It is spelled as **reachability over the arms below** rather than as a second body-kind
/// list, so the two cannot drift: an arrow argument is hug-eligible when
/// `could_expand_arrow_chain` claims it (a block body, an object/array body, or an arrow chain
/// ending in one), or when the break-body arm claims it (a call through a trailing `!`, or a
/// ternary) and that arm's own refusal stays silent — the SAME `arrow_hug_refused_by_comments`
/// the arm guards itself with. Anything else — a template, a binary, a member, a `new` body —
/// no state below can hug, and the caller's general path prints it whole.
pub(super) fn try_expand_last_arg(
    printer: &Printer<'_>,
    arguments: &[internal::Expression<'_>],
    callee: DocId,
    paren_open: u32,
    call_end: u32,
    has_comments: bool,
    owner: ArgOwner,
) -> Option<DocId> {
    let d = printer.d();
    if arguments.len() < 2 {
        return None;
    }
    let opener = ArgOpener::Callee(callee);

    let last_arg = arguments.last();
    let last_is_function = matches!(
        last_arg,
        Some(
            internal::Expression::ArrowFunctionExpression(_)
                | internal::Expression::FunctionExpression(_)
        )
    );
    // Prettier excludes "concise" arrays (all numeric literals) — those use fill layout, whose
    // break characteristics the hug states can't express.
    let last_is_collection = last_arg
        .is_some_and(|arg| is_array_or_object_unwrapped(arg) && !is_concise_numeric_array(arg));
    if !(last_is_function || last_is_collection) {
        return None;
    }

    let hug_eligible = match last_arg {
        Some(internal::Expression::ArrowFunctionExpression(arrow)) => {
            could_expand_arrow_chain(arrow)
                || match &arrow.body {
                    internal::ArrowFunctionBody::Expression(body_expr) => {
                        (arrow_body_is_call_through_non_null(body_expr)
                            || is_ternary_arrow_body(body_expr))
                            && !(has_comments
                                && arrow_hug_refused_by_comments(printer, arrow, body_expr))
                    }
                    internal::ArrowFunctionBody::BlockStatement(_) => false,
                }
        }
        _ => true,
    };
    // …with one exception, which is why prettier can ask `couldExpandArg` late: a broken
    // `=>`→body gap selects its own state ([`last_arg_arrow_gap_break`]) ahead of the
    // body-kind question the arms below ask. It is not ahead of `couldExpandArg` itself —
    // that gate is the first thing the helper applies, since a body prettier would not
    // expand never reaches the arrow printing whose softline this state exists to carry.
    // The state needs the argument docs, so it keeps the block below — and returns from
    // inside it, so no second build follows.
    let arrow_gap_break = if has_comments {
        last_arg.and_then(|last| last_arg_arrow_gap_break(printer, last))
    } else {
        None
    };
    if !(hug_eligible || arrow_gap_break.is_some()) {
        return None;
    }

    // `useMemo(() => func(), [foo, bar, baz])` — prettier's `shouldExpandLastArg` refuses to hug
    // a deps array behind an arrow (`call-arguments.js`), so both spellings break every argument
    // out. Distinct from the React-hook deps LAYOUT (`isReactHookCallWithDepsArray`, asked far
    // above this by both callers), which is narrower: it wants a zero-parameter BLOCK arrow. This
    // one takes any arrow, so an expression-bodied one — whose doc forces no break — reaches
    // here and is the only shape where the two answers are observable.
    if arguments.len() == 2
        && matches!(
            arguments.first(),
            Some(internal::Expression::ArrowFunctionExpression(_))
        )
        && matches!(last_arg, Some(internal::Expression::ArrayExpression(_)))
    {
        return None;
    }

    // A comment that needs a line of its own defeats the hug; an inline block glued between an
    // argument and its comma does not. The `(`→first-argument gap and a spread's stripped-paren
    // interior are the shared predicate's too.
    if has_comments && any_comment_forces_expansion_slice(arguments, printer, paren_open, call_end)
    {
        return None;
    }
    // On-page: a leading or trailing comment on the last argument defeats the expand-last hug
    // (prettier's `shouldExpandLastArg` asks `hasComment(lastArg, Leading | Trailing)`), owned
    // or not — an owned comment rides inside the argument's doc, so this must count it (on
    // page), not just the emit-keyed ones, or it hugs blind.
    if has_comments && last_arg_has_comments(arguments, printer, call_end, paren_open) {
        return None;
    }

    // Expand-last arrow whose body is a call / object / array: build the body ONCE and inject it
    // so the whole-arrow arg doc reuses it (the break-body / hug state below reuses it too).
    // Building it in both places recurses into itself → O(2^depth).
    //
    // The gap-break gate above already built one for the state it selects — that state's ladder
    // reaches shapes neither prebuild here covers (a block terminal, a plain expression body),
    // and it is also what answers the gate's own width half. So when the gate fired, these two
    // are skipped outright: the state below returns before either arm that reads them, and
    // building the object/array one again would be the exact second build this whole
    // pre-building exists to avoid.
    let (body_reuse, obj_reuse) = if arrow_gap_break.is_some() {
        (None, None)
    } else {
        let call_body = prebuild_expand_last_break_body(printer, last_arg, has_comments);
        let obj_body = if call_body.is_none() {
            prebuild_expand_last_obj_array_body(printer, last_arg)
        } else {
            None
        };
        (call_body, obj_body)
    };
    let inject = match &arrow_gap_break {
        Some(gap_break) => gap_break.inject,
        None => body_reuse.or(obj_reuse),
    };

    let (head_parts, last_arg_doc, all_args_broken) = printer
        .with_arrow_body_inject(inject, || {
            build_args_split_last(arguments, printer, paren_open, has_comments)
        });

    // Prettier: if (headArgs.some(willBreak)) return allArgsBrokenOut()
    if let Some(bail) = opener.expand_all_if_head_breaks(d, &head_parts, all_args_broken) {
        return Some(bail);
    }

    // A broken `=>`→body gap drops the closing paren to its own line, asked above every
    // body-kind arm for the reason [`last_arg_arrow_gap_break`] gives.
    if arrow_gap_break.is_some()
        && let Some(last) = last_arg
    {
        return Some(build_arrow_gap_break_multi_arg_doc(
            printer,
            opener,
            inject,
            &head_parts,
            last_arg_doc,
            all_args_broken,
            || printer.build_expression_doc(last),
        ));
    }

    if last_is_function {
        // Expression arrow with a call / conditional body: the head arguments stay inline and
        // only the body breaks after `=>` (`fn({ a: 1 }, (x) =>⏎\tcall(x, …)⏎)`).
        if let Some(internal::Expression::ArrowFunctionExpression(arrow)) = last_arg
            && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
            && (arrow_body_is_call_through_non_null(body_expr)
                || matches!(&**body_expr, internal::Expression::ConditionalExpression(_)))
            // The reassembling arm's refusal pair, which holds at every one of these states —
            // and the same question the eligibility gate above already answered to let this
            // block be entered at all.
            && !(has_comments && arrow_hug_refused_by_comments(printer, arrow, body_expr))
        {
            let sig_doc = build_arrow_sig_doc(printer, arrow);
            // Reuse the pre-built call body (see above); conditional bodies build fresh.
            let body_doc =
                body_reuse.map_or_else(|| printer.build_expression_doc(body_expr), |(_, doc)| doc);
            let body_doc =
                prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);

            // The ladder, shared with the member chain's spelling of this same layout.
            return Some(build_break_body_ladder(
                d,
                opener,
                &head_parts,
                sig_doc,
                body_doc,
                last_arg_doc,
                all_args_broken,
            ));
        }

        // Expression arrow with an object / array terminal: the head arguments stay inline and
        // the body expands internally (`fn(arg, (x) => ({⏎\ta: x⏎}))`), through any number of
        // curried heads (`fn(arg, (x) => (y) => ({⏎\ta: x⏎}))`). `couldExpandArg` keys only on
        // the terminal's type, so a typed arrow expands the same way.
        if let Some(arg) = last_arg
            && let internal::Expression::ArrowFunctionExpression(arrow) = arg
            && arrow_body_expands_internally(arrow)
            // A break forced inside the signature — the hug renders the callee and the
            // signature run on one line. NOT the body-tail question its reassembling
            // siblings ask ([`arrow_hug_refused_by_comments`]): these states render the
            // argument's own `expandLastArg` printing, so the body-end→arrow-end gap has an
            // emitter here. See [`arrow_body_tail_has_comments`] for why refusing it was a
            // bug rather than conservatism.
            && !(has_comments && arrow_signature_has_breaking_comments(printer, arrow))
        {
            // The ladder, shared with the member chain's spelling of this same layout.
            return Some(build_expand_last_obj_array_doc(
                printer,
                opener,
                obj_reuse,
                &head_parts,
                all_args_broken,
                || printer.build_expression_doc(arg),
            ));
        }

        // Block-body arrows, block-terminal arrow chains, and function expressions: inline vs
        // expand-all, since the argument's own doc expands.
        if last_arg.is_some_and(|arg| owner.callback_signature_refuses_hug(printer, arg)) {
            return Some(opener.expand_all(d, all_args_broken));
        }
        return Some(opener.inline_or_expand_all(d, &head_parts, last_arg_doc, all_args_broken));
    }

    // Array / object last argument. Prettier disables the hug state when the last two arguments
    // have the same outer type (`penultimateArg.type === lastArg.type`), so a breaking last
    // argument expands everything instead.
    if last_two_args_same_type(arguments) {
        if d.will_break(last_arg_doc) {
            return Some(opener.expand_all(d, all_args_broken));
        }
        return Some(opener.inline_or_expand_all(d, &head_parts, last_arg_doc, all_args_broken));
    }

    // Different types: the 3-state ladder (inline → hug → expand all). For a breaking last
    // argument prettier keeps the hug — `[breakParent, conditionalGroup([hug,
    // allArgsBrokenOut])]` — there is no forced-break → inline-or-expand-all form.
    Some(opener.inline_hug_or_expand_all(d, &head_parts, last_arg_doc, all_args_broken))
}
