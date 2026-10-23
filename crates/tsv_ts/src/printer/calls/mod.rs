// Call and member expression printing for TypeScript
//
// Handles printing of:
// - Call expressions: `foo()`, `obj.method(arg1, arg2)`
// - Member expressions: `obj.prop`, `arr[0]`
// - Method chains: `arr.filter().map()`
// - Test function calls: `it()`, `test.skip()`, `describe()`, etc.
// - Import expressions: `import('module')`, `import('module', options)`
//
// ## Module Organization
//
// - **mod.rs** (this file): Re-exports and entry point methods
// - **test_patterns.rs**: Test function detection (Jest, Mocha, Playwright, etc.)
// - **module_paths.rs**: Module path patterns (require, import.meta)
// - **arg_comments.rs**: Comment handling in argument lists
// - **arg_predicates.rs**: Call-argument and arrow shape predicates
// - **arg_wrapping.rs**: Argument classification and wrapping utilities
// - **call_formatting.rs**: Main call expression formatting logic
// - **expand_last.rs**: Prettier's `shouldExpandLastArg` layout, shared by calls and `new`
// - **new_expression.rs**: `new` expression formatting (shares the call wrapping patterns)
// - **import_expr.rs**: Import expression and meta property handling
// - **chain_args.rs**: Chain-specific argument building

mod arg_comments;
pub(in crate::printer) mod arg_predicates;
mod arg_wrapping;
mod call_formatting;
mod chain_args;
mod expand_last;
mod import_expr;
mod module_paths;
mod new_expression;
mod test_patterns;

// The module's public surface: exactly what OTHER printer modules consume. A name used only
// inside `calls/` does not belong here — re-exporting it makes a module-internal helper look
// like crate API and blocks its visibility from being narrowed. (A sibling reaching a helper
// through this list rather than through `super::<file>` is what kept nine dead entries alive.)
pub(crate) use arg_comments::{
    PartitionedComments, has_stripped_paren_gap, skip_stripped_open_paren,
};
pub(in crate::printer) use arg_wrapping::AssumedParamHug;
pub(in crate::printer) use import_expr::{ImportOptionsArg, build_import_args_comment_layout};

use super::chain::{self, ChainCall, call_callee_paren_leading_start};
use super::{ArrowChainContext, ParenContext, Printer, SecondTypeArgs, is_curried_arrow_chain};
use crate::ast::internal;
use crate::printer::expressions::functions::callback_signature_refuses_hug;
use arg_comments::{any_arg_empty_line, any_comment_forces_expansion, last_arg_has_comments};
use arg_predicates::is_block_function;
use arg_wrapping::{
    build_args_split_last, is_sole_multiline_template_arg, multiline_template_hug_applies,
};
use tsv_lang::Span;
use tsv_lang::doc::arena::DocId;

/// A call's callee gap: whether the callee prints a REQUIRED pair that keeps both its gaps
/// inside it, the operand→`)` region such a pair emits itself, how the call spells its `?.`,
/// and where every window past the callee opens.
///
/// ⚠️ **ONE derivation**, because the pair's own doc and the windows that open past its `)`
/// must agree: a `trailing_gap` the doc does not emit is a DROPPED comment, and the reverse
/// is a DOUBLE-PRINT. The three values were once derived twice — inline in
/// [`call_formatting::build_call_doc_with_wrapping`] and again in a type-args-only
/// `call_paren_open` — and the
/// two disagreed on exactly this pair. [`Self::optional`] joins them for the same reason: it
/// is the region [`optional_callee_gap_doc`] emits, and [`Self::start`] is what every other
/// window opens PAST it at, so deriving either alone re-splits the gap the wrong way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CalleeGap {
    /// The callee prints its own required pair (an IIFE, a sealed optional chain).
    pub(super) owned_pair: bool,
    /// The operand→`)` region that pair emits itself, when it owns one.
    pub(super) trailing_gap: Option<(u32, u32)>,
    /// Where every window past the callee opens: past that `)`, else the callee's span end
    /// — then past the `?.` of an optional call whose gap [`Self::optional`] splits.
    pub(super) start: u32,
    /// How this call spells its `?.` — see [`CallOptional`].
    pub(super) optional: CallOptional,
    /// The callee→`(` gap's **pre-paren half**, `[start, end)` with `end` the `(`, when
    /// that half holds a `//` and the gap therefore splits there.
    ///
    /// The non-optional sibling of [`CallOptional::Split`], and the same rule: a comment
    /// belongs to the side of the delimiter the author wrote it on. A `//` before the `(`
    /// trails the callee and hangs the whole argument list on a continuation line
    /// (`call // c⏎\t(a)`), which is the shape the EMPTY-argument spelling already takes
    /// through [`arg_comments::build_empty_args_parens_doc`] — so the two argument
    /// spellings stop answering one gap differently.
    ///
    /// ⚠️ **Line comments only.** A block comment forces nothing and leads the first
    /// argument in both formatters (`call /* c */(a)` → `call(/* c */ a)`), so splitting on
    /// one would turn a match into a divergence. The gap stays whole there, which is also
    /// what keeps `import /* c */ ('m')` intact
    /// ([`Printer::paren_line_share_anchor`] states the prefix rule this preserves).
    pub(super) paren_split: Option<(u32, u32)>,
}

/// The `?.` of a call, as the whole family needs to read it: WHO prints the token, and
/// whether the callee→`(` gap SPLITS at it.
///
/// ⚠️ The two are **not** the same question and a `bool` for either one answered the other
/// wrongly: a call whose gap declines to split still prints its own `?.`
/// ([`Self::Unsplit`]), so "no split" read as "no `?.`" DROPPED the token, and "prints its
/// own `?.`" read as "the gap splits" re-split a gap that must stay whole. Naming all four
/// states is what makes both unaskable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CallOptional {
    /// Not an optional call: no `?.` anywhere, and the whole callee→`(` gap belongs to the
    /// argument side.
    Absent,
    /// The `?.` FUSES into the argument list's own `?.(` opener rather than being printed by
    /// the callee side — the empty-argument spelling with no type arguments, where nothing
    /// follows the `?.` at all.
    ///
    /// The gap does not split here, and that is the same rule as [`Self::Split`] read from
    /// the other end rather than a carve-out: nothing follows the `?.` for an after-`?.`
    /// comment to lead, so both formatters normalize either authoring onto the callee side
    /// and there is no side left to preserve. That is the pure-separator entry the
    /// empty-argument twin is cataloged under
    /// (`docs/conformance_prettier_ts_comments.md` §Comment relocation), and its argument
    /// stops exactly here.
    Fused,
    /// This call prints its own `?.`, over a gap that deliberately does NOT split — an
    /// honored directive in the callee-side half (see [`call_optional`]), or a `?` the scan
    /// could not find. The whole gap stays the argument side's.
    Unsplit,
    /// This call prints its own `?.` behind the callee-side half of its gap, `[start, end)`,
    /// `end` being the `?`.
    ///
    /// Both formatters split this gap at the `?.` and preserve the authored side: a comment
    /// before it trails the callee (`fn /* c */?.(a)`), one after it leads whatever follows
    /// — the first argument (`fn?.(/* c */ a)`) or the type-argument list
    /// (`fn?./* c */ <A>(a)`).
    Split { start: u32, end: u32 },
}

