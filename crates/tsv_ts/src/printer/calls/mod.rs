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
pub(in crate::printer) use import_expr::{ImportOptionsArg, build_import_args_comment_layout};

use super::Printer;
use super::chain::{self, ChainCall, call_callee_paren_leading_start};
use crate::ast::internal;
use arg_comments::{any_arg_empty_line, any_comment_forces_expansion, last_arg_has_comments};
use arg_predicates::is_block_function;
use arg_wrapping::{build_args_split_last, multiline_template_hug_applies};
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
#[derive(Clone, Copy)]
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
}

/// The `?.` of a call, as the whole family needs to read it: WHO prints the token, and
/// whether the callee→`(` gap SPLITS at it.
///
/// ⚠️ The two are **not** the same question and a `bool` for either one answered the other
/// wrongly: a call whose gap declines to split still prints its own `?.`
/// ([`Self::Unsplit`]), so "no split" read as "no `?.`" DROPPED the token, and "prints its
/// own `?.`" read as "the gap splits" re-split a gap that must stay whole. Naming all four
/// states is what makes both unaskable.
#[derive(Clone, Copy)]
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
        call.type_arguments
            .as_ref()
            .map_or(self.start, |ta| ta.span.end)
    }
}

/// Resolve a call's [`CalleeGap`].
pub(super) fn callee_gap(printer: &Printer<'_>, call: &internal::CallExpression<'_>) -> CalleeGap {
    let callee_end = call.callee.span().end;
    let owned_pair = call_callee_paren_leading_start(call).is_some();
    let trailing_gap = printer.owned_pair_trailing_gap(callee_end, owned_pair);
    let start = Printer::gap_start_after_owned_pair(callee_end, trailing_gap);
    let optional = call_optional(printer, call, start);
    CalleeGap {
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

/// Derive a call's [`CallOptional`] — the family's one answer to who prints the `?.` and
/// where the callee→`(` gap splits, from which [`CalleeGap::start`] then opens.
fn call_optional(
    printer: &Printer<'_>,
    call: &internal::CallExpression<'_>,
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
                .map_or(call.span.end, |arg| arg.span().start)
        },
        |ta| ta.span.start,
    );
    let Some(question) = printer.find_char_outside_comments(start, end, b'?') else {
        return CallOptional::Unsplit;
    };
    // An honored directive in the callee-side half DECLINES the split, the same refusal
    // [`arg_wrapping::multiline_template_hug_applies`] makes for the same reason: the run
    // would print ahead of the `?.`, a placement where a directive is INERT, and the freeze
    // window — which opens at [`CalleeGap::start`] — would no longer reach it, so pass 2
    // silently loses the freeze ⚠️ with no gate able to see it. Stated as what the split
    // DESTROYS rather than as "a directive is in the gap": one the author wrote AFTER the
    // `?.` sits in both windows, is unmoved by the split, and keeps it.
    if !call.arguments.is_empty()
        && printer.args_frozen_span(start, call.arguments, 0).is_some()
        && printer
            .args_frozen_span(question + OPTIONAL_TOKEN_LEN, call.arguments, 0)
            .is_none()
    {
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
/// through [`arg_comments::push_empty_args`]: the same gap, so the `?.` cannot change how it
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
pub(super) fn call_paren_open(printer: &Printer<'_>, call: &internal::CallExpression<'_>) -> u32 {
    callee_gap(printer, call).paren_open(call)
}

/// Check if a chain expression contains any call expressions
pub(in crate::printer) fn chain_has_calls(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::CallExpression(_) => true,
        internal::Expression::MemberExpression(member) => chain_has_calls(member.object),
        internal::Expression::TSNonNullExpression(non_null) => chain_has_calls(non_null.expression),
        // Look through await/yield to find nested calls: (await fn()).method()
        internal::Expression::AwaitExpression(await_expr) => chain_has_calls(await_expr.argument),
        internal::Expression::YieldExpression(yield_expr) => yield_expr
            .argument
            .as_ref()
            .is_some_and(|arg| chain_has_calls(arg)),
        _ => false,
    }
}

/// The required paren pair around a CALLEE — `(x as T)()`, `new (x as T)()`.
///
/// Prettier's `printBinaryCastExpression` (print/binary-cast-expression.js) gives an
/// `as` / `satisfies` cast its own hanging group in exactly two positions: the callee of a
/// call or `new`, and the OBJECT of a member access. tsv already spells the object half
/// (the chain base's `build_expanding_parens_body_doc`), and this is the callee half — so
/// the pair breaks around the operand (`new (⏎\tx as T⏎)()`) instead of welding `new (` to
/// it and breaking inside the operand, which is the shape the same cast takes one position
/// over.
///
/// Every other callee kind takes the plain pair, and that is prettier's answer too: a
/// ternary, an `await`, an optional chain and a sequence callee all weld. A BINARY callee
/// hangs as well, through the `new` printer's own arm — it needs the ungrouped operand doc,
/// which this seam does not build.
pub(super) fn build_callee_parens_doc(
    printer: &Printer<'_>,
    callee: &internal::Expression<'_>,
    callee_doc: DocId,
) -> DocId {
    let d = printer.arena();
    let body = if matches!(
        callee,
        internal::Expression::TSAsExpression(_) | internal::Expression::TSSatisfiesExpression(_)
    ) {
        printer.build_expanding_parens_body_doc(callee_doc)
    } else {
        callee_doc
    };
    d.parens(body)
}

/// Check if callee is a member expression (used for chain detection)
///
/// Prettier's `isMemberish`, and the gate on BOTH of its chain decisions: the
/// `printMemberChain` redirect here, and — in the linearizer's
/// `mark_own_call_layout` — which of the chain's own calls that redirect swallowed.
pub(in crate::printer) fn is_memberish(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::MemberExpression(_) | internal::Expression::TSNonNullExpression(_)
    )
}