impl CallOptional {
    /// Whether the argument list opens with `?.(` because the `?.` fused into it — the one
    /// spelling of that question, asked by both call printers (the plain path's
    /// `fuse_optional` and the chain's `prefix`) so the two cannot answer differently.
    pub(super) fn fused(self) -> bool {
        matches!(self, Self::Fused)
    }

    /// Where the ARGUMENT side of the callee→`(` gap opens on a split gap: past the `?.`.
    /// `None` when the gap does not split, leaving the argument side the whole gap.
    fn arg_side_start(self) -> Option<u32> {
        match self {
            Self::Split { end, .. } => Some(end + OPTIONAL_TOKEN_LEN),
            Self::Absent | Self::Fused | Self::Unsplit => None,
        }
    }
}

/// The width of `?.`, which a split gap's argument side opens past.
const OPTIONAL_TOKEN_LEN: u32 = "?.".len() as u32;

impl CalleeGap {
    /// The position the call's `(` follows — [`Self::start`], then past the type-argument
    /// list when the call has one (`fn<T>(a)`). Every argument-gap window in this family
    /// opens here: the leading-argument comment scans, and Rule A's first-argument freeze
    /// window ([`Printer::args_frozen_span`]). Separate from [`Self::start`] so a caller
    /// can't accidentally open the window at the callee and swallow the type arguments'
    /// own comments.
    pub(super) fn paren_open(&self, call: &internal::CallExpression<'_>) -> u32 {
        // On a split gap the argument side opens AT the `(` — the pre-paren half is the
        // callee side's ([`Self::paren_split`]), and opening before it would hand these
        // windows a comment the head has already emitted. Opening *at* the paren rather than
        // past it is deliberate: everything downstream still finds the `(` where it expects
        // it, so the after-`(` comments keep the delimiter-line rule unchanged.
        self.paren_split
            .map_or_else(|| args_side_start(call, self.start), |(_, paren)| paren)
    }
}

/// Resolve a call's [`CalleeGap`].
///
/// Nearly every call is the plain one — not optional, its callee printing no pair of its
/// own, and no comment between the callee and the first argument — and for it every field
/// is known without a search: nothing owns a pair, nothing splits, and the windows open at
/// the callee's end. That answer is given here, by tests that run no search, so the plain
/// call pays for none of the scans the derivation ([`callee_gap_wide`]) makes; every other
/// call is handed to it.
pub(super) fn callee_gap(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    span: Span,
) -> CalleeGap {
    if !call.optional && call_callee_paren_leading_start(call, span).is_none() {
        let start = call.callee.span().end;
        // [`paren_split_for`]'s own gate, over the gap it would be asked about.
        let gap_start = args_side_start(call, start);
        if call
            .arguments
            .first()
            .is_none_or(|arg| printer.gap_known_comment_free(gap_start, arg.span().start))
        {
            let plain = CalleeGap {
                owned_pair: false,
                trailing_gap: None,
                start,
                optional: CallOptional::Absent,
                paren_split: None,
            };
            debug_assert_eq!(
                plain,
                callee_gap_wide(printer, call, span),
                "the plain call's gap is the derivation's answer"
            );
            return plain;
        }
    }
    callee_gap_wide(printer, call, span)
}

/// The derivation behind [`callee_gap`] — the one definition of every field, out of line
/// for the calls its inline tests cannot answer.
#[inline(never)]
fn callee_gap_wide(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    span: Span,
) -> CalleeGap {
    let callee_end = call.callee.span().end;
    let owned_pair = call_callee_paren_leading_start(call, span).is_some();
    let trailing_gap = printer.owned_pair_trailing_gap(callee_end, owned_pair);
    let start = Printer::gap_start_after_owned_pair(callee_end, trailing_gap);
    let optional = call_optional(printer, call, span, start);
    let paren_open = args_side_start(call, optional.arg_side_start().unwrap_or(start));
    CalleeGap {
        paren_split: call_paren_split(printer, call, paren_open),
        owned_pair,
        trailing_gap,
        // Past the `?.` when the gap splits: everything below — the type-argument gap, the
        // leading-argument scans, the freeze window, the template hug — asks about the
        // region the ARGUMENT side owns, and opening it at the callee hands them a comment
        // `optional_callee_gap_doc` has already emitted.
        start: optional.arg_side_start().unwrap_or(start),
        optional,
    }
}

/// Where a callee gap's ARGUMENT side opens before the pre-paren split is taken into account:
/// past the type-argument list when the call has one, else `start_after_optional` (the callee's
/// end, or past the `?.` of a split optional gap).
///
/// One spelling for the two readers, per the ⚠️ on [`CalleeGap`]: [`callee_gap`] needs it to
/// derive [`CalleeGap::paren_split`] — which cannot be read off the struct that is still being
/// built — and [`CalleeGap::paren_open`] is this plus that split. Written out twice, the two
/// disagreed the moment either grew a clause, which is the failure that struct's doc names.
fn args_side_start(call: &internal::CallExpression<'_>, start_after_optional: u32) -> u32 {
    call.type_arguments
        .as_ref()
        .map_or(start_after_optional, |ta| ta.span.end)
}

/// Derive [`CalleeGap::paren_split`] — the pre-paren half of a NON-optional call's
/// callee→`(` gap, when a `//` written there splits it.
///
/// `gap_start` is where the argument side would otherwise open: past the `?.` of a split
/// optional gap, past the type-argument list when there is one. An optional call is excluded
/// outright — its gap already splits at the `?.`, one delimiter earlier, and the after-`?.`
/// half is the argument side's by that split's own rule.
///
/// The empty-argument spelling is excluded too, and reaches the same shape by its own route
/// ([`arg_comments::build_empty_args_parens_doc`]): with no arguments the whole gap is
/// pre-paren, so there is nothing to split off.
///
/// ⚠️ An honored **directive** in the pre-paren half DECLINES the split, for the reason
/// [`call_optional`] declines there: the freeze window opens at the argument side, so moving
/// that side past the `(` would leave the directive outside it and pass 2 would silently lose
/// the freeze, with no gate able to see it. Stated as what the split DESTROYS — a directive
/// written after the `(` sits in both windows and keeps it.
fn call_paren_split(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    gap_start: u32,
) -> Option<(u32, u32)> {
    if call.optional {
        return None;
    }
    paren_split_for(printer, gap_start, call.arguments)
}

/// [`call_paren_split`] over any argument list — the `new` expression's head asks it too, its
/// callee→`(` gap being the same gap under a different keyword.
///
/// `gap_start` is where the argument side would otherwise open. Returns the half to hang the
/// list under, `[gap_start, paren)`, or `None` when the gap does not split: no arguments (the
/// whole gap is pre-paren and `build_empty_args_parens_doc` owns it), no `(` found, no `//`
/// in the half, or a directive the split would strand outside the freeze window.
///
/// Asked of every call, `new` and `import()` with arguments, and nearly every one has no
/// comment anywhere in its callee→first-argument gap — most are the bare `(`, one byte. So
/// the comment question comes FIRST, inline, over `[gap_start, first_arg_start)`: the half
/// the split needs a `//` in, `[gap_start, paren)`, lies inside it (the `(` is found before
/// the first argument), so a gap known to hold no comment at all
/// ([`Printer::gap_known_comment_free`]) answers `None` exactly — without locating the paren
/// the answer would then have discarded, and without a call.
#[inline]
pub(super) fn paren_split_for(
    printer: &Printer<'_>,
    gap_start: u32,
    arguments: &[&internal::Expression<'_>],
) -> Option<(u32, u32)> {
    let first_arg_start = arguments.first()?.span().start;
    if printer.gap_known_comment_free(gap_start, first_arg_start) {
        debug_assert!(
            split_paren_wide(printer, gap_start, first_arg_start, arguments).is_none(),
            "a comment-free callee gap split at its `(`"
        );
        return None;
    }
    split_paren_wide(printer, gap_start, first_arg_start, arguments).map(|paren| (gap_start, paren))
}

/// [`paren_split_for`] outlined, behind a function call, for `import()`: its builder inlines
/// into the recursive expression dispatch, whose frame every nested expression pays, and a
/// construct that rare has no use for the inline gate there.
#[inline(never)]
pub(super) fn paren_split_for_outlined(
    printer: &Printer<'_>,
    gap_start: u32,
    arguments: &[&internal::Expression<'_>],
) -> Option<(u32, u32)> {
    paren_split_for(printer, gap_start, arguments)
}

/// The search half of [`paren_split_for`] — one outlined copy, for a callee→first-argument
/// gap the inline gate could not clear: locate the `(`, then ask the pre-paren half for a
/// `//`. Returns the `(` the gap splits at; the half's start is the caller's own `gap_start`.
///
/// Returns the paren alone rather than the pair so the answer comes back in a register: a
/// twelve-byte `Option<(u32, u32)>` is returned through a stack slot, and the common `None`
/// written there as a four-byte tag would be copied into [`CalleeGap`] with an eight-byte
/// load — a store that cannot forward to it.
#[inline(never)]
fn split_paren_wide(
    printer: &Printer<'_>,
    gap_start: u32,
    first_arg_start: u32,
    arguments: &[&internal::Expression<'_>],
) -> Option<u32> {
    let paren = printer.find_char_outside_comments(gap_start, first_arg_start, b'(')?;
    if !printer.has_line_comments_between(gap_start, paren) {
        return None;
    }
    if split_strands_directive(printer, gap_start, paren, arguments) {
        return None;
    }
    Some(paren)
}

/// Join a construct's HEAD to an argument list hung under the callee→`(` gap's pre-paren
/// half: the head, then the run, then the list one level in.
///
/// ⚠️ **`args` must have been built with an EMPTY head doc.** The head is emitted here
/// instead, ahead of the run, so the run's `indent(…)` wraps the list alone — an argument
/// builder that emitted its own head would print it twice, and one that emitted neither drops
/// the callee outright. Naming the contract here is the point: all three keywords that open an
/// argument list ([`call_formatting`], [`new_expression`], [`import_expr`]) build their list
/// through a builder that takes the head as a parameter, so no arm of any of them can forget.
pub(super) fn hang_args_under_split(
    printer: &Printer<'_>,
    head: DocId,
    split: (u32, u32),
    args: DocId,
) -> DocId {
    let (start, paren) = split;
    printer
        .d()
        .concat(&[head, printer.build_line_split_gap_doc(start, paren, args)])
}

/// Whether splitting a callee gap would STRAND an honored format-ignore directive: one that
/// reaches the first argument's freeze window from `gap_start` but no longer would from
/// `arg_side_start`, where the split leaves that window opening.
///
/// Both splits ask it — the `?.` ([`call_optional`]) and the `(` ([`paren_split_for`]) — and
/// the reason is the same at both, so it is stated once. The stranded run prints ahead of the
/// token, a placement where a directive is INERT, and pass 2 then silently loses the freeze
/// ⚠️ with no gate able to see it (the refusal
/// [`arg_wrapping::multiline_template_hug_applies`] makes, for this reason).
///
/// ⚠️ Phrased as what the split DESTROYS, never as "a directive is in the gap": one the author
/// wrote AFTER the token sits in both windows, is unmoved by the split, and keeps its freeze.
fn split_strands_directive(
    printer: &Printer<'_>,
    gap_start: u32,
    arg_side_start: u32,
    arguments: &[&internal::Expression<'_>],
) -> bool {
    !arguments.is_empty()
        && printer.args_frozen_span(gap_start, arguments, 0).is_some()
        && printer
            .args_frozen_span(arg_side_start, arguments, 0)
            .is_none()
}

/// Derive a call's [`CallOptional`] — the family's one answer to who prints the `?.` and
/// where the callee→`(` gap splits, from which [`CalleeGap::start`] then opens.
fn call_optional(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    span: Span,
    start: u32,
) -> CallOptional {
    if !call.optional {
        return CallOptional::Absent;
    }
    if call.arguments.is_empty() && call.type_arguments.is_none() {
        return CallOptional::Fused;
    }
    // The gap holds only whitespace, comments and the `?.` itself, so the first `?` outside
    // a comment is that token. Bounded by whatever follows it, never by the call's end: a
    // `?` inside an argument (`fn?.(a ? b : c)`) is past the bound and can't be reached.
    let end = call.type_arguments.as_ref().map_or_else(
        || {
            call.arguments
                .first()
                .map_or(span.end, |arg| arg.span().start)
        },
        |ta| ta.span.start,
    );
    let Some(question) = printer.find_char_outside_comments(start, end, b'?') else {
        return CallOptional::Unsplit;
    };
    // An honored directive the split would strand DECLINES it ([`split_strands_directive`]).
    if split_strands_directive(
        printer,
        start,
        question + OPTIONAL_TOKEN_LEN,
        call.arguments,
    ) {
        return CallOptional::Unsplit;
    }
    CallOptional::Split {
        start,
        end: question,
    }
}

/// The `?.` this call prints itself — the doc that goes between the callee and the argument
/// list's `(`, behind the callee-side half of its gap when that gap splits.
///
/// `None` when the call prints no `?.` of its own: a plain call, or the fused spelling whose
/// `?.` belongs to the argument list's `?.(` opener ([`CallOptional::Fused`]).
///
/// A **line** comment in the split half runs to end of line, so the `?.` cannot stay on it —
/// it takes the uniform forced-continuation indent every line-comment-split construct
/// shares, and a block comment stays inline glued in front of the `?.`. That is exactly
/// [`Printer::build_line_split_gap_doc`], shared with the answer the fused spelling reaches
/// through [`arg_comments::build_empty_args_parens_doc`]: the same gap, so the `?.` cannot change how it
/// reads.
pub(super) fn optional_callee_gap_doc(printer: &Printer<'_>, gap: CalleeGap) -> Option<DocId> {
    let d = printer.d();
    match gap.optional {
        CallOptional::Absent | CallOptional::Fused => None,
        CallOptional::Unsplit => Some(d.text("?.")),
        CallOptional::Split { start, end } => {
            Some(printer.build_line_split_gap_doc(start, end, d.text("?.")))
        }
    }
}

/// [`CalleeGap::paren_open`] for a caller that wants only the position — the dispatcher's
/// bypass tests and the test-call predicate, neither of which builds a callee doc.
pub(super) fn call_paren_open(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
    span: Span,
) -> u32 {
    callee_gap(printer, call, span).paren_open(call)
}

/// Whether a chain expression holds a call: its spine — member objects, `!` operands, and
/// `await` / `yield` arguments (`(await fn()).method()`) — reaches a call expression.
///
/// A loop down the spine, forced inline: every call build asks it, and LLVM keeps an
/// out-of-line copy otherwise, a call per ask for what is usually one or two kind tests.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(in crate::printer) fn chain_has_calls(mut expr: &internal::Expression<'_>) -> bool {
    loop {
        expr = match &expr.kind {
            internal::ExpressionKind::CallExpression(_) => return true,
            internal::ExpressionKind::MemberExpression(member) => member.object,
            internal::ExpressionKind::TSNonNullExpression(non_null) => non_null.expression,
            internal::ExpressionKind::AwaitExpression(await_expr) => await_expr.argument,
            internal::ExpressionKind::YieldExpression(yield_expr) => match yield_expr.argument {
                Some(argument) => argument,
                None => return false,
            },
            _ => return false,
        };
    }
}

/// The required paren pair around a CALLEE — `(x as T)()`, `(a && b)()`, `new (x ? y : z)()`.
///
/// **Ask the callee's kind BEFORE building its body.** Two of the three shapes want an
/// operand doc that is not the one `build_expression_doc` gives every other position, so a
/// caller that built the body first and asked afterwards would have to throw it away. That
/// is the whole reason this is a kind rather than a doc wrapper: [`Self::build_body_doc`]
/// and [`Self::build_doc`] are two consumers of ONE derivation, and the shape and the
/// builder can never disagree about which callee they are looking at.
///
/// Shared by both callee positions prettier treats alike — a call's and a `new`'s.
pub(super) enum CalleeParens<'a> {
    /// `(x ? y : z)()` — the pair welds to the argument list and the operand takes its
    /// ordinary doc. Prettier's answer for every callee kind but the two below: a ternary,
    /// an `await`, an optional chain and a sequence callee all weld.
    Welded(&'a internal::Expression<'a>),
    /// `(⏎\tx as T⏎)()` — prettier's `printBinaryCastExpression`
    /// (print/binary-cast-expression.js) gives an `as` / `satisfies` cast its own hanging
    /// group in exactly two positions: the callee of a call or `new`, and the OBJECT of a
    /// member access. tsv already spells the object half (the chain base's
    /// [`Printer::build_expanding_parens_body_doc`]), and this is the callee half — so the
    /// pair breaks around the operand instead of welding `new (` to it and breaking inside
    /// the operand, which is the shape the same cast takes one position over.
    ///
    /// Takes the same PAIR as [`Self::Binary`] and differs only in its body: it spelled the
    /// group *outside* the parens until that was measured against the shared inside-the-parens
    /// shape and found byte-identical over the whole fixture suite and the whole gates corpus.
    /// The outside-the-parens helper stays for the callers that have no choice — the ones
    /// whose `(` / `)` a different seam emits, so only a body is theirs to wrap.
    Cast(&'a internal::Expression<'a>),
    /// `(⏎\ta &&⏎\tb⏎)()` — prettier's binaryish EARLY RETURN, `key === "callee" &&
    /// isCallOrNewExpression(parent)` (`printBinaryishExpression`, `print/binaryish.js`), taken ahead of `shouldNotIndent`
    /// and so ahead of every indent question: `group([indent([softline, ...parts]),
    /// softline])` over FLAT parts. The operand is therefore the ungrouped chain — the
    /// paren group alone decides whether to break after `(` — and not the
    /// continuation-indented default a binaryish value takes everywhere else.
    Binary(&'a internal::BinaryExpression<'a>, Span),
}

impl<'a> CalleeParens<'a> {
    /// The pair `callee` prints in `context`, or `None` where it needs no pair. Ask before
    /// building the body. `second_type_args` is the call's or `new`'s own type argument
    /// list with its arguments, when it has one: an instantiation callee straight ahead of
    /// it keeps the pair [`Printer::instantiation_keeps_pair`] states.
    ///
    /// The `needs_parens` gate lives INSIDE the constructor so the type cannot exist for a
    /// callee that prints no pair — every one of its shapes is a claim about a pair, and a
    /// caller holding one for a bare callee would emit parens the grammar never asked for.
    pub(super) fn of(
        printer: &Printer<'_>,
        callee: &'a internal::Expression<'a>,
        context: ParenContext,
        second_type_args: Option<SecondTypeArgs>,
    ) -> Option<Self> {
        let pair = printer.needs_parens(callee, context)
            || second_type_args
                .is_some_and(|follow| printer.instantiation_keeps_pair(callee, follow))
            || (matches!(context, ParenContext::NewCallee)
                && printer.new_callee_holds_bare_call(callee));
        if !pair {
            return None;
        }
        Some(match &callee.kind {
            internal::ExpressionKind::TSAsExpression(_)
            | internal::ExpressionKind::TSSatisfiesExpression(_) => Self::Cast(callee),
            internal::ExpressionKind::BinaryExpression(binary) => Self::Binary(binary, callee.span),
            _ => Self::Welded(callee),
        })
    }

    /// The operand doc this pair wants, which is the ordinary expression doc for every
    /// shape but [`Self::Binary`].
    ///
    /// `pair_is_comment_free` is the caller's reading of the pair's own two gaps, which only
    /// it can see. A curried arrow chain takes prettier's callee shape
    /// ([`ArrowChainContext::Callee`]) when it holds — that shape opens the pair onto its own
    /// lines from INSIDE the operand's doc, and a comment in either gap hands the pair to a
    /// shell builder that owns those lines instead.
    pub(super) fn build_body_doc(
        &self,
        printer: &Printer<'_>,
        pair_is_comment_free: bool,
    ) -> DocId {
        match self {
            Self::Binary(binary, span) => printer.build_binary_chain_doc_ungrouped(binary, *span),
            Self::Welded(callee) if pair_is_comment_free && is_curried_arrow_chain(callee) => {
                printer.build_with_arrow_chain_context(ArrowChainContext::Callee, || {
                    printer.build_expression_doc(callee)
                })
            }
            Self::Welded(callee) | Self::Cast(callee) => printer.build_expression_doc(callee),
        }
    }

    /// The pair itself, around a body from [`Self::build_body_doc`].
    pub(super) fn build_doc(&self, printer: &Printer<'_>, body: DocId) -> DocId {
        match self {
            Self::Welded(_) => printer.arena().parens(body),
            Self::Cast(_) | Self::Binary(..) => printer.build_expanding_parens_doc(body),
        }
    }
}

/// Whether an arrow's expression `body` is one of the two kinds prettier's `couldExpandArg`
/// hugs to the call's parentheses directly (a call — through a trailing `!` too — or a
/// conditional), rather than breaking the call's arguments out around the arrow.
pub(in crate::printer) fn arrow_body_hugs_call_parens(body: &internal::Expression<'_>) -> bool {
    arg_predicates::arrow_body_is_call_through_non_null(body)
        || arg_predicates::is_ternary_arrow_body(body)
}

/// Whether a call's callee is memberish — a member expression or a `!` — so the call is a link
/// of a member chain.
///
/// The gate on BOTH of the chain decisions: the `printMemberChain` redirect here, and — in the
/// linearizer's `mark_own_call_layout` — which of the chain's own calls that redirect swallowed.
///
/// Prettier's `isMemberish` admits a member (or bind) expression; this one also takes a
/// `TSNonNullExpression` on purpose, so a call on `fnnn!` stays a link of its chain, laid out as
/// the same call without the `!` — prettier ends the chain at it
/// (`docs/conformance_prettier_ts.md` §Non-null callee in a member chain). A `!` over a call
/// (`fnnn(x)!(…)`) is a curried call instead, which [`Printer::build_call_doc`] routes ahead of
/// the chain as it routes the same call without the `!` ([`callee_is_curried_member_chain_call`]).
pub(in crate::printer) fn is_memberish(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr.kind,
        internal::ExpressionKind::MemberExpression(_)
            | internal::ExpressionKind::TSNonNullExpression(_)
    )
}

/// Whether `callee`, the call a curried call is made on, roots in a member chain: down its
/// callees through any number of further curried calls, the innermost call has a memberish
/// callee (`a.b().c(x)` of `a.b().c(x)(y)`, `a.c(x)(y)` of `a.c(x)(y)(z)`).
///
/// Prettier's `printMemberChain` is reached only by a call with a memberish callee, so a
/// call on a call never roots one: `printCallExpression` prints it `group([callee, args])`,
/// and the member chain stops at the innermost call, its parent being that outer call
/// rather than a statement. tsv's linearizer would instead fold the curried calls into the
/// chain's last group, where the chain's layout decides their lists too
/// ([`Printer::build_curried_member_chain_call_doc`] keeps them out). A curried run on any
/// other callee (`f(a)(b)`) has no member chain to stop, and stays one chain group.
fn curried_run_roots_in_member_chain(callee: &internal::CallExpression<'_>) -> bool {
    let mut call = callee;
    while let internal::ExpressionKind::CallExpression(inner) = &call.callee.kind {
        call = inner;
    }
    is_memberish(call.callee)
}

/// Whether a call on `callee` is a curried call on a member chain: `callee`, through any `!`
/// over it, is a call whose run roots in a member chain ([`curried_run_roots_in_member_chain`]),
/// as in `a.b().c(x)(y)` and `a.b().c(x)!(y)`.
///
/// Prettier's `isMemberish` takes no `TSNonNullExpression`, so a call on `c(x)!` reaches
/// `printCallExpression`'s `group([callee, args])` exactly as a call on `c(x)` does: the member
/// chain ends at the innermost call either way. tsv lays the two out alike — a `!` asserts a
/// type and nothing else — which here is prettier's layout too.
fn callee_is_curried_member_chain_call(callee: &internal::Expression<'_>) -> bool {
    let mut callee = callee;
    while let internal::ExpressionKind::TSNonNullExpression(non_null) = &callee.kind {
        callee = non_null.expression;
    }
    matches!(
        &callee.kind,
        internal::ExpressionKind::CallExpression(inner) if curried_run_roots_in_member_chain(inner)
    )
}

/// Whether `call`'s callee is a call taking more arguments than `call` does, and `call` takes
/// at least one — prettier's `isLongCurriedCallExpression`, asked from the outer call. The
/// chain decides the same question per link ([`chain::ChainCall::ungroup_args`]); this is
/// the question for a call that prints its callee whole: the plain call's head arms
/// [`Printer::long_curried_target`] with it, and the curried-callback route in
/// [`Printer::build_call_doc`], which builds the callee without that head, declines on it.
fn callee_is_long_curried(call: &internal::CallExpression<'_>) -> bool {
    matches!(
        &call.callee.kind,
        internal::ExpressionKind::CallExpression(inner)
            if !call.arguments.is_empty() && inner.arguments.len() > call.arguments.len()
    )
}

impl<'a> Printer<'a> {
    /// Whether `call` takes the member-chain bypass for a sole multiline template on the
    /// `(` line — prettier's `isTemplateOnItsOwnLine` arm of `printCallExpression`, which
    /// returns `[callee, "(", template, ")"]` before the `printMemberChain` redirect is
    /// reached ([`arg_wrapping::multiline_template_hug_applies`] is the argument half).
    ///
    /// Declines on a LINE comment anywhere in the callee: a `//` in a chain gap defers as a
    /// `line_suffix`, and with the heads flat there is no break of its own left to flush
    /// at (see the bypass in [`Self::build_call_doc`]).
    ///
    /// The one definition, asked where a call is printed ([`Self::build_call_doc`]) and
    /// where a member or a `!` is printed over such a call
    /// ([`Self::build_whole_call_operand_doc`]).
    ///
    /// Forced inline: it is asked of every call and member spine, and almost every one fails
    /// the shape test, so the call site keeps only that test and the rest stays out of line
    /// ([`Self::call_takes_template_bypass_beyond_shape`]).
    #[expect(clippy::inline_always)]
    #[inline(always)]
    pub(in crate::printer) fn call_takes_template_bypass(
        &self,
        call: &internal::CallExpression<'_>,
        span: Span,
    ) -> bool {
        // The shape test comes first: it reads the argument list alone, where the rest
        // needs the callee gap resolved to know where the `(` follows.
        is_sole_multiline_template_arg(call.arguments)
            && self.call_takes_template_bypass_beyond_shape(call, span)
    }

    /// [`Self::call_takes_template_bypass`] past its shape test: the argument half's gap
    /// reads, and no line comment between the call's start and its `(`.
    #[inline(never)]
    fn call_takes_template_bypass_beyond_shape(
        &self,
        call: &internal::CallExpression<'_>,
        span: Span,
    ) -> bool {
        let paren_open = call_paren_open(self, call, span);
        multiline_template_hug_applies(self, call.arguments, paren_open)
            && !self.has_line_comments_between(span.start, paren_open)
    }

    /// Whether the first call down `expr`'s object spine — through member objects and
    /// `!` operands — is one prettier prints whole rather than as a link of a member chain:
    /// a call taking the template bypass ([`Self::call_takes_template_bypass`]), or a curried
    /// call on a member chain ([`callee_is_curried_member_chain_call`]).
    ///
    /// Prettier prints a member through `printMemberExpression` (`[print(object),
    /// lookup]`) and a `!` as `[print(expression), "!"]`; only a CALL reaches
    /// `printMemberChain`, which flattens its callee spine without printing the links. So
    /// a member or `!` over a call that never reaches it prints over that call whole:
    ///
    /// - A stack of bypassed calls (`` v.m(r).o(`⏎a⏎`).o(`⏎b⏎`) ``) prints each link
    ///   through its own call printer — each glued to the template that closes the link
    ///   below — and a member or `!` ending such a stack (`` …o(`⏎b⏎`).p ``) prints over it
    ///   the same way.
    /// - A call on a call is `printCallExpression`'s `group([callee, args])`, so a member
    ///   over a curried call on a member chain (`obj.a().b().fnnn(x)(z).p`) prints over that
    ///   call, whose member chain ends at the innermost call — as the curried call prints
    ///   with nothing over it ([`Self::build_curried_member_chain_call_doc`]).
    ///
    /// tsv's member and `!` printers linearize instead, so they ask this first. A call whose
    /// callee spine merely CONTAINS such a call still linearizes it — `printMemberChain`
    /// absorbs the templates, and the curried calls, once a call of its own opens the chain
    /// (`` …o(`⏎b⏎`).p(x) ``, `…fnnn(x)(z).p()`).
    fn spine_first_call_prints_whole(&self, expr: &internal::Expression<'_>) -> bool {
        let mut expr = expr;
        loop {
            expr = match &expr.kind {
                internal::ExpressionKind::MemberExpression(member) => member.object,
                internal::ExpressionKind::TSNonNullExpression(non_null) => non_null.expression,
                internal::ExpressionKind::CallExpression(call) => {
                    return callee_is_curried_member_chain_call(call.callee)
                        || self.call_takes_template_bypass(call, expr.span);
                }
                _ => return false,
            };
        }
    }

    /// The doc of `operand` printed whole, when the member or `!` spanning `span` over it
    /// prints as prettier's member and non-null printers do — the operand's spine reaches a
    /// call prettier prints whole ([`Self::spine_first_call_prints_whole`]) — and `None`
    /// when the caller linearizes as usual. `parent_optional` is the member's own `?.`
    /// (`false` for a `!`).
    ///
    /// An operand that is a parenthesized optional chain the parent's access seals
    /// (`` (v?.o(`⏎a⏎`)).p `` — [`chain::child_stops_optional_chain`]) keeps its pair:
    /// without it the access would join the chain, moving its short-circuit boundary. So
    /// does a shell a comparison region keeps ([`Self::region_keeps_shell`]), as the
    /// linearizer keeps both at a member object. The assignment-target / `new`-callee mark
    /// follows every other operand down to the call
    /// ([`Self::build_with_inline_every_lookup_forwarded`]).
    ///
    /// Declines for any comment on the page in `span`, block comments included — this route
    /// does not place them where they belong: one in the lookup's own gap prints inside the
    /// lookup's break group, after its softline, where prettier prints it ahead of the
    /// group (an own-line one stays inline, one after the `.` moves before it), and one
    /// inside the pair kept around a sealed operand is dropped, the pair being printed
    /// around the bare operand with no emitter for the gap it strips. A `//` the operand
    /// defers would be owed prettier's `lineSuffixBoundary` between the two halves, which
    /// this route does not print. The linearized chain places all of them.
    pub(in crate::printer) fn build_whole_call_operand_doc(
        &self,
        operand: &internal::Expression<'_>,
        span: Span,
        parent_optional: bool,
    ) -> Option<DocId> {
        if !self.spine_first_call_prints_whole(operand)
            || self.has_comments_on_page_between(span.start, span.end)
        {
            return None;
        }
        // No expression-statement mark to clear: the spine ends at a call printed whole,
        // whose own route clears it ([`Self::build_call_doc`]).
        Some(
            if chain::child_stops_optional_chain(span.start, parent_optional, operand)
                || self.region_keeps_shell(operand)
            {
                // A sealed chain is the `ChainExpression` prettier's ancestor walks stop
                // at, so the mark stays behind with this lookup; a region's shell, tsv's
                // own pair, is given the same answer.
                self.d().parens(self.build_expression_doc(operand))
            } else {
                self.build_with_inline_every_lookup_forwarded(span, operand, || {
                    self.build_expression_doc(operand)
                })
            },
        )
    }

    /// A member whose object reaches a call prettier prints whole, printed as prettier's
    /// `printMemberExpression` prints it — the object whole, then this lookup — rather than
    /// linearizing the object's links into a chain ([`Self::spine_first_call_prints_whole`]);
    /// `None` for every other member.
    ///
    /// Out of line: [`Self::build_member_doc`] is inlined into the expression dispatch, the
    /// expression recursion itself, and this rare shape must add nothing to its frames.
    #[inline(never)]
    fn build_whole_call_member_doc(
        &self,
        member: &internal::MemberExpression<'_>,
        span: Span,
    ) -> Option<DocId> {
        let object_doc = self.build_whole_call_operand_doc(member.object, span, member.optional)?;
        Some(chain::build_lookup_after_operand_doc(
            member, span, object_doc, self,
        ))
    }

    /// A curried call on a member chain ([`callee_is_curried_member_chain_call`]), as
    /// `printCallExpression` prints a call on a call: `group([callee, args])`, the callee
    /// whole. The callee's chain is printed for this call rather than as a statement's, so it takes no short-head merge (prettier's
    /// `shouldMerge` reads `isExpressionStatement` off the chain's parent, which is this call);
    /// a callee taking more arguments than this call leaves its own list without a group
    /// ([`Printer::long_curried_target`], armed by the callee build).
    #[inline(never)]
    fn build_curried_member_chain_call_doc(
        &self,
        call: &internal::CallExpression<'_>,
        span: Span,
    ) -> DocId {
        self.is_expression_statement.set(false);
        let doc = self.build_call_doc_with_wrapping(call, span);
        self.d().group(doc)
    }

    /// Build a Doc for a call expression with argument wrapping (not chain-aware)
    pub(super) fn build_call_doc_with_wrapping(
        &self,
        call: &internal::CallExpression<'_>,
        span: Span,
    ) -> DocId {
        call_formatting::build_call_doc_with_wrapping(self, call, span)
    }

    /// Build a Doc for a call expression (for nested contexts)
    ///
    /// Uses the chain module for:
    /// 1. True chains (callee contains nested calls, like `a().b()`)
    /// 2. Memberish callees with comments between member segments
    ///
    /// Simple calls like `obj.method()` use the simple call path unless they have
    /// comments between member segments.
    pub(super) fn build_call_doc(&self, call: &internal::CallExpression<'_>, span: Span) -> DocId {
        // A curried call on a member chain (`a.b().c(x)(y)`, `a.b().c(x)!(y)`) is not a link of
        // that chain.
        if callee_is_curried_member_chain_call(call.callee) {
            return self.build_curried_member_chain_call_doc(call, span);
        }
        // Curried call with callback pattern: fn()('arg', () => { ... })
        // When the callee is a simple call expression and the last argument is a
        // block function, use conditional_group to try inline first, then expand-all.
        //
        // Skip when the inner call has array/object args — those may force multiline,
        // and the chain formatter handles that correctly via group(oneLine).
        if let internal::ExpressionKind::CallExpression(inner) = &call.callee.kind {
            let inner_has_multiline_arg = inner.arguments.iter().any(|arg| {
                matches!(
                    arg.kind,
                    internal::ExpressionKind::ArrayExpression(_)
                        | internal::ExpressionKind::ObjectExpression(_)
                )
            });
            let any_arg_empty_line = any_arg_empty_line(call.arguments, self);
            let paren_open = call.callee.span().end;
            // Whole-call comment-presence gate (one binary search over the argument
            // window); short-circuits the comment predicates below and threads into
            // build_args_split_last. Canonical reference: build_params_doc_with_comments.
            //
            // Counts owned comments: this asks whether the argument window puts any comment
            // text on the page (a *layout* question), not who emits it. A bundler annotation
            // on the last argument must still disable the expand-last hug, exactly as an
            // ordinary leading comment does — prettier's `shouldExpandLastArg` sees it.
            let call_has_comments = self.has_comments_on_page_between(paren_open, span.end);
            // The route rebuilds the call from its callee and arguments alone, so a call with
            // type arguments or a `?.` takes the general path, which prints them.
            if call.type_arguments.is_none()
                && !call.optional
                && call.arguments.len() >= 2
                && call
                    .arguments
                    .last()
                    .copied()
                    .is_some_and(is_block_function)
                && !any_arg_empty_line
                && !(call_has_comments
                    && any_comment_forces_expansion(call, span, self, paren_open))
                && !(call_has_comments
                    && last_arg_has_comments(call.arguments, self, span.end, paren_open))
                && !inner_has_multiline_arg
                // A long-curried callee prints its list without a group, which this route —
                // printing the callee whole — cannot express. The chain path prints it.
                && !callee_is_long_curried(call)
                // The hug renders the callback's signature head on this line, so a
                // signature that will break refuses it here as at every expand-last site;
                // the fall-through breaks every argument out.
                && !call.arguments.last().is_some_and(|arg| {
                    callback_signature_refuses_hug(self, arg, call.arguments.len())
                })
            {
                let d = self.d();
                let callee_doc = self.build_expression_doc(call.callee);

                // Build args split into head (with commas) and last
                // Leading comments before first arg are handled inside build_args_split_last
                let assumed = call
                    .arguments
                    .last()
                    .and_then(|last| AssumedParamHug::new(self, last));
                let (head_parts, last_arg_doc, all_args_broken) =
                    AssumedParamHug::sharing(assumed.as_ref(), self, || {
                        build_args_split_last(call.arguments, self, paren_open, call_has_comments)
                    });
                let last_index = call.arguments.len() - 1;
                let assumed = assumed.map(|hug| {
                    // Prettier's `shouldExpandParameters`: a plain call's last of several
                    // arguments flattens a `function`'s parameters in the hug.
                    hug.build(self, call.arguments.len() > 1, || {
                        arg_wrapping::build_joined_argument_doc(
                            self,
                            paren_open,
                            call.arguments,
                            last_index,
                        )
                    })
                });

                // The two-state ladder every expand-last printer shares, so the curried
                // callee stays outside its states like any other callee.
                return arg_wrapping::ArgOpener::callee(d, callee_doc)
                    .inline_assumed_or_expand_all(
                        d,
                        &head_parts,
                        last_arg_doc,
                        assumed,
                        all_args_broken,
                    );
            }
        }

        // Check if this is a true chain (callee contains calls, like `a().b()`)
        let is_true_chain = chain_has_calls(call.callee);

        // For memberish callees, use chain module to format the entire call expression.
        // This ensures proper handling of member chains in assignments - the chain module
        // returns group(oneLine) for short chains, letting the assignment's Fluid layout
        // decide whether to break after `=`.
        //
        // Without this, the callee is formatted separately as a member chain with
        // conditional_group/fill that has internal break points, causing the chain
        // to break before the assignment breaks.
        let callee_is_memberish = is_memberish(call.callee);

        if is_true_chain || callee_is_memberish {
            // ⚠️ The MEMBER-CHAIN BYPASS, stated once. Prettier states it once too: the
            // `printMemberChain` redirect sits BELOW one `if (isTemplateLiteralSingleArg ||
            // … || isTestCall(…))` in `printCallExpression`, so a call in any of those shapes
            // never reaches the chain layout at all. Both arms route to the same place —
            // `build_call_doc_with_wrapping`, where each layout is actually answered — so
            // this block decides ROUTING only, and neither rule gets a second definition
            // here. It lives inside this arm because that is the only routing it changes: a
            // non-memberish callee already falls through to the wrapping path below.
            //
            // 1. **Test calls** (`it.skip`, `test.only`, …) stay on one line past the print
            //    width; the chain path knows nothing about that special-casing.
            // 2. **A sole multiline template on the `(` line** keeps the whole call flat
            //    (`` a.b().c().d(`x⏎y`) ``, where the chain path would break the heads).
            //
            // Each declines on a comment its flat form has no emitter for — the test call on
            // an argument-gap comment (its own predicate), the template on a LINE comment
            // anywhere in the callee. A `//` in a chain gap defers as a `line_suffix`, and
            // with the heads flat there is no break of its own left to flush at, so it drains
            // at the first break the ARGUMENT side produces — welding onto the `(`-line run's
            // own comment (`a.b().c() // c⏎.d(// z⏎…)` → `.d(// z // c`), content loss the
            // print-once ledger is blind to. Line comments only: ownership binds a block
            // (`owned ⇒ is_block`) and a block never defers, so this is the axis-free half of
            // the question.
            if test_patterns::test_call_flat_layout_applies(call, span, self) {
                return self.build_call_doc_with_wrapping(call, span);
            }
            if self.call_takes_template_bypass(call, span) {
                // The callee is printed for this call, not as the statement's chain: a
                // chain inside it takes no short-head merge (prettier's `shouldMerge` reads
                // `isExpressionStatement(parent)`, and its parent is the callee's member).
                self.is_expression_statement.set(false);
                return self.build_call_doc_with_wrapping(call, span);
            }

            // Use chain wrapping for chains (nested calls) or memberish callees
            let mut nodes = chain::ChainNodeVec::new();
            let long_curried = chain::linearize_chain_from_call_into(
                call,
                span,
                self.linearize_input(),
                &mut nodes,
            );
            // This chain is the callee of a call taking fewer arguments: its last link's list
            // prints without a group, and a short chain returns without one of its own
            // ([`chain::ChainCall::ungroup_args`]).
            if self.long_curried_target.get() == Some(span)
                && let Some(chain::ChainNode::Call { facts, .. }) = nodes.last_mut()
            {
                facts.ungroup_args = true;
            }
            chain::build_linearized_chain_doc(
                &mut nodes,
                long_curried,
                call.callee,
                span.start,
                span,
                self,
            )
        } else {
            // Simple call (non-memberish callee) - wrap args directly
            self.build_call_doc_with_wrapping(call, span)
        }
    }

    /// Build a Doc for a member expression with optional breaking at dots
    ///
    /// Uses the new chain architecture based on prettier's member-chain.js:
    /// 1. Linearize AST into flat list of chain nodes
    /// 2. Group nodes by natural break points
    /// 3. Build doc with conditionalGroup for oneLine/expanded alternatives
    pub(super) fn build_member_doc(
        &self,
        member: &internal::MemberExpression<'_>,
        span: Span,
    ) -> DocId {
        // A format-ignore directive attached to this member access (in the gap between
        // the object and the property) makes prettier print the entire member
        // expression verbatim from source — preserving inner call args (numbers,
        // etc.) that the chain formatter would otherwise reformat. Mirrors
        // prettier's `hasPrettierIgnore` → verbatim-print behavior.
        if self.member_gap_frozen(member.object.span().end, member.property.span().start) {
            return self.build_frozen_opaque_node_doc(span);
        }

        // Asked only of an object that is itself a link — a call, a member or a `!`; the
        // object of `a.b` or `this.b` reaches no call — so the common member pays one kind
        // test and no call.
        if matches!(
            member.object.kind,
            internal::ExpressionKind::CallExpression(_)
                | internal::ExpressionKind::MemberExpression(_)
                | internal::ExpressionKind::TSNonNullExpression(_)
        ) && let Some(doc) = self.build_whole_call_member_doc(member, span)
        {
            return doc;
        }

        // Use chain-based implementation
        let mut nodes = chain::ChainNodeVec::new();
        let long_curried = chain::linearize_chain_from_member_into(
            member,
            span,
            self.linearize_input(),
            &mut nodes,
        );
        chain::build_linearized_chain_doc(
            &mut nodes,
            long_curried,
            member.object,
            span.start,
            span,
            self,
        )
    }

    /// Build a Doc for a dynamic import expression: `import('module')` or `import('module', options)`
    pub(super) fn build_import_expression_doc(
        &self,
        import_expr: &internal::ImportExpression<'_>,
        span: Span,
    ) -> DocId {
        import_expr::build_import_expression_doc(self, import_expr, span)
    }

    /// Build a Doc for a meta property: `import.meta`, `new.target`
    pub(super) fn build_meta_property_doc(&self, meta: &internal::MetaProperty<'_>) -> DocId {
        import_expr::build_meta_property_doc(self, meta)
    }

    /// Build a Doc for call arguments only (for chain printing)
    ///
    /// Uses proper group wrapping so args can break independently from the chain.
    pub(super) fn build_call_args_doc_for_chain(
        &self,
        call: &internal::CallExpression<'_>,
        span: Span,
        facts: ChainCall,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain(self, call, span, facts)
    }

    /// The doc a chain call prints ahead of its `(` ([`chain_args::chain_call_head_doc`]).
    pub(in crate::printer) fn chain_call_head_doc(
        &self,
        call: &internal::CallExpression<'_>,
        span: Span,
    ) -> Option<DocId> {
        chain_args::chain_call_head_doc(self, call, callee_gap(self, call, span))
    }

    /// Build a Doc for call arguments with forced expansion (hardlines instead of softlines)
    ///
    /// Used for the "args expanded, chain inline" state in conditionalGroup.
    pub(super) fn build_call_args_doc_for_chain_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        span: Span,
        facts: ChainCall,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain_expanded(self, call, span, facts)
    }
}