impl<'a> Printer<'a> {
    /// Build a Doc for a call expression with argument wrapping (not chain-aware)
    pub(super) fn build_call_doc_with_wrapping(
        &self,
        call: &internal::CallExpression<'_>,
    ) -> DocId {
        call_formatting::build_call_doc_with_wrapping(self, call)
    }

    /// Build a Doc for a call expression (for nested contexts)
    ///
    /// Uses the chain module for:
    /// 1. True chains (callee contains nested calls, like `a().b()`)
    /// 2. Memberish callees with comments between member segments
    ///
    /// Simple calls like `obj.method()` use the simple call path unless they have
    /// comments between member segments.
    pub(super) fn build_call_doc(&self, call: &internal::CallExpression<'_>) -> DocId {
        // Curried call with callback pattern: fn()('arg', () => { ... })
        // When the callee is a simple call expression and the last argument is a
        // block function, use conditional_group to try inline first, then expand-all.
        //
        // Skip when the inner call has array/object args — those may force multiline,
        // and the chain formatter handles that correctly via group(oneLine).
        if let internal::Expression::CallExpression(inner) = call.callee {
            let inner_has_multiline_arg = inner.arguments.iter().any(|arg| {
                matches!(
                    arg,
                    internal::Expression::ArrayExpression(_)
                        | internal::Expression::ObjectExpression(_)
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
            let call_has_comments = self.has_comments_on_page_between(paren_open, call.span.end);
            if call.arguments.len() >= 2
                && call.arguments.last().is_some_and(is_block_function)
                && !any_arg_empty_line
                && !(call_has_comments && any_comment_forces_expansion(call, self, paren_open))
                && !(call_has_comments
                    && last_arg_has_comments(call.arguments, self, call.span.end, paren_open))
                && !inner_has_multiline_arg
            {
                let d = self.d();
                let callee_doc = self.build_expression_doc(call.callee);

                // Build args split into head (with commas) and last
                // Leading comments before first arg are handled inside build_args_split_last
                let (head_parts, last_arg_doc, all_args_broken) =
                    build_args_split_last(call.arguments, self, paren_open, call_has_comments);

                let state_inline = d.concat(&[
                    callee_doc,
                    d.text("("),
                    d.concat(&head_parts),
                    last_arg_doc,
                    d.text(")"),
                ]);
                let state_expand_all = d.concat(&[
                    callee_doc,
                    d.text("("),
                    d.indent(d.concat(&[d.line(), all_args_broken])),
                    d.line(),
                    d.text(")"),
                ]);

                return d.conditional_group(&[state_inline, state_expand_all]);
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
            if test_patterns::test_call_flat_layout_applies(call, self) {
                return self.build_call_doc_with_wrapping(call);
            }
            let paren_open = call_paren_open(self, call);
            if multiline_template_hug_applies(self, call.arguments, paren_open)
                && !self.has_line_comments_between(call.span.start, paren_open)
            {
                return self.build_call_doc_with_wrapping(call);
            }

            // Use chain wrapping for chains (nested calls) or memberish callees
            let nodes = chain::linearize_chain_from_call(call, self.linearize_input());
            let (head_start, head_end) =
                chain_head_comment_window(&nodes, call.callee, call.span.start);
            let groups = chain::group_chain_nodes(&nodes, self.comments);
            let chain_doc = chain::build_chain_doc(&groups, call.span, self);
            self.prepend_removed_paren_comments(head_start, head_end, chain_doc)
        } else {
            // Simple call (non-memberish callee) - wrap args directly
            self.build_call_doc_with_wrapping(call)
        }
    }

    /// Build a Doc for a member expression with optional breaking at dots
    ///
    /// Uses the new chain architecture based on prettier's member-chain.js:
    /// 1. Linearize AST into flat list of chain nodes
    /// 2. Group nodes by natural break points
    /// 3. Build doc with conditionalGroup for oneLine/expanded alternatives
    pub(super) fn build_member_doc(&self, member: &internal::MemberExpression<'_>) -> DocId {
        // A format-ignore directive attached to this member access (in the gap between
        // the object and the property) makes prettier print the entire member
        // expression verbatim from source — preserving inner call args (numbers,
        // etc.) that the chain formatter would otherwise reformat. Mirrors
        // prettier's `hasPrettierIgnore` → verbatim-print behavior.
        if self.member_gap_frozen(member.object.span().end, member.property.span().start) {
            return self.build_frozen_opaque_node_doc(member.span);
        }

        // Use chain-based implementation
        let nodes = chain::linearize_chain_from_member(member, self.linearize_input());
        let (head_start, head_end) =
            chain_head_comment_window(&nodes, member.object, member.span.start);
        let groups = chain::group_chain_nodes(&nodes, self.comments);
        let chain_doc = chain::build_chain_doc(&groups, member.span, self);

        // Prepend comments from removed parentheses at the chain base — the share the
        // chain's own widened nodes did not claim ([`chain_head_comment_window`]).
        self.prepend_removed_paren_comments(head_start, head_end, chain_doc)
    }

    /// Build a Doc for a dynamic import expression: `import('module')` or `import('module', options)`
    pub(super) fn build_import_expression_doc(
        &self,
        import_expr: &internal::ImportExpression<'_>,
    ) -> DocId {
        import_expr::build_import_expression_doc(self, import_expr)
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
        facts: ChainCall,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain(self, call, facts)
    }

    /// Build a Doc for call arguments with forced expansion (hardlines instead of softlines)
    ///
    /// Used for the "args expanded, chain inline" state in conditionalGroup.
    pub(super) fn build_call_args_doc_for_chain_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        facts: ChainCall,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain_expanded(self, call, facts)
    }

    /// Build a Doc for call arguments with standard forced expansion
    ///
    /// Always uses `(\n  args,\n)` form, never arrow-hugging `(sig =>\n  body,\n)`.
    pub(super) fn build_call_args_doc_for_chain_standard_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        facts: ChainCall,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain_standard_expanded(self, call, facts)
    }
}

/// The window `prepend_removed_paren_comments` claims at a chain's head: the leading
/// region the chain's own nodes do NOT claim.
///
/// The whole region is `[chain start, base start)` — the stripped grouping parens the
/// author wrapped the chain in, and any comment among them. It is claimed by two
/// emitters and they must PARTITION it (`docs/comments.md` hazard 3):
///
/// - A member whose gap linearization widened back over a stripped `(` claims that
///   paren's own prefix, `[member start, its object's start)` — prettier relocates a
///   comment written just inside such a `(` to just before that member.
/// - The head takes the REST, which is everything from the innermost widened paren's
///   object inward.
///
/// So the window's `start` is the innermost widened claim's END (the largest
/// [`chain::ChainNode::paren_gap_skip`] start, since the parens nest), not the chain's own
/// start — reading it as the chain's start hands the head a region a member already
/// claimed, and stopping the head at the widened claim's START (what this returned
/// before, as a single bound) handed it NOTHING while the member skipped the same
/// region: `((⟨⟩a).b).c(x)` had no emitter at all and DROPPED every comment there.
///
/// Only call chains widen at all — prettier places comments mid-chain only when the
/// chain has calls — so a member-only chain keeps the whole region at the head.
fn chain_head_comment_window(
    nodes: &[chain::ChainNode<'_>],
    expr: &internal::Expression<'_>,
    chain_start: u32,
) -> (u32, u32) {
    // A base that OWNS its leading gap (a sealed optional chain, an IIFE callee —
    // `ChainNode::Base::paren_leading_start`) emits it INSIDE the pair it prints, so
    // the claim stops at that `(` and this prepend has nothing left to take. Claiming
    // it here hoists the run out in front of a pair that survives, which is what both
    // positions used to do.
    if let Some(chain::ChainNode::Base {
        paren_leading_start: Some(start),
        ..
    }) = nodes.first()
    {
        return (chain_start, *start);
    }
    let base_start = get_chain_base_start(expr);
    // The innermost widened claim's end. `paren_gap_skip` is `Some` exactly on a node
    // linearization widened, and its start is that node's object's own span start —
    // never past the base, since the object contains it — so the max is a position in
    // `[chain_start, base_start]` and the window can't invert.
    let start = nodes
        .iter()
        .filter_map(chain::ChainNode::paren_gap_skip)
        .map(|skip| skip.start)
        .max()
        .unwrap_or(chain_start);
    (start, base_start)
}

/// Get the start position of the innermost base expression in a chain
fn get_chain_base_start(expr: &internal::Expression<'_>) -> u32 {
    match expr {
        internal::Expression::MemberExpression(member) => get_chain_base_start(member.object),
        internal::Expression::CallExpression(call) => get_chain_base_start(call.callee),
        internal::Expression::TSNonNullExpression(non_null) => {
            get_chain_base_start(non_null.expression)
        }
        // Note: TaggedTemplateExpression is NOT traversed here because its own
        // build_tagged_template_doc handles comments from removed parentheses
        _ => expr.span().start,
    }
}
