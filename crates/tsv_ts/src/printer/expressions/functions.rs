// Function expression printing for TypeScript
//
// This module handles function-related expressions:
// - Arrow function expressions (async, parameters, return types, body)
// - Function expressions (parameters, return types, body)
//
// Note: Block statements are in blocks.rs as a reusable utility

use crate::ast::internal;
use crate::printer::ArrowChainContext;
use crate::printer::class_common::ClassHeaderOptions;
use crate::printer::class_common::ClassTypeParamsGap;
use crate::printer::layout::hang_after_operator;
use crate::printer::needs_parens::leftmost_no_lookahead;
use crate::printer::statements::function::FunctionHeadModifier;
use crate::printer::types::helpers::is_huggable_type;
use crate::printer::{CommentSpacing, LeadingGlue};
use crate::printer::{
    CommentVec, ParenContext, Printer, is_multiline_template_expression, unwrap_parenthesized,
};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::doc::{DocBuf, GroupId};
use tsv_lang::source_scan::{find_char_skipping_comments, has_newline_before_position};

/// Check if an arrow body should stay on the same line as `=>` (no line break option).
///
/// Prettier's `mayBreakAfterShortPrefix` - these expression types stay hugged to `=>`:
/// - Object literals: `() => ({...})`
/// - Array literals: `() => [...]`
/// - Arrow functions: `() => () => ...`
/// - Block statements (handled separately)
/// - JSX elements (not yet supported)
///   When true, body uses `" " + body` (simple space).
///   When false, body uses `indent([line, body])` (can break to new line).
///
/// Note: Template literals are NOT included here — they need source-position-dependent
/// handling (hug when on same line as `=>`, break when on own line). That check is
/// done in the caller via `is_template_on_same_line` which has access to source text.
fn should_hug_arrow_body(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::ObjectExpression(_)
            | internal::Expression::ArrayExpression(_)
            | internal::Expression::ArrowFunctionExpression(_)
    )
}

/// Check if an expression is a multiline template literal on the same line as `=>`.
///
/// Prettier's `isTemplateOnItsOwnLine` — hug when the backtick is on the same line
/// as `=>` (no newline before it in source), break when the user placed it on its own line.
/// This creates dual-stable behavior: both forms are preserved.
fn is_template_on_same_line(source: &str, expr: &internal::Expression<'_>) -> bool {
    is_multiline_template_expression(expr)
        && !has_newline_before_position(source, expr.span().start)
}

/// Span of the ObjectExpression at an expression's leftmost (no-lookahead) position,
/// or `None` if the leftmost token isn't an object literal.
///
/// In arrow bodies, `{...}` at the start is ambiguous with a block statement and needs
/// parens around *just the object* — e.g. `{} as T`, `{}.prop`, `{} && a`, `{} ? a : b`,
/// `{}.b++`. Delegates to the shared `leftmost_no_lookahead` walk (prettier's
/// `startsWithNoLookaheadToken`, also used by the expression-statement paren path) and
/// keeps the result only when it's an object. Assignment and sequence *bodies* are
/// excluded — they get whole-body parens instead — matching prettier's
/// needs-parentheses.js arrow-body carve-out (which still recurses through *nested*
/// assignments/sequences, so the carve-out guards only the top-level body type).
///
/// Returns the object's span so the printer wraps exactly that node (keyed by span,
/// robust to a chain rebuilding its base across conditional-group variants) and never
/// a same-shaped object nested deeper.
fn leftmost_object_span(expr: &internal::Expression<'_>) -> Option<Span> {
    if matches!(
        expr,
        internal::Expression::AssignmentExpression(_) | internal::Expression::SequenceExpression(_)
    ) {
        return None;
    }
    match leftmost_no_lookahead(expr) {
        internal::Expression::ObjectExpression(o) => Some(o.span),
        _ => None,
    }
}

/// Prepend an optional leading comment run to a doc, allocating nothing when there is none.
///
/// One spelling, shared by [`Printer::build_arrow_body_doc_with_leading`] (which decides
/// which side of the body's parens the run belongs on) and by the one arm that emits its
/// paren tokens itself. The no-run case must stay node-free: this sits on the arrow-body
/// hot path.
#[inline]
fn prepend_leading(d: &DocArena, leading: Option<DocId>, doc: DocId) -> DocId {
    match leading {
        Some(run) => d.concat(&[run, doc]),
        None => doc,
    }
}

/// Whether an expression has an ObjectExpression at its leftmost position.
/// See [`leftmost_object_span`].
pub(in crate::printer) fn has_leftmost_object_expression(expr: &internal::Expression<'_>) -> bool {
    leftmost_object_span(expr).is_some()
}

/// Check if an expression is a huggable pattern for function parameters.
///
/// Prettier's `shouldHugFunctionParameters` hugs single object/array patterns,
/// keeping `({` and `}: Type)` together while letting the pattern's content break.
///
/// Shared with the signature-param path (`build_signature_params_doc`) so bodyless
/// declarations (declare / overload) and type-member signatures (method / call /
/// construct) hug the lone param exactly like value-param functions do.
pub(in crate::printer) fn is_huggable_pattern(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::ObjectPattern(_) | internal::Expression::ArrayPattern(_) => true,
        // Assignment pattern with object/array on left: `{a, b} = default`
        internal::Expression::AssignmentPattern(ap) => {
            matches!(
                ap.left,
                internal::Expression::ObjectPattern(_) | internal::Expression::ArrayPattern(_)
            )
        }
        _ => false,
    }
}

/// The offset just past an arrow's `=>` — where the gap before its body opens.
///
/// One spelling of that bound. Its readers had two: `arrow.arrow_token` raw at the
/// expand-last-arg hug gate, `arrow_token + "=>".len()` everywhere else. The `=>` token's own
/// bytes can hold no comment, so both ranges held the same comments and the drift stayed
/// invisible — which is the argument for not carrying two of them, not for tolerating it.
pub(in crate::printer) fn arrow_token_end(arrow: &internal::ArrowFunctionExpression<'_>) -> u32 {
    arrow.arrow_token + "=>".len() as u32
}

/// The start an arrow shares with its lone **unparenthesized** parameter — the one shape
/// where a node and its child begin at the same byte while the parent still prints ahead of
/// it (the `(` the printer synthesizes around that parameter).
///
/// `None` for every other arrow, the async paren-less one included (`async /* c */ x => x`):
/// its span starts at `async`, so the two positions differ and the comment is never the
/// arrow's to begin with — [`Printer::push_async_arrow_head`] prints it positionally.
///
/// Read only by [`Printer::build_arrow_params_doc_ungrouped`], to suppress the parameter's
/// duplicate owned-comment claim; see the ⚠️ there.
fn bare_param_sharing_arrow_start(arrow: &internal::ArrowFunctionExpression<'_>) -> Option<u32> {
    if arrow.params_start.is_some() {
        return None;
    }
    let start = arrow.params.first()?.span().start;
    (start == arrow.span.start).then_some(start)
}

/// Whether `arrow`'s signature — its params' `(` through its `=>` — holds a comment that
/// **forces** a break.
///
/// The expand-last-arg hug renders that signature on one line, which such a comment makes
/// impossible: the break is forced out mid-hug (`map(([/* a⏎b */ x, y]) => …`) instead of
/// breaking the call, which is neither layout anyone asked for.
///
/// Prettier reaches the same conclusion by another route: the hug state's hard line
/// propagates a break to its enclosing `conditionalGroup`, whose printer then jumps
/// straight to the most-expanded state (`call-arguments.js`, `printer.js`'s `doc.break`
/// arm). tsv has no break-propagation pass — `should_break` is only ever set explicitly —
/// so the question is asked here instead, of the **source**, before either hug path builds
/// anything. Asking pre-build is also what keeps this off the exponential-rebuild rake: the
/// alternative, building the arrow and inspecting `will_break`, needs a second build of the
/// same subtree on the fall-through.
///
/// Both hug paths (`call_formatting.rs`'s plain call, `chain_args.rs`'s member chain) route
/// through this one predicate — the two disagreeing about when a hug is legal is exactly
/// how the mangle survived on `arr.map(…)` while `fn(…)` was fine.
///
/// A **single-line** block comment is deliberately absent: `fn(([/* c */ x, y]) => [y, x])`
/// hugs, in tsv and prettier alike, because it inlines without forcing anything.
pub(in crate::printer) fn arrow_signature_has_breaking_comments(
    printer: &Printer<'_>,
    arrow: &internal::ArrowFunctionExpression<'_>,
) -> bool {
    // `arrow_token` is the `=>`, captured by the parser for exactly this kind of question.
    // An unparenthesized `x => …` has no `(`, so the signature starts at the arrow node
    // itself — which still catches `x /* a⏎b */ => y`.
    let start = arrow.params_start.unwrap_or(arrow.span.start);
    let end = arrow.arrow_token;
    printer.has_line_comments_between(start, end)
        || printer.has_multiline_block_comments_on_page_between(start, end)
}

/// Check if an expression has a huggable type annotation.
///
/// Parameters with huggable type annotations like `a?: { b: T }` should be hugged:
/// - The opening `{` stays on the same line as the parameter name
/// - The content expands internally
/// - The closing `}` comes on its own line
///
/// This matches Prettier's behavior where `fn(a?: {` stays together.
///
/// NOTE: Does NOT recurse into AssignmentPattern. When a param has a default value
/// like `a: { b: T } = {}`, the `= {}` prevents hugging — prettier breaks the
/// param list instead. Destructuring patterns with defaults (`{a, b} = {}`) are
/// handled separately by `is_huggable_pattern`.
///
/// Shared with the signature-param path (`build_signature_params_doc`) — see
/// `is_huggable_pattern`.
pub(in crate::printer) fn has_huggable_type_annotation(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::Identifier(id) => id
            .type_annotation()
            .is_some_and(|ann| is_huggable_type(ann.type_annotation)),
        _ => false,
    }
}

impl<'a> Printer<'a> {
    /// Build a doc with `context` active so the outermost curried arrow chain in
    /// it picks the right flattened layout. The arrow printer consumes the
    /// context at entry (`replace(None)`); restoring the prior value here keeps it
    /// from leaking to a sibling argument / operand / RHS. Mirrors prettier
    /// routing the parent context into `printArrowFunctionSignatures`.
    pub(in crate::printer) fn build_with_arrow_chain_context(
        &self,
        context: ArrowChainContext,
        build: impl FnOnce() -> DocId,
    ) -> DocId {
        let prev = self.arrow_chain_context.replace(context);
        let doc = build();
        self.arrow_chain_context.set(prev);
        doc
    }

    /// Run `build` with the curried-typed-arrow flag set to `value`, restoring the
    /// prior value afterward. Mirrors `build_with_arrow_chain_context`: the flag is
    /// per-chain layout state that must not leak to sibling arrows nested inside the
    /// body (callbacks, object-property arrows). The arms that set it to the value
    /// it already holds rely on the restore returning that same value.
    fn build_with_in_curried(&self, value: bool, build: impl FnOnce() -> DocId) -> DocId {
        let prev = self.in_curried_typed_arrow.replace(value);
        let doc = build();
        self.in_curried_typed_arrow.set(prev);
        doc
    }

    /// Print an arrow function expression using doc-based formatting with width-aware wrapping.
    ///
    /// Prettier behavior for arrow functions:
    /// Build a Doc for an arrow function with width-aware wrapping.
    ///
    /// Prettier's algorithm (from arrow-function.js and function-parameters.js):
    /// 1. Type params are wrapped in their OWN group - they break independently
    /// 2. Function params are NOT in their own group - just softlines
    /// 3. The whole signature (type params group + params + return type) is wrapped in a group
    /// 4. When the signature group breaks, params break but type params may stay flat
    ///
    /// Structure:
    /// ```text
    /// group([
    ///     group(type_params),  // inner group - breaks independently
    ///     "(", indent([softline, params...]), ifBreak(","), softline, ")",
    ///     return_type,
    ///     " =>"
    /// ])
    /// " " + body
    /// ```
    fn build_arrow_doc_wrapping(&self, arrow: &internal::ArrowFunctionExpression<'_>) -> DocId {
        // Consume the chain context (set by the enclosing assignment / call-arg /
        // binary-operand printer) so only the outermost chain arrow uses it;
        // nested arrows in the chain reset to the default layout.
        let chain_context = self.arrow_chain_context.replace(ArrowChainContext::None);
        if self.should_use_arrow_chain_layout(arrow, chain_context) {
            return self.build_arrow_chain_doc(arrow, chain_context);
        }

        let d = self.d();
        let mut parts = DocBuf::new();

        // The `=>` token position (parser-recorded) distinguishes:
        // - Comments between the signature and `=>` → print BEFORE `=>`
        // - Comments between `=>` and body → print AFTER `=>`
        let arrow_end = arrow_token_end(arrow);

        // Build the signature (async + type params + params + return type) via the
        // shared builder, then append any comment between the signature and `=>`
        // (`(x) /* c */ =>`) — the seam the call-argument states share.
        let sig_doc = self.append_pre_arrow_comments(arrow, self.build_arrow_signature_doc(arrow));

        // Wrap entire signature in a group. In expand-last-arg context, render the
        // signature flat (remove_lines) so the params can't break — prettier's
        // `expandLastArg` path prints params with `removeLines`, which is what lets a
        // force-broken arrow keep its destructuring param inline and fall through to
        // the all-args-broken-out layout instead of shattering the param.
        if self.expand_last_arg_flat_params.get() {
            parts.push(d.remove_lines(sig_doc));
        } else {
            parts.push(d.group(sig_doc));
        }

        // " =>" outside the sig group so fits() look-ahead sees it as text
        // consuming remaining width. When sig + " =>" = print_width, the " =>"
        // leaves remaining=0 for the body — ternary body's " " text then
        // pushes to -1 and forces the sig group to break params.
        parts.push(d.text(" =>"));

        // Body: expression bodies can break to a new line with indent; block bodies
        // stay hugged to `=>`.
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                self.build_arrow_expression_body(&mut parts, expr, arrow, arrow_end);
            }
            internal::ArrowFunctionBody::BlockStatement(block) => {
                self.build_arrow_block_body(&mut parts, block, arrow_end);
            }
        }

        d.concat(&parts)
    }

    /// Emit the body of an arrow with an expression body (the `=>` already pushed)
    /// into `parts`. Branches on whether the body hugs `=>` (object/array/template),
    /// hangs on the next line, joins a curried chain, or carries comments — mirroring
    /// prettier's `shouldPutBodyOnSameLine` / `shouldAddParensIfNotBreak` cascade.
    fn build_arrow_expression_body(
        &self,
        parts: &mut DocBuf,
        expr: &internal::Expression<'_>,
        arrow: &internal::ArrowFunctionExpression<'_>,
        arrow_end: u32,
    ) {
        let d = self.d();

        // Check for trailing comments from stripped grouping parens.
        // When the parser strips parens from `() => (x /* c */)`, comments
        // between body expr end and arrow span end are lost. Re-add parens
        // to preserve them, matching the unary expression approach.
        let body_end = expr.span().end;
        let has_trailing_paren_comments =
            self.has_trailing_paren_comments(body_end, arrow.span.end);

        if has_trailing_paren_comments {
            parts.push(d.text(" "));
            // Leading comments between `=>` and body (if any):
            // `() => /* lead */ (x /* trail */)` — emit inline leading,
            // then paren-wrapped body with trailing.
            let body_start = expr.span().start;
            if self.has_comments_to_emit_between(arrow_end, body_start) {
                for comment in comments_to_emit_in_range(self.comments, arrow_end, body_start) {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
            }
            parts.push(self.build_expression_doc_keep_paren_comments(expr, arrow.span.end));
            // Skip normal body handling — paren wrapping covers all cases
            return;
        }

        // Check for comments between `=>` and body start
        // These are comments like: `() => /* comment */ expr`
        let body_start = expr.span().start;
        let has_post_arrow_comments = self.has_comments_to_emit_between(arrow_end, body_start);

        // Prettier's `hasLeadingOwnLineComment`: checks if any comment
        // between `=>` and body has a newline after it. Inline block
        // comments like `=> /* c */ expr` return false (body stays hugged),
        // while own-line comments return true (body breaks).
        let has_own_line_comment =
            has_post_arrow_comments && self.has_own_line_post_arrow_comment(arrow_end, body_start);

        // Prettier's `shouldPutBodyOnSameLine`: certain expression types stay hugged to =>
        // Object/array literals always hug.
        // Nested arrows hug ONLY when outer has no return type annotation.
        // With return type: const f = (x: T): H => (y) => expr; // breaks
        // Without:          const f = (x: T) => (y) => expr;    // hugs
        let is_arrow_body = matches!(expr, internal::Expression::ArrowFunctionExpression(_));

        // Check if this is a curried arrow where ANY arrow triggers chain breaking.
        // Triggers: return type with params, type parameters, non-identifier params.
        // Skip when skip_arrow_chain is set (call arg expand-last context) — prettier's
        // shouldPrintAsChain is false when expandLastArg is true, so chain detection
        // is disabled and the body is hugged.
        let chain_has_return_type = is_arrow_body
            && !self.skip_arrow_chain.get()
            && crate::printer::arrow_chain_has_return_type(arrow);

        // Check if body arrow has trailing param comments (forces break)
        let body_arrow_has_trailing_param_comments = matches!(
            expr,
            internal::Expression::ArrowFunctionExpression(body_arrow)
                if self.arrow_has_trailing_param_comments(body_arrow)
        );

        // Inline block comments don't prevent hugging — only own-line comments do.
        // `() => /* comment */ ({...})` hugs (inline block comment)
        // `() =>\n  /* comment */\n  ({...})` breaks (own-line comment)
        let should_hug = !has_own_line_comment
            && (should_hug_arrow_body(expr) || is_template_on_same_line(self.source, expr))
            && !chain_has_return_type
            && !body_arrow_has_trailing_param_comments;

        // The curried-chain and parenthesized-ternary arms below *reassemble* the body —
        // they build its doc and wrap it themselves — so nothing on those routes emits the
        // `=>`→body gap and the comment is dropped outright (`(x) => /* c */ (a ? b : c)`).
        // The run is handed to the body (`build_arrow_body_doc_with_leading`), which is the
        // single place that decides which side of the body's parens it belongs on: inside a
        // ternary's `if_break` layout parens, outside every required one. That is where the
        // call-argument routes already put it (`prepend_arrow_body_comments`) and where
        // prettier puts it — the ternary's parens are a layout artifact that vanishes when
        // the body breaks, so a position outside them exists in only one rendering.
        //
        // Own-line comments took the first arm, so the gap holds nothing but inline blocks
        // and takes the same emitter the hug and normal arms use. The lookup is the *emit*
        // axis, which skips owned comments: one glued to the body's first token rides the
        // body's own doc and still prints exactly once.
        // Lazy: only the four arms below read it. The own-line arm always has
        // `has_post_arrow_comments` set (it is a conjunct of `has_own_line_comment`) and the
        // hug and normal arms emit the run themselves, so building it up front would leave a
        // dead comments doc in the arena on every commented arrow that isn't one of the four.
        let gap_run = || {
            has_post_arrow_comments
                .then(|| self.build_inline_post_arrow_comments_doc(arrow_end, body_start))
        };

        if has_own_line_comment {
            // Own-line or line comments — always break
            let body_with_comments =
                self.build_arrow_body_with_comments_doc(expr, arrow_end, body_start);
            parts.push(hang_after_operator(d, body_with_comments));
        } else if should_hug {
            // Hugged body (possibly with inline block comments):
            // `() => ({...})` or `() => /* c */ ({...})`
            parts.push(d.text(" "));
            if has_post_arrow_comments {
                parts.push(self.build_inline_post_arrow_comments_doc(arrow_end, body_start));
            }
            parts.push(self.build_arrow_body_doc(expr));
        } else if is_arrow_body && (chain_has_return_type || self.in_curried_typed_arrow.get()) {
            // Curried arrow chain - all arrows break without indent so they align:
            // const f = (x: T): H => (y) => expr   // outer has return type
            // const f = (x: T) => (y): H => expr   // inner has return type
            // becomes:
            // const f =
            //     (x: T): H =>      or      (x: T) =>
            //     (y) =>                    (y): H =>
            //         expr                      expr
            //
            // The flag is already set when reached via `in_curried_typed_arrow`, so
            // unconditionally setting it `true` for the body build is equivalent.
            let body_doc = self.build_with_in_curried(true, || {
                self.build_arrow_body_doc_with_leading(expr, gap_run())
            });
            parts.push(d.concat(&[d.hardline(), body_doc]));
        } else if is_arrow_body && body_arrow_has_trailing_param_comments {
            // Nested arrow with trailing param comments - first level gets indent,
            // subsequent levels align (use curried pattern)
            // (a, // c) => (b, // c) => {}
            // becomes:
            // (a, // c) =>
            //     (b, // c) =>
            //     (c, // c) => {}
            let body_doc = self.build_with_in_curried(true, || {
                self.build_arrow_body_doc_with_leading(expr, gap_run())
            });
            parts.push(d.indent_hardline(body_doc));
        } else if self.in_curried_typed_arrow.get() {
            // Innermost arrow in curried chain - body is NOT another arrow.
            // This needs indent since it's the final expression.
            // Reset flag so arrows inside the body (e.g. callback args) aren't
            // treated as part of the curried chain; restore to `true` (its value on
            // entry, since this arm is reached only when the flag is set) afterward.
            let body_doc = self.build_with_in_curried(false, || {
                self.build_arrow_body_doc_with_leading(expr, gap_run())
            });
            parts.push(d.indent_hardline(body_doc));
        } else if matches!(expr, internal::Expression::ConditionalExpression(_))
            && !has_leftmost_object_expression(expr)
        {
            // Prettier's shouldAddParensIfNotBreak: ternary body gets conditional
            // parens when inline, no parens when on its own line.
            // Excludes ternaries whose test starts with ObjectExpression (matches
            // Prettier's startsWithNoLookaheadToken check) — those fall through
            // to the normal path which calls build_arrow_body_doc for object parens.
            //
            // Structure: [" ", group([ifBreak("","("), indent([softline, body]),
            //                         ifBreak("",")")])]
            //
            // The " " TEXT element before the group is critical for fits() boundary:
            // when sig + " =>" = exactly print_width, remaining=0. The " " consumes
            // 1 char (→ -1), making the sig group fail fits() and break params.
            // With the old group(indent(line, body)), line() in Break mode would
            // short-circuit fits() to return true, keeping the sig flat.
            //
            // Flat:  ` => (cond ? a : b)` — parens, same line
            // Break: ` =>\n\tcond ? a : b` — no parens, next line
            //
            // Expand-last-arg body reuse: the multi-arg conditional-body break state builds
            // this same ternary separately; reuse the pre-built DocId (span-keyed) to avoid an
            // O(2^depth) double build when the ternary recurses. See the `arrow_body_inject` field.
            let body_doc = if let Some((span, doc)) = self.arrow_body_inject.get()
                && span == expr.span().start
            {
                doc
            } else {
                self.build_expression_doc(expr)
            };
            // This arm emits the paren tokens itself rather than going through
            // `build_arrow_body_doc`, so it prepends the run on its own seam — the same
            // side of the parens that helper picks for a ternary, i.e. inside.
            // `will_break` asks about the BODY, so it reads the raw doc.
            let with_leading = prepend_leading(d, gap_run(), body_doc);
            if d.will_break(body_doc) {
                // Body has hardlines (multiline template in ternary, etc.)
                // Use normal break layout — no parens needed
                parts.push(hang_after_operator(d, with_leading));
            } else {
                parts.push(d.text(" "));
                parts.push(d.group(d.concat(&[
                    d.if_break(d.empty(), d.text("(")),
                    d.indent(d.concat(&[d.softline(), with_leading])),
                    d.if_break(d.empty(), d.text(")")),
                ])));
            }
        } else {
            // Normal expression: can break after => with indentation
            // Short: (x) => x + 1
            // Long:  (veryLongParams) =>
            //            veryLongExpr
            //
            // The body is wrapped in a group so it can make its own fits() decision.
            // This allows the arrow body to stay inline even when the parent element
            // is in break mode, as long as the body content fits from its position.
            //
            // Normal expression body: can break after => with indentation.
            // Template literal bodies with literalline nodes will propagate
            // breaks naturally, enabling chain/call expansion decisions.
            let body_doc = self.build_arrow_body_doc(expr);
            if has_post_arrow_comments {
                // Inline block comments before non-huggable body:
                // `() => /* comment */ a + b`
                let comments_doc = self.build_inline_post_arrow_comments_doc(arrow_end, body_start);
                parts.push(hang_after_operator(d, d.concat(&[comments_doc, body_doc])));
            } else {
                parts.push(hang_after_operator(d, body_doc));
            }
        }
    }

    /// Emit the body of an arrow with a block-statement body (the `=>` already
    /// pushed) into `parts`. A block body always stays hugged to `=>` and
    /// terminates any curried-arrow chain.
    fn build_arrow_block_body(
        &self,
        parts: &mut DocBuf,
        block: &internal::BlockStatement<'_>,
        arrow_end: u32,
    ) {
        let d = self.d();

        // Block body: always stays hugged to => (no break)
        // (params) => {
        //     ...
        // }
        // Check for comments between `=>` and body start
        let body_start = block.span.start;
        let has_post_arrow_comments = self.has_comments_to_emit_between(arrow_end, body_start);

        // A block body terminates any curried-arrow chain — arrows nested
        // inside it (callbacks, object-property arrows) are NOT part of the
        // chain, so clear the flag so they aren't force-broken after `=>`.
        // Mirrors the innermost expression-body case above.
        let block_doc = self.build_with_in_curried(false, || self.build_block_statement_doc(block));

        // A line comment (or own-line block comment) between `=>` and the block
        // body must break so the comment sits on its own line and the `{` drops
        // to the next line — emitting it inline would let the `//` run to
        // end-of-line and swallow the `{`, dropping the whole block body
        // (non-idempotent, non-reparseable). Mirrors the expression-body path
        // (`build_arrow_body_with_comments_doc`).
        if has_post_arrow_comments && self.has_own_line_post_arrow_comment(arrow_end, body_start) {
            let mut body_parts: DocBuf = DocBuf::new();
            self.push_leading_comment_run(
                &mut body_parts,
                comments_to_emit_in_range(self.comments, arrow_end, body_start),
                body_start,
                LeadingGlue::Adjacent,
                d.empty(),
            );
            body_parts.push(block_doc);
            parts.push(hang_after_operator(d, d.concat(&body_parts)));
            return;
        }

        // Otherwise the block stays hugged to `=>`; an inline block comment
        // (`=> /* c */ {}`) sits between and can't swallow the brace.
        if has_post_arrow_comments {
            let mut comment_parts: DocBuf = DocBuf::new();
            for comment in comments_to_emit_in_range(self.comments, arrow_end, body_start) {
                comment_parts.push(d.text(" "));
                comment_parts.push(self.build_comment_doc(comment));
            }
            parts.push(d.concat(&comment_parts));
        }

        parts.push(d.text(" "));
        parts.push(block_doc);
    }

    /// Whether to render an arrow as a flattened curried chain (prettier's
    /// `printArrowFunctionSignatures`). Covers the untyped assignment-RHS and
    /// call-arg/binaryish contexts: the body must be another arrow, the chain
    /// must carry no return type / type params / non-identifier param (those
    /// route through the existing break-after-operator path), and there must be
    /// no comments in the heads region (which the existing path owns). A `None`
    /// context (no enclosing chain site) or the call-arg expand-last-arg path
    /// (`skip_arrow_chain`) routes to the default arrow layout.
    fn should_use_arrow_chain_layout(
        &self,
        arrow: &internal::ArrowFunctionExpression<'_>,
        context: ArrowChainContext,
    ) -> bool {
        if context == ArrowChainContext::None || self.skip_arrow_chain.get() {
            return false;
        }
        let body_is_arrow = matches!(
            &arrow.body,
            internal::ArrowFunctionBody::Expression(b)
                if matches!(b, internal::Expression::ArrowFunctionExpression(_))
        );
        if !body_is_arrow {
            return false;
        }
        if crate::printer::arrow_chain_has_return_type(arrow) {
            return false;
        }
        // Any comment anywhere in the chain (heads, between `=>`s, around the body,
        // or trailing a stripped grouping paren) routes to the existing path, which
        // owns the chain's comment handling.
        !self.has_comments_to_emit_between(arrow.span.start, arrow.span.end)
    }

    /// Build a flattened curried arrow chain: the signature heads
    /// (`(a) => (b) => …`) form a breakable group keyed on `GroupId::ArrowChain`,
    /// so they stay on one line when they fit and break otherwise. The terminal
    /// arrow's `=>` is emitted after the group so the body hugs the last head; a
    /// hugging body (object/array/template/block) stays inline, others hang on
    /// the next line.
    ///
    /// Mirrors prettier's `printArrowFunction`. The heads' shape depends on the
    /// parent context (`printArrowFunctionSignatures` branches):
    /// - `AssignmentRhs`: all heads join in one group indented one level after
    ///   `=` (the leading softline is the break-after-`=`); when they break, each
    ///   head shares the same indent. The break-after-`=` decision is supplied by
    ///   the enclosing fluid assignment layout (`choose_layout` routes untyped
    ///   chains to `Fluid`).
    /// - `CallArgOrBinaryish`: progressive indent — the first head stays on the
    ///   line, the rest indent one level (`group([sig0, " =>", indent([line,
    ///   join([" =>", line], rest)])])`).
    fn build_arrow_chain_doc(
        &self,
        head: &internal::ArrowFunctionExpression<'_>,
        context: ArrowChainContext,
    ) -> DocId {
        let d = self.d();

        // Walk the chain, collecting each arrow's signature, until the terminal
        // (non-arrow) body.
        let mut sig_docs: DocBuf = DocBuf::new();
        let mut current = head;
        let mut is_head = true;
        let terminal: &internal::ArrowFunctionBody<'_> = loop {
            // Each signature is its own group so its params break independently of
            // the chain (prettier wraps each `printArrowFunctionSignature` in a
            // group): when the heads break onto separate lines, the params stay
            // flat unless a single signature genuinely overflows.
            let sig = d.group(self.build_arrow_signature_doc(current));
            // An INNER arrow is built from its signature here and never routed through
            // `build_expression_doc`, so this is the only place its owned leading comment
            // can be claimed — otherwise it is dropped. The head is not: this whole chain
            // doc is what `build_expression_doc` wraps, and it claims the head's comment
            // there, so claiming it again here would print it twice.
            let sig = if is_head {
                sig
            } else {
                self.prepend_owned_leading_comment_at(current.span.start, sig)
            };
            is_head = false;
            sig_docs.push(sig);
            match &current.body {
                internal::ArrowFunctionBody::Expression(b) => {
                    if let internal::Expression::ArrowFunctionExpression(inner) = b {
                        current = inner;
                    } else {
                        break &current.body;
                    }
                }
                internal::ArrowFunctionBody::BlockStatement(_) => break &current.body,
            }
        };

        // The heads group is keyed on `GroupId::ArrowChain`; its shape depends on
        // the parent context. Either way the terminal `=>` + body are emitted
        // after the group (below), and `indent_if_break` ties the body's indent
        // to this group's break decision.
        let sep = d.concat(&[d.text(" =>"), d.line()]);
        let heads = match context {
            // Assignment-RHS: the inner group joins ALL heads with ` =>` + line so
            // they stay on one line when they fit and each drop to their own line
            // otherwise; the outer group wraps `indent([softline, inner])`, so
            // when the chain doesn't fit on the `=` line its leading softline
            // breaks (newline after `=`) and indents the heads one level. The
            // enclosing fluid assignment marker stays flat — the break-after-`=`
            // is this softline.
            ArrowChainContext::AssignmentRhs => {
                let inner = d.group(d.join_doc(sig_docs, sep));
                d.group_with_id(
                    d.indent(d.concat(&[d.softline(), inner])),
                    GroupId::ArrowChain,
                )
            }
            // Call-arg/binaryish: progressive indent. The first head stays on the
            // current line; the rest indent one level and each drop to their own
            // line when the group breaks. Mirrors prettier's
            // `group([sig0, " =>", indent([line, join([" =>", line], rest)])])`.
            // (`None` is unreachable — `should_use_arrow_chain_layout` gates it —
            // but falls back to this progressive shape.)
            ArrowChainContext::CallArgOrBinaryish | ArrowChainContext::None => {
                // `split_first` is always `Some` here — a curried chain has ≥2
                // heads — but matching avoids a panic path; the `None` arm falls
                // back to the assignment-style joined group.
                match sig_docs.split_first() {
                    Some((&sig0, rest)) => {
                        let rest_joined = d.join_doc(rest.iter().copied(), sep);
                        d.group_with_id(
                            d.concat(&[
                                sig0,
                                d.text(" =>"),
                                d.indent(d.concat(&[d.line(), rest_joined])),
                            ]),
                            GroupId::ArrowChain,
                        )
                    }
                    None => d.group_with_id(d.join_doc(sig_docs, sep), GroupId::ArrowChain),
                }
            }
        };

        // The terminal body (`=> body`) is wrapped in `indent_if_break` keyed on
        // the heads group: when the heads broke onto their own indented lines, the
        // body sits at the heads' indent level too (so a block/object body's own
        // content lands one level deeper); when the heads stayed on the `=` line,
        // the body keeps the base indent. Mirrors prettier's
        // `indentIfBreak(bodyDoc, { groupId: chainGroupId })`.
        let body_part = match terminal {
            internal::ArrowFunctionBody::Expression(b) => {
                let expr = b;
                if should_hug_arrow_body(expr) || is_template_on_same_line(self.source, expr) {
                    // Object/array/template body: hugs the last head, supplies its
                    // own internal indent.
                    d.concat(&[d.text(" "), self.build_arrow_body_doc(expr)])
                } else if matches!(expr, internal::Expression::ConditionalExpression(_))
                    && !has_leftmost_object_expression(expr)
                {
                    // Ternary body: parens when inline, none when broken
                    // (prettier's shouldAddParensIfNotBreak).
                    let body_doc = self.build_expression_doc(expr);
                    if d.will_break(body_doc) {
                        // No own group — the body's line is governed by the outer
                        // chain group below (prettier's `indent([line, bodyDoc])`).
                        d.indent_line(body_doc)
                    } else {
                        d.concat(&[
                            d.text(" "),
                            d.group(d.concat(&[
                                d.if_break(d.empty(), d.text("(")),
                                d.indent(d.concat(&[d.softline(), body_doc])),
                                d.if_break(d.empty(), d.text(")")),
                            ])),
                        ])
                    }
                } else {
                    // Other expression body: hang on the next line when the chain
                    // breaks. No own group — the body's `line` is governed by the
                    // outer chain group below, so the body hangs whenever the heads
                    // break (matching prettier's `indent([line, bodyDoc])` inside
                    // the outer `group([…])`), not on an independent fit check.
                    d.indent_line(self.build_arrow_body_doc(expr))
                }
            }
            internal::ArrowFunctionBody::BlockStatement(block) => {
                d.concat(&[d.text(" "), self.build_block_statement_doc(block)])
            }
        };

        // Outer group, mirroring prettier's `printArrowFunction` return
        // (`group([group(signaturesDoc, {id}), " =>", indentIfBreak(bodyDoc)])`).
        // The body's hanging `line` is governed by THIS group, so a non-hugging
        // body hangs whenever the chain doesn't fit — even when the body itself is
        // short — while the nested heads group makes its own break decision.
        d.group(d.concat(&[
            heads,
            d.text(" =>"),
            d.indent_if_break(body_part, GroupId::ArrowChain),
        ]))
    }

    /// Build doc for return type annotation in arrow function context
    /// Union return types get special handling when the signature breaks:
    ///
    /// Flat: (): A | B | C =>
    /// Break: ):
    ///            | A
    ///            | B
    ///            | C =>
    ///
    /// Function types as return types get wrapped in parentheses for disambiguation:
    /// `(x: T): ((y: T) => U) =>` not `(x: T): (y: T) => U =>`
    fn build_arrow_return_type_doc(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
        params_start: Option<u32>,
    ) -> DocId {
        let d = self.d();
        // One depth-tracked close-`)` scan feeds both `)`→`:` questions below.
        let close_paren_after = self.return_type_close_paren(params_start, annotation.span.start);
        // An alone-on-line format-ignore directive in the `)`→`:` gap freezes the whole
        // `: type` annotation and keeps its own line — before the function-type paren
        // wrapping below, which would rebuild a frozen type from parts.
        if let Some(frozen) = self.build_frozen_return_type_doc(close_paren_after, annotation) {
            return frozen;
        }
        // Preserve a block comment between `)` and the return type `:`
        // (`(x) /* c */ : T => ...`); prettier adds a space before `:`.
        let comment_prefix = self
            .build_close_paren_to_return_type_comments(close_paren_after, annotation.span.start);

        // Function types need parentheses to disambiguate from the arrow's `=>`
        // Example: `(x: T): ((y: T) => U) =>` not `(x: T): (y: T) => U =>`
        // Unwrap any explicit parenthesized types to check the inner type
        let inner_type = unwrap_parenthesized(annotation.type_annotation);
        if matches!(inner_type, internal::TSType::Function(_)) {
            let type_doc = self.build_type_doc(inner_type);
            return d.concat(&[comment_prefix, d.text(": ("), type_doc, d.text(")")]);
        }

        // Use return type version - only wraps for complex type args (unions/intersections)
        // Simple cases like Promise<void> let params break first
        d.concat(&[
            comment_prefix,
            self.build_type_annotation_doc_for_return_type(annotation),
        ])
    }

    /// Build doc for arrow params NOT in their own group (outer signature group controls breaking)
    ///
    /// Structure matches prettier's function-parameters.js:
    /// `[typeParams, "(", indent([softline, ...params]), ifBreak(","), softline, ")"]`
    ///
    /// ⚠️ **A paren-less arrow's parameter starts where the ARROW does**, so the arrow's
    /// owned leading comment answers the parameter's position-keyed lookup too and both
    /// claimed it — `/* c */ x => x` printed as `/* c */ (/* c */ x) => x`, at every
    /// position an arrow reaches. The claim belongs to the arrow: the `(` here is
    /// *synthesized*, so pushing the claim down would move the comment inside a paren the
    /// author never wrote (prettier keeps it outside too — `/* c */ (x) => x`). The
    /// parameter declines through [`Self::with_owned_comment_claimed_above`], whose mark is
    /// scoped to this list so a nested arrow in the body still claims its own.
    fn build_arrow_params_doc_ungrouped(
        &self,
        arrow: &internal::ArrowFunctionExpression<'_>,
    ) -> DocId {
        // The trailing boundary stops at `)`, not at the return type or the body:
        // comments between `)` and `=>` belong to the arrow printer
        // ([`Self::append_pre_arrow_comments`]).
        let build = || {
            self.build_params_doc_with_comments(
                arrow.params,
                arrow.params_start,
                self.arrow_params_end(arrow),
            )
        };
        match bare_param_sharing_arrow_start(arrow) {
            Some(start) => self.with_owned_comment_claimed_above(start, build),
            None => build(),
        }
    }

    /// Emit an async arrow's `async` keyword and the gap between it and the head of
    /// its parameters — the type-parameter `<`, the params `(`, or a bare single
    /// parameter's own start.
    ///
    /// A comment there is kept where the author wrote it (prettier relocates it into
    /// the body, into the parameter list, or not at all, keyed on which of the three
    /// heads follows — see `docs/conformance_prettier_ts_comments.md` §Comment
    /// relocation). Nothing else prints the gap, so an emitter here is what keeps the
    /// comment from being dropped.
    ///
    /// Only a **single-line block** comment can occupy the gap: every other kind puts
    /// a line terminator between `async` and the parameters, which
    /// `AsyncArrowFunction : async [no LineTerminator here] ArrowFormalParameters`
    /// forbids and the parser rejects. The shared keyword-gap emitter handles the
    /// line-comment shapes anyway rather than assuming that stays true.
    ///
    /// The gap is scanned from the arrow's own span start rather than from a computed
    /// end of the `async` token: the keyword cannot contain a comment, so the two
    /// ranges hold the same comments and this one needs no arithmetic on a keyword
    /// position. (An async arrow's span starts at `async` — the `<T>async (…)` graft
    /// acorn admits is a type assertion to tsv and does not parse.)
    pub(crate) fn push_async_arrow_head(
        &self,
        parts: &mut DocBuf,
        arrow: &internal::ArrowFunctionExpression<'_>,
    ) {
        if !arrow.r#async {
            return;
        }
        let d = self.d();
        parts.push(d.text("async"));
        let head_start = arrow
            .type_parameters
            .as_ref()
            .map(|tp| tp.span.start)
            .or(arrow.params_start)
            .or_else(|| arrow.params.first().map(|p| p.span().start))
            .unwrap_or_else(|| arrow.body.span().start);
        parts.push(self.build_keyword_to_name_comments(arrow.span.start, head_start));
    }

    /// Where an arrow's **parameter list** ends — just past its `)`, or, for a lone
    /// unparenthesized parameter, at that parameter's own end.
    ///
    /// **The one spelling of that bound**, shared by the three readers that must agree on
    /// it: the parameter-list emitter ([`Self::build_arrow_params_doc_ungrouped`]'s
    /// `trailing_comments_end`, and through it the per-parameter
    /// [`Self::param_trailing_end`]), the force-break gate over that emitter
    /// ([`Self::arrow_has_trailing_param_comments`]), and the signature end
    /// ([`Self::arrow_signature_end`]). A second derivation is how the gate and the emitter
    /// come to disagree about which comments are in the list — the gate opening a list for
    /// a comment no emitter there claims, or the pre-`=>` emitter reaching back over one
    /// the list already printed.
    ///
    /// `None` only for a shape the grammar does not produce — a parenthesized list whose
    /// `)` cannot be found, or no parens and no parameter — which is why the readers that
    /// need a concrete offset supply their own floor rather than this returning one.
    fn arrow_params_end(&self, arrow: &internal::ArrowFunctionExpression<'_>) -> Option<u32> {
        match arrow.params_start {
            // Find closing `)` to get accurate boundary
            Some(params_start) => self.find_closing_paren(params_start, arrow.body.span().start),
            // No parens (single param arrow like `x => x`) - use param end
            None => arrow.params.last().map(|p| p.span().end),
        }
    }

    /// Where an arrow's **signature** ends — after its return type when it has one,
    /// otherwise after its parameter list. The start of the gap before `=>`.
    fn arrow_signature_end(&self, arrow: &internal::ArrowFunctionExpression<'_>) -> u32 {
        if let Some(rt) = &arrow.return_type {
            return rt.span.end;
        }
        // The floor is reached only on the shape [`Self::arrow_params_end`] calls out, and
        // it still lands past the parameter list either way — so the pre-`=>` emitter can
        // never reach back over a parameter's own comment and print it a second time.
        self.arrow_params_end(arrow).unwrap_or_else(|| {
            arrow
                .params_start
                .map_or(arrow.span.start, |_| arrow.body.span().start)
        })
    }

    /// Whether a comment sits in an arrow's parameter list after its last parameter:
    ///
    /// ```text
    /// (a: string, // comment
    /// ) => {}
    /// ```
    ///
    /// The call printers read this as "the params will be multiline", and force their
    /// wrapped state on it rather than hugging the callback.
    ///
    /// ⚠️ It is a question about the **parameter list**, so it stops at
    /// [`Self::arrow_params_end`] — never at the `=>`. Reading on to the `=>` swept in two
    /// regions that hold no parameter comment at all, the return type and the signature→`=>`
    /// gap, and force-wrapped the whole call around a comment that leaves the list flat:
    /// `fn((a) /* c */ => { … })`, `fn((a): T /* c */ => …)` and `fn((a): /* c */ T => …)` each
    /// lost prettier's hug. The over-reach was invisible while
    /// [`Self::append_pre_arrow_comments`]'s gap went unprinted — the layout was wrong about a
    /// comment that wasn't in the output.
    ///
    /// A `Printer` method rather than one more predicate in `calls/arg_predicates.rs`: it needs
    /// the comment table, and it owns its own bound, so no call site can hand it a different
    /// one.
    pub(in crate::printer) fn arrow_has_trailing_param_comments(
        &self,
        arrow: &internal::ArrowFunctionExpression<'_>,
    ) -> bool {
        let Some(last_param) = arrow.params.last() else {
            return false;
        };
        let Some(params_end) = self.arrow_params_end(arrow) else {
            return false;
        };
        self.has_comments_to_emit_between(last_param.span().end, params_end)
    }

    /// Append the comments an author wrote between an arrow's signature and its `=>`
    /// (`(x) /* c */ =>`), returning `sig` untouched when that gap is empty — the
    /// overwhelmingly common case, which costs one span search and no doc node.
    ///
    /// ⚠️ **One gap, one emitter.** Every call-argument state reassembles an arrow as
    /// signature + `" =>"` + body without routing it through
    /// [`Self::build_arrow_doc`](Self::build_arrow_doc), so a gap those states don't ask
    /// about is a gap **nobody** prints (`docs/comments.md` hazard 4). That dropped the
    /// comment outright in `fn((x) /* c */ => call(x))`, `new Comp(…)` and the
    /// return-type spelling alike — at every reassembly site — while the plain arrow and
    /// the member chain printed it, which is what made the loss look context-dependent
    /// rather than structural.
    ///
    /// Only block-shaped payloads can occur here: `=>` is preceded by
    /// `[no LineTerminator here]`, so a `//` in this gap is a parse error, not a layout
    /// question.
    pub(in crate::printer) fn append_pre_arrow_comments(
        &self,
        arrow: &internal::ArrowFunctionExpression<'_>,
        sig: DocId,
    ) -> DocId {
        let sig_end = self.arrow_signature_end(arrow);
        let arrow_pos = arrow.arrow_token;
        if !self.has_comments_to_emit_between(sig_end, arrow_pos) {
            return sig;
        }
        let d = self.d();
        let mut parts: DocBuf = smallvec![sig];
        for comment in comments_to_emit_in_range(self.comments, sig_end, arrow_pos) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        d.concat(&parts)
    }

    /// Build just the arrow function signature (async + type params + params + return type)
    /// WITHOUT the ` =>` and body. Used by call printer for expand-last-arg pattern.
    ///
    /// This is extracted from `build_arrow_doc_wrapping` to support the special case
    /// where call expressions need to build arrows with conditional parens around the body.
    pub(crate) fn build_arrow_signature_doc(
        &self,
        arrow: &internal::ArrowFunctionExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        self.push_async_arrow_head(&mut parts, arrow);

        // Type parameters: always their own group so they break independently of
        // the rest of the signature (Prettier's printTypeParameters semantics) —
        // which is what `_wrapping` is, so an arrow shares the declaration printer
        // rather than restating its layout. A hand-copied twin lived here and emitted
        // only its children's docs, so every block comment in the list's own gaps was
        // DROPPED (`<T /* c */, U>` → `<T, U>`, and the three sibling positions) —
        // hazard 4, masked by the line-comment arm routing to the comment-aware
        // builder, which is why only a `//` survived.
        if let Some(tp) = &arrow.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(tp));

            // Comments between type_params `>` and `(` go after type_params
            if let Some(pp) = find_char_skipping_comments(
                self.source.as_bytes(),
                tp.span.end as usize,
                self.source.len(),
                b'(',
            ) {
                self.append_type_params_to_paren_comments(&mut parts, tp.span.end, pp as u32);
            }
        }

        // Function parameters
        parts.push(self.build_arrow_params_doc_ungrouped(arrow));

        // Return type annotation
        if let Some(return_type) = &arrow.return_type {
            parts.push(self.build_arrow_return_type_doc(return_type, arrow.params_start));
        }

        d.concat(&parts)
    }

    /// Check if any param has a trailing line comment or own-line block comment.
    ///
    /// The last param's arm reads own-line-ness from the SOURCE
    /// ([`Printer::block_comment_owns_its_line`] with `item_follows: false` — no item is
    /// left to lead, so the `)` sharing the comment's line is not glue and the predicate
    /// reduces to its leading half). Anchoring it on the param's line instead called a
    /// comment glued to the list's own comma own-line and opened a list that fits
    /// (`docs/comments.md` §Own-line-ness is a SOURCE question). Paired with
    /// [`Self::build_trailing_gap_comments_ext`], which the last param's arm of
    /// `build_function_params_doc` emits that region through: it trails exactly the comments
    /// this does not force a break for.
    fn has_trailing_line_comment_in_params(
        &self,
        params: &[internal::Expression<'_>],
        trailing_comments_end: Option<u32>,
    ) -> bool {
        params.iter().enumerate().any(|(i, param)| {
            // The PRINTED end, the same anchor the loop's trailing arms claim from: a `//`
            // inside a stripped-paren shell (`a = (1 // c⏎)`) is in the gap they emit, so a
            // gate that starts past the shell lets that comment defer out of a list that
            // never breaks — past the `)` and the body, swallowing whatever follows.
            let param_end = param.printed_end();
            let trailing_end = self.param_trailing_end(params, i, trailing_comments_end);
            if self.has_line_comments_between(param_end, trailing_end) {
                return true;
            }
            // For the last param, also check for own-line block comments before `)`
            i == params.len() - 1
                && self.has_own_line_block_comment_before_closer(param_end, trailing_end)
        })
    }

    /// Where the trailing region after `params[index]` ENDS: the next param's printed
    /// start ([`Self::param_start_with_decorators`]), or — for the last param — the
    /// list's trailing bound. The one spelling of that bound, shared by the emitters in
    /// `build_params_doc_with_comments` and by the force-break gate over them
    /// ([`Self::has_trailing_line_comment_in_params`]), so a comment the gate opens the
    /// list for is the same set the emitters claim.
    ///
    /// A DECORATED next param prints its own decorator region (`build_param_decorators_doc`),
    /// so this bound stops at its first `@`, never at its binding: the region past the `@`
    /// is that builder's to claim, and a trailing scan reaching into it prints the
    /// decorator's comment a second time on the previous param's comma
    /// (`docs/comments.md` §The element-comma seam).
    fn param_trailing_end(
        &self,
        params: &[internal::Expression<'_>],
        index: usize,
        trailing_comments_end: Option<u32>,
    ) -> u32 {
        params.get(index + 1).map_or_else(
            || trailing_comments_end.unwrap_or_else(|| params[index].span().end),
            |next| self.param_start_with_decorators(next),
        )
    }

    /// Build doc for arrow function body expression.
    fn build_arrow_body_doc(&self, expr: &internal::Expression<'_>) -> DocId {
        self.build_arrow_body_doc_with_leading(expr, None)
    }

    /// `build_arrow_body_doc` with a leading comment run placed correctly relative to
    /// whatever parens the body takes.
    ///
    /// The run rides **inside** the ternary's `if_break` parens and **outside** every
    /// other kind. The others are *required* parens (object / assignment
    /// disambiguation), where both formatters keep a leading comment before the `(`;
    /// the ternary's are a layout artifact that vanishes the moment the body breaks, so
    /// a run left outside them would occupy a position that exists in one rendering
    /// only, and the two renderings would disagree about what the comment leads. Inside
    /// is also what lets the authored blank-after-`(` spelling reach its fixed point in
    /// ONE pass, as prettier does, instead of relocating on a second.
    ///
    /// `None` is the no-run case and adds no doc node — this is a hot path.
    fn build_arrow_body_doc_with_leading(
        &self,
        expr: &internal::Expression<'_>,
        leading: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let prepend = |doc: DocId| prepend_leading(d, leading, doc);
        // Expand-last-arg body reuse: when the enclosing call/new expand-last path has
        // pre-built this exact body (to also compose the break-body state), reuse that
        // DocId instead of rebuilding — rebuilding here *and* separately recurses into
        // itself for `f(lead, x => f(lead, y => …))`, making the doc-node count
        // O(2^depth). See the `arrow_body_inject` field.
        if let Some((span, doc)) = self.arrow_body_inject.get()
            && span == expr.span().start
        {
            return prepend(doc);
        }
        // An arrow body is a value gap (`mark_jsdoc_cast_value_gap`).
        self.mark_jsdoc_cast_value_gap(expr);
        // Object at leftmost position in arrow body needs parens to avoid block ambiguity.
        // Examples: `() => ({}) as T`, `() => ({}).prop`, `() => ({}) && a`, `() => ({}).b++`.
        // The span target tells build_expression_doc to wrap exactly that ObjectExpression
        // in parens when reached. Keyed by span (not a bool) so it survives a chain base
        // being rebuilt across conditional-group variants, and never matches a same-shaped
        // object nested deeper (e.g. a call argument). Saved/restored for nested arrows.
        if let Some(obj_span) = leftmost_object_span(expr) {
            let prev = self.arrow_body_object_parens_target.replace(Some(obj_span));
            let doc = self.build_expression_doc(expr);
            self.arrow_body_object_parens_target.set(prev);
            return prepend(doc);
        }

        // Conditional expressions: parens when inline, none when on own line.
        // The primary ternary path is shouldAddParensIfNotBreak in
        // build_arrow_doc_wrapping. This branch handles ternaries reached
        // via other callers (curried innermost arrow, post-arrow comments)
        // where the body always breaks via hardline — if_break selects
        // the break variant (no parens).
        if matches!(expr, internal::Expression::ConditionalExpression(_)) {
            let body_doc = self.build_expression_doc(expr);
            // The leading run rides INSIDE these parens, so the paren decision wraps
            // both — but `will_break` still asks about the BODY alone.
            let with_leading = prepend(body_doc);
            // If body contains hardlines (will definitely break), no parens
            if d.will_break(body_doc) {
                return with_leading;
            }
            // Otherwise, use if_break to check enclosing group
            return d.if_break(with_leading, d.parens(with_leading));
        }

        // Standard cases: objects and assignments always need parens
        if self.needs_parens(expr, ParenContext::ArrowBody) {
            prepend(d.parens(self.build_expression_doc(expr)))
        } else {
            prepend(self.build_expression_doc(expr))
        }
    }

    /// Build doc for arrow function body with own-line leading comments.
    ///
    /// Called when at least one comment between `=>` and body is on its own line
    /// (line comment or block comment with newline after). Inline block comments
    /// use `build_inline_post_arrow_comments_doc` instead.
    /// ```typescript
    /// () =>
    ///     /* comment */
    ///     expr
    /// ```
    fn build_arrow_body_with_comments_doc(
        &self,
        expr: &internal::Expression<'_>,
        sig_end: u32,
        body_start: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();

        // Print leading comments: a block comment inline-adjacent to the next
        // comment / the body hugs it with a space; a line comment or own-line
        // block drops to its own line (a line comment must break so it can't
        // absorb the body). Same shape as the RHS-of-`=` leading run.
        self.push_leading_comment_run(
            &mut parts,
            comments_to_emit_in_range(self.comments, sig_end, body_start),
            body_start,
            LeadingGlue::Adjacent,
            d.empty(),
        );

        // The run is handed to the body rather than concatenated ahead of it, so it
        // lands on the correct side of whatever parens the body takes — see
        // `build_arrow_body_doc_with_leading`. For every body but a ternary the two are
        // the same bytes; for a ternary the run belongs inside its layout parens.
        self.build_arrow_body_doc_with_leading(expr, Some(d.concat(&parts)))
    }

    /// Check if any comment between `=>` and body is on its own line.
    ///
    /// Matches Prettier's `hasLeadingOwnLineComment` which checks `hasNewline(text, locEnd(comment))`
    /// — whether there's a newline after each comment. Inline block comments like
    /// `=> /* c */ expr` have no newline after them (returns false). Own-line comments
    /// and line comments have a newline after (returns true).
    pub(crate) fn has_own_line_post_arrow_comment(&self, sig_end: u32, body_start: u32) -> bool {
        for comment in comments_to_emit_in_range(self.comments, sig_end, body_start) {
            // A line comment, a multiline block, or a block that starts on its own
            // line (a newline precedes it) forces the body onto its own line. A
            // single-line block glued to `=>` keeps the body hugged even when the
            // body follows on the next source line (`=> /* c */⏎expr` → `=> /* c */ expr`).
            if self.comment_cannot_glue_to_operator(comment) {
                return true;
            }
        }
        false
    }

    /// Build doc for inline block comments between `=>` and body.
    ///
    /// Only called when all comments are inline (no own-line comments).
    /// Emits each comment followed by a space: `/* c1 */ /* c2 */ `
    fn build_inline_post_arrow_comments_doc(&self, sig_end: u32, body_start: u32) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        for comment in comments_to_emit_in_range(self.comments, sig_end, body_start) {
            parts.push(self.build_comment_doc(comment));
            parts.push(d.text(" "));
        }
        d.concat(&parts)
    }

    /// Build a Doc for an arrow function (simple, non-wrapping version for nested contexts)
    pub(super) fn build_arrow_doc(&self, arrow: &internal::ArrowFunctionExpression<'_>) -> DocId {
        // For nested contexts where we don't want independent wrapping decisions,
        // use the wrapping version which will be evaluated in context
        self.build_arrow_doc_wrapping(arrow)
    }

    /// Build a Doc for just the function expression signature (type params, params, return type).
    /// Body is printed separately via imperative printer to preserve comments.
    ///
    /// One depth-tracked scan locates the params' close `)`; every boundary derived
    /// from it shares that scan (same contract as `build_callable_signature_doc`).
    /// Returns the doc plus the signature end — where comments before the body
    /// begin: the return type's end when present, otherwise just past the `)`
    /// (falling back to the body start if the paren can't be located).
    fn build_function_expression_signature_doc(
        &self,
        func: &internal::FunctionExpression<'_>,
    ) -> (DocId, u32) {
        let d = self.d();
        let mut sig_parts = DocBuf::new();

        let body_start = func.body.span.start;

        // Type parameters (TypeScript generics): <T, U>
        // Use _wrapping version for width-based line breaking
        if let Some(type_params) = &func.type_parameters {
            sig_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));

            // Comments between type_params `>` and `(` go after type_params
            if let Some(pp) = find_char_skipping_comments(
                self.source.as_bytes(),
                type_params.span.end as usize,
                self.source.len(),
                b'(',
            ) {
                self.append_type_params_to_paren_comments(
                    &mut sig_parts,
                    type_params.span.end,
                    pp as u32,
                );
            }
        }

        // Params + return type + single-param hug + signature end, shared with
        // `build_callable_signature_doc`.
        let (params_doc, return_type_doc, sig_end) = self.build_signature_params_return(
            func.params,
            func.type_parameters.as_ref(),
            func.return_type.as_ref(),
            func.params_start,
            body_start,
        );

        sig_parts.push(params_doc);
        if let Some(rt_doc) = return_type_doc {
            sig_parts.push(rt_doc);
        }

        // Wrap signature in a group for width-aware breaking
        (d.group(d.concat(&sig_parts)), sig_end)
    }

    /// Build a Doc for function expression body (type params, params, return type, body).
    ///
    /// Used for method shorthand in objects where the key is printed separately.
    /// For standalone function expressions, use `build_function_doc` instead.
    pub(in crate::printer) fn build_function_doc_body(
        &self,
        func: &internal::FunctionExpression<'_>,
    ) -> DocId {
        let d = self.d();
        // sig_end bounds the outer comment detection before the body.
        let (sig_doc, sig_end) = self.build_function_expression_signature_doc(func);

        let mut parts: DocBuf = smallvec![sig_doc];
        self.append_body_with_sig_comments(&mut parts, sig_end, &func.body);
        d.concat(&parts)
    }

    /// Build a Doc for a standalone function expression with width-aware wrapping.
    ///
    /// This includes:
    /// - `async` keyword if present
    /// - `function` keyword
    /// - `*` for generators
    /// - optional name
    /// - type parameters
    /// - parameters and return type
    /// - body
    pub(in crate::printer) fn build_function_doc(
        &self,
        func: &internal::FunctionExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();

        // `async`, the gap before `function`, the keyword and a generator `*` — the
        // same head the declaration prints. The cursor it returns is where the
        // keyword→name gap starts; reading that gap from `func.span.start` instead
        // let it swallow the `async`→`function` comment and reprint it after the
        // keyword.
        let head_end = self.push_function_keyword_head(
            &mut parts,
            func.span.start,
            func.id
                .as_ref()
                .map_or(func.params_start, |id| id.span.start),
            FunctionHeadModifier::from_async(func.r#async),
            func.generator,
        );

        // Optional function name
        if let Some(id) = &func.id {
            // Comments between keywords and the name (same as FunctionDeclaration)
            parts.push(self.build_keyword_to_name_comments(head_end, id.span.start));
            parts.push(self.build_identifier_doc(id));

            // Comments between name and type params/parens: `function fn1/* c */ <T>()` or `fn1 /* c */()`
            // Line comments get a hardline to prevent absorbing type params as comment text
            let comment_end = func
                .type_parameters
                .as_ref()
                .map_or(func.params_start, |tp| tp.span.start);
            self.push_name_to_type_params_comments(
                &mut parts,
                id.span.end,
                comment_end,
                CommentSpacing::for_type_params(func.type_parameters.is_some()),
            );
        }

        // Space before type params or params if no name: `function <T>` or `function ()`
        // Also extract comments between keyword and next element: `function /* c */ ()`
        // Line comments get hardline to prevent absorbing parens: `function // c\n()`
        if func.id.is_none() {
            let next_start = func
                .type_parameters
                .as_ref()
                .map_or(func.params_start, |tp| tp.span.start);
            // From `head_end`, not from the span start: the `async`→`function` gap is
            // already printed, and re-reading it here printed those comments TWICE.
            parts.push(self.build_keyword_to_name_comments(head_end, next_start));
        }

        // Type params, params, return type, and body (signature_doc handles type params)
        parts.push(self.build_function_doc_body(func));

        d.concat(&parts)
    }

    /// A value-side parameter list item: the freeze-aware layer over
    /// `build_function_parameter_doc`. An alone-on-line format-ignore directive in
    /// parameter `i`'s leading gap freezes the parameter verbatim (Rule A); one written
    /// between its decorators and its binding freezes just the binding. The type-side
    /// twin is `build_function_type_param_item_doc`.
    ///
    /// `list_frozen` is the caller's already-resolved outer freeze
    /// ([`Printer::param_frozen_span`]) rather than a lookup of its own: the caller needs
    /// the same answer for the comment seam's claim anchor, and one lookup shared is what
    /// keeps the doc and the anchor from disagreeing about what was printed.
    fn build_function_parameter_item_doc(
        &self,
        list_frozen: Option<Span>,
        param: &internal::Expression<'_>,
    ) -> DocId {
        if let Some(frozen) = list_frozen {
            return self.build_frozen_span_doc(frozen);
        }
        self.build_frozen_param_binding_doc(param)
            // FunctionParameter context for object patterns
            .unwrap_or_else(|| self.build_function_parameter_doc(param))
    }

    /// Shared implementation for building params doc with comment handling
    ///
    /// Used by arrow functions, function expressions, function declarations, and class methods.
    pub(crate) fn build_params_doc_with_comments(
        &self,
        params: &[internal::Expression<'_>],
        params_start: Option<u32>,
        trailing_comments_end: Option<u32>,
    ) -> DocId {
        let d = self.d();
        // A test call's callback keeps its parameter LIST flat at any width — prettier's
        // `isParametersInTestCall` (`print/function-parameters.js`), which returns
        // `["(", ...printed, ")"]`: `", "` separators, no indent, no softlines, and each
        // parameter's OWN doc left breakable. Consumed here, at the top and before any child
        // doc is built, so it reaches this list and nothing under it — a function nested in a
        // parameter default keeps ordinary width-driven params. See the field doc on
        // `Printer::test_call_flat_params`.
        let test_call_flat = self.test_call_flat_params.replace(false);
        if params.is_empty() {
            // Search to the end of source rather than `trailing_comments_end` — that
            // boundary is clamped to the `)` position for non-empty params, which is
            // too tight here (the depth-tracked search must reach the `)` itself).
            return self
                .build_empty_params_with_comments_doc(params_start, self.source.len() as u32);
        }

        // Zero-comment fast gate: one binary search over the whole params window.
        // Every comment sub-query below (the hug/force-break predicates and the
        // per-gap lookups in the build loop) is bounded within
        // [window_start, window_end] — so when no comment lies inside the window, every
        // sub-query is provably empty/false. Skip them all, including the per-gap
        // `find_comma_after` trivia scans, whose results feed only comment placement.
        //
        // **On page**, not to-emit: this gate guards layout decisions (the single-pattern
        // hug, the force-break legs, the blank-line scan's comment-aware bound), and an
        // owned comment still occupies the page for every one of them. An emit-keyed gate
        // here would make an owned comment vanish from a decision it is visibly part of
        // (`Printer::has_comments_on_page_between`). The sub-queries stay emit-keyed —
        // they answer "what must *this* caller print", which is the other question.
        let comments_present = {
            let window_start = params_start.unwrap_or_else(|| params[0].span().start);
            let last_end = params[params.len() - 1].span().end;
            let window_end = trailing_comments_end.map_or(last_end, |end| end.max(last_end));
            self.has_comments_on_page_between(window_start, window_end)
        };

        // Prettier's shouldHugFunctionParameters: single param that's an object/array pattern
        // gets hugged - no breaks added around it, the pattern handles its own expansion.
        // This keeps `({` and `}: Type)` together, letting the pattern's content break:
        //   function fn({
        //       a,
        //       b,
        //   }: Type): void {}
        // NOT:
        //   function fn(
        //       {a, b}: Type,
        //   ): void {}
        //
        // Also applies to parameters with TypeLiteral type annotations like `a?: { b: T }`:
        //   function fn(a?: {
        //       b: T;
        //   }): void {}
        // NOT:
        //   function fn(
        //       a?: { b: T },
        //   ): void {}
        let no_leading_comments = !comments_present
            || !self.has_comments_to_emit_between(
                params_start.unwrap_or_else(|| params[0].span().start),
                params[0].span().start,
            );
        let no_trailing_comments = !comments_present
            || trailing_comments_end
                .is_none_or(|end| !self.has_comments_to_emit_between(params[0].span().end, end));
        let should_hug_single_pattern = params.len() == 1
            && (is_huggable_pattern(&params[0]) || has_huggable_type_annotation(&params[0]))
            && no_leading_comments
            && no_trailing_comments
            // An own-line parameter decorator forces the list to expand (prettier),
            // which the hug can't express — fall through to the breakable path.
            && !self.param_has_own_line_decorators(&params[0]);

        if should_hug_single_pattern {
            // Hug mode: just ( + pattern + optional trailing comma + )
            let param_doc = self.build_function_parameter_doc(&params[0]);
            return d.parens(param_doc);
        }

        // A line comment trailing the opening `(` (`fn( // c`) is kept on the `(`
        // line, matching the function-type / call-signature `(` and the whole
        // open-delimiter family — via the same `delimiter_line_comment_prefix`
        // helper as `build_type_params_multiline_parts`. Prettier relocates it to
        // the first param's own line (function expression / arrow) or floats it
        // past the declaration (function declaration). The pull fires only for a
        // same-line comment forcing expansion, so it always forces the break path
        // below. See conformance_prettier_ts_comments.md §Comment relocation and
        // open_paren_line_comment_prettier_divergence.
        //
        // The gap ends where the first param's doc STARTS PRINTING
        // ([`Printer::param_start_with_decorators`]), not at its binding: a decorated
        // param prints its own decorator region, comments included
        // (`build_param_decorators_doc`), so a gap reaching past the `@` is claimed
        // twice — once here and once by that builder — and one authored comment is
        // printed twice (`docs/comments.md` §The element-comma seam). The leading run
        // this pull partitions against (`build_leading_param_comments`) already stops
        // at the same position, which is why `skip_delim` cannot see, let alone
        // suppress, a comment pulled from inside the decorators.
        let (paren_prefix, paren_pull_pos) = match params_start {
            Some(open) if comments_present => self
                .delimiter_line_comment_prefix(open, self.param_start_with_decorators(&params[0])),
            _ => (DocBuf::new(), None),
        };

        // Check if any trailing line comments exist on params
        // If so, we must use hardlines to force the group to break
        let has_trailing_line_comment = comments_present
            && self.has_trailing_line_comment_in_params(params, trailing_comments_end);

        // Check if any leading line comments exist on their own line before params
        // Line comments on their own line also force break
        let has_leading_own_line_comment =
            comments_present && self.has_leading_own_line_comment_in_params(params, params_start);

        // Prettier rule: force break when 2+ params and at least one is TSParameterProperty
        // (has access modifiers like private/public/protected/readonly)
        let should_break_for_param_properties = params.len() > 1
            && params
                .iter()
                .any(|p| matches!(p, internal::Expression::TSParameterProperty(_)));

        // A blank line the author left between two params forces the list to expand,
        // and the separator emission preserves it — matching prettier and tsv's own
        // object-literal behavior (a bare blank is authorial intent, like one around
        // a comment).
        let has_blank_line_between_params = self.has_blank_line_between_params(params);

        // Force multiline when comments, param-property modifiers, or an author blank
        // line require it.
        // A test call's flat separator outranks the blank-line arm, as it does in prettier
        // (`isParametersInTestCall` is checked BEFORE `isNextLineEmpty`), so an author blank
        // inside the callback's parameter list does not open the list. It does NOT outrank the
        // comment arms: a comment there breaks the list in both formatters.
        let force_break = has_trailing_line_comment
            || has_leading_own_line_comment
            || should_break_for_param_properties
            || (has_blank_line_between_params && !test_call_flat)
            || paren_pull_pos.is_some();
        let flat_list = test_call_flat && !force_break;

        let mut inner_parts = d.pooled_docbuf();
        // The comma closing the PREVIOUS param, carried across the iteration rather than
        // re-scanned: the gap `params[i-1].end → params[i].start` holds exactly one comma,
        // and iteration `i-1` already located it as its own `comma_pos`. Scanning it again
        // as this iteration's `prev_comma_pos` would ask the same source range twice.
        let mut prev_comma_pos: Option<u32> = None;
        // The previous param's claim anchor (`Printer::element_claim_anchor`), carried for
        // the same reason as `prev_comma_pos`: it is one answer about one gap, and the
        // trailing side has already computed it.
        let mut prev_param_end: Option<u32> = None;
        for (i, param) in params.iter().enumerate() {
            let param_start = param.span().start;
            let is_last = i == params.len() - 1;

            // Where this param's leading gap opens: the previous param's CLAIM anchor
            // ([`Printer::element_claim_anchor`]), which its span end overshoots by a
            // stripped-paren shell (`a = (1 /* c */)`) whose interior would then belong to
            // no emitter at all (`docs/comments.md` §The element-comma seam). Carried from
            // the previous iteration rather than recomputed, so the two sides of the gap
            // cannot disagree — in particular about the FREEZE arm, where the anchor stays
            // at the frozen slice's end and a printed-end reading here would re-emit a
            // comment that slice already printed.
            //
            // Every question in this iteration opens here, the blank-line one included: it
            // takes the DISTANCE anchor derived from this position (`element_shell_end`,
            // below), never the raw span end, so the shell is peeled with a comment inside
            // it left measurable.
            let gap_start = prev_param_end.unwrap_or_else(|| {
                // First param: from just after `(`.
                params_start.map_or(param_start, |pos| pos + 1)
            });

            // The three facts that decide which comments lead this param, derived ONCE and
            // shared by the separator and the leading-comment emitter below. They are the
            // whole input to that split, so two derivations of them are two answers — the
            // separator would measure its gap against a boundary the emitter never agrees to.
            // (`prev_comma_pos` is carried from the previous iteration; both consumers sit
            // under `comments_present`, so a comment-free list never locates a comma at all.)
            //
            // The first param excludes any comment already pulled onto the `(`
            // line by `delimiter_line_comment_prefix`, so it isn't emitted twice.
            let skip_delim = if i == 0 { paren_pull_pos } else { None };
            let param_render_start = self.param_start_with_decorators(param);

            // Add separator before non-first params
            if i > 0 {
                // Use hardline when forcing break (trailing line comments or param properties)
                if force_break {
                    // Preserve a blank line the author left before this param's printed
                    // content — prettier keeps one blank line in the expanded list.
                    //
                    // The DISTANCE anchor: the previous param's shell end
                    // (`Printer::element_shell_end`), the same peel every list that shares
                    // the item separator takes (`Printer::push_item_blank_separator`). With
                    // no comment in the shell it lands on the span end, which is where the
                    // break gate (`has_blank_line_between_params`) measures from, so the
                    // gate and this emitter still cannot disagree about a bare blank — and
                    // where they could, a comment in the shell, that comment has already
                    // forced the break on its own.
                    //
                    // The scan stops at this param's content start, and `is_next_line_empty`
                    // steps past the previous param's same-line trailing comment (`a, // x`)
                    // rather than treating it as the boundary, so a blank line *after* it
                    // still counts.
                    let blank_start = self.element_shell_end(gap_start, param_start);
                    let content_start = if comments_present {
                        self.param_content_start(
                            gap_start,
                            param_render_start,
                            prev_comma_pos,
                            skip_delim,
                            param,
                        )
                    } else {
                        param_start
                    };
                    self.push_next_line_empty_hardline(
                        &mut inner_parts,
                        blank_start,
                        content_start,
                    );
                } else if flat_list {
                    inner_parts.push(d.text(" "));
                } else {
                    inner_parts.push(d.line());
                }
            }

            // Add leading comments for this param
            // Use proper line breaks for line comments on their own line
            if comments_present {
                inner_parts.push(self.build_leading_param_comments(
                    gap_start,
                    param_render_start,
                    prev_comma_pos,
                    skip_delim,
                ));
            }

            // A parameter has TWO freeze positions and the item doc takes whichever fired
            // — the list gap first, then the decorators→binding gap. Resolved here rather
            // than inside that builder because the claim anchor below needs the same
            // answer, and the second position is the one an outer-only reading misses.
            let list_frozen = self.param_frozen_span(params_start, params, i);
            inner_parts.push(self.build_function_parameter_item_doc(list_frozen, param));

            // Where this param's doc STOPS PRINTING. Every comment question in this
            // iteration takes it, so the trailing arms and the next param's leading run
            // partition one gap from one anchor.
            let param_end = Self::element_claim_anchor(
                list_frozen.or_else(|| self.param_binding_frozen_span(param)),
                param.printed_end(),
            );

            // Handle trailing same-line comments. One spelling for both arms
            // ([`Self::param_trailing_end`]) — the non-last bound is the next param's
            // printed start, which is also what the force-break gate reads, so the gate
            // and these emitters agree by construction rather than by coincidence.
            let search_end = self.param_trailing_end(params, i, trailing_comments_end);

            // Find this param's separator comma, bounded by `search_end` — the next param's
            // start, or (last param) the end of its trailing range. The bound is what makes
            // one arm serve both: a last param usually has NO comma, and an unbounded scan
            // would run to the next comma anywhere later in the file and read it as this
            // param's separator, relocating an after-comma block comment across it. Same
            // hazard the element-comma collector names (`collect_trailing_comments`).
            // Consumed only by comment placement, so the zero-comment gate skips the scan.
            let comma_pos = comments_present
                .then(|| self.find_comma_in_range(param_end, search_end))
                .flatten();

            if is_last {
                // The LAST param's whole trailing region — its printed end to the `)` — in
                // ONE ordered pass through the shared last-item→closer walk, since no comma
                // is emitted here to split it around. `true`: a `//` in the run defers
                // through `line_suffix` (zero width) and can only be the run's LAST member,
                // so nothing is emitted behind it on that line; the `)` this list is forced
                // open around ends it.
                //
                // The gate over this seam ([`Self::has_trailing_line_comment_in_params`])
                // reads the same source, and must: a gate that forces the break for a
                // comment this emitter trails inline opens the list around nothing.
                if comments_present {
                    inner_parts
                        .extend(self.build_trailing_gap_comments_ext(param_end, search_end, true));
                }
            } else {
                // The param's same-line **block** comments, split by side of the comma
                // below. Line comments are deliberately not in here: their claim asks the
                // comma as well as the param (`param_trailing_line_comment`), so a shared
                // "same-line run" would read as answering for them too while quietly using
                // the narrower anchor.
                //
                // "Same line" is the gap's anchor-line run ([`Printer::gap_anchor_line_end`]),
                // which FOLLOWS a multi-line block to its closing `*/` line — a bare
                // `is_same_line(param_end, …)` reads a comment glued past that `*/`
                // (`aaaa /* x⏎y */ /* c */,`) as own-line and hands it to the next param's
                // leading run. `leading_param_comments` reads the same split, so the two
                // still partition the gap.
                let anchor_line_end = if comments_present {
                    self.gap_anchor_line_end(param_end, search_end)
                } else {
                    param_end
                };
                let same_line_blocks: CommentVec<'_> = if comments_present {
                    comments_to_emit_in_range(self.comments, param_end, search_end)
                        .filter(|c| c.is_block && c.span.start < anchor_line_end)
                        .collect()
                } else {
                    CommentVec::new()
                };

                // Block comments BEFORE comma go before comma
                for comment in same_line_blocks
                    .iter()
                    .filter(|c| comma_pos.is_none_or(|pos| c.span.start < pos))
                {
                    inner_parts.push(d.text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                }

                // Add inter-param separator comma (only between params; the last param
                // gets no trailing comma — trailingComma: 'none').
                inner_parts.push(d.text(","));
                // A stranded after-comma block (on the comma's line, but a newline
                // before the next param) trails the comma — preserving the author's
                // placement, matching call args / declarators (prettier relocates it
                // before the comma). A block hugging the next param leads it instead
                // (as a leading comment). See conformance_prettier_ts_comments.md §Comment relocation.
                if let Some(cp) = comma_pos {
                    let next_start = self.param_start_with_decorators(&params[i + 1]);
                    self.push_stranded_after_comma_blocks(&mut inner_parts, cp, next_start);
                }

                // Line comments trailing this param go after the comma (excluded from
                // width). Block comments AFTER the comma are handled as leading for the
                // next param.
                //
                // Scanned rather than filtered out of `same_line_blocks`: the claim's
                // anchor is the COMMA as well as the param (`param_trailing_line_comment`),
                // and a comment on a comma the author pushed onto its own line is not on
                // the param's line at all. `leading_param_comments` excludes exactly this
                // set, so the two partition the gap.
                if comments_present {
                    for comment in comments_to_emit_in_range(self.comments, param_end, search_end)
                        .filter(|c| {
                            self.param_trailing_line_comment(
                                c,
                                param_end,
                                anchor_line_end,
                                comma_pos,
                            )
                        })
                    {
                        inner_parts.push(self.build_trailing_line_comment_doc(comment));
                    }
                }
            }

            prev_comma_pos = comma_pos;
            prev_param_end = Some(param_end);
        }

        // No group - outer signature group controls breaking
        let mut result: DocBuf = smallvec![d.text("(")];
        // A pulled `( // c` comment renders on the `(` line before the break.
        result.extend(paren_prefix);

        if force_break {
            // When forcing break (trailing comments or param properties), use hardlines.
            result.push(d.indent_hardline(d.concat(&inner_parts)));
            result.push(d.hardline());
        } else if flat_list {
            // No indent and no softlines: the list itself offers no break point, exactly as
            // the single-pattern hug above already does. Each parameter's own doc is
            // untouched, so a destructured pattern still expands on its own.
            result.push(d.concat(&inner_parts));
        } else {
            // No trailing comma (trailingComma: 'none').
            result.push(d.indent_softline(d.concat(&inner_parts)));
            result.push(d.softline());
        }

        result.push(d.text(")"));

        d.concat(&result)
    }

    /// Whether any param is led by a comment on its own line — the single answer to "does a
    /// comment force this param list open?", shared by the value-level router
    /// (`build_function_params_doc`) and the type-level one (`function_types`), so the two
    /// cannot disagree about the same gap. See [`Self::has_own_line_comment_between`] for the
    /// per-gap rule.
    ///
    /// The gap opens at the previous param's PRINTED end, like every other gate over this
    /// seam (`docs/comments.md` §The element-comma seam): a comment inside a stripped paren
    /// shell (`a = (1⏎/* c */)`) sits before the span end, so a span-anchored gate calls the
    /// list comment-free while the leading-run emitter prints the comment anyway. The list
    /// then breaks on that comment's own hardline with `force_break` still false — and the
    /// separator, which is the only thing that preserves an author blank, never runs.
    pub(in crate::printer) fn has_leading_own_line_comment_in_params(
        &self,
        params: &[internal::Expression<'_>],
        params_start: Option<u32>,
    ) -> bool {
        for (i, param) in params.iter().enumerate() {
            let search_start = if i == 0 {
                params_start.map_or_else(|| param.span().start, |pos| pos + 1)
            } else {
                params[i - 1].printed_end()
            };

            // Check if there's a line comment on its own line before this param
            if self
                .has_own_line_comment_between(search_start, self.param_start_with_decorators(param))
            {
                return true;
            }
        }
        false
    }

    /// Whether a comment between two params forces the param list to expand.
    ///
    /// A **line** comment always forces it (it runs to end-of-line, so the following param
    /// can't share the line). A **block** comment forces it under the shared classification
    /// ([`Printer::block_comment_owns_its_line`], which carries the argument): either
    /// adjacency keeps it inline, matching prettier, which collapses `a,⏎/* c */ b` and
    /// `a /* c */,⏎b` both back to `a, /* c */ b`.
    ///
    /// A parameter list is the container `docs/conformance_prettier.md` §Comment Position
    /// Philosophy names outright — it flattens when it fits — so this gate exists for the
    /// comment the author gave a line of its own, and for nothing else.
    fn has_own_line_comment_between(&self, start: u32, end: u32) -> bool {
        self.comments_on_page_between(start, end).any(|c| {
            // `end` is the next param's start, so an item always follows; the trailing
            // position past the last param is `has_own_line_block_comment_before_closer`'s.
            !c.is_block || self.block_comment_owns_its_line(c, true)
        })
    }

    /// Whether the author left a blank line between any two consecutive params.
    /// A bare blank is authorial intent (like one around a comment), so it forces
    /// the param list to expand and the separator emission preserves it — matching
    /// prettier. Shared by regular function params and the type-level param lists
    /// (function/constructor types, method/call/construct signatures).
    ///
    /// This gate and the separator that *emits* the blank must answer **one** question:
    /// they both route through [`Printer::is_next_line_empty`]. They did not always — the
    /// gate measured the whole gap while the emitter measured only past the comma, so
    /// `f(a⏎⏎, b)` broke the list and then dropped the blank, and the second pass (no blank
    /// left to force the break) collapsed it again.
    pub(in crate::printer) fn has_blank_line_between_params(
        &self,
        params: &[internal::Expression<'_>],
    ) -> bool {
        params.windows(2).any(|pair| {
            // Measure to the next param's first decorator, not its binding — a
            // decorator written on its own line sits between the two bindings but
            // is not an author blank line.
            self.is_next_line_empty(
                pair[0].span().end,
                self.param_start_with_decorators(&pair[1]),
            )
        })
    }

    /// **to emit**: the comments this gap prints ahead of the param at `param_render_start` —
    /// the ones that lead it, as opposed to those trailing the *previous* param.
    ///
    /// The single definition of that split. Both the leading-comment emitter and the
    /// separator ([`Self::param_content_start`]) read it, so neither can drift from the
    /// other's idea of which comments belong to this param.
    fn leading_param_comments(
        &self,
        start: u32,
        param_render_start: u32,
        prev_comma_pos: Option<u32>,
        skip_delim: Option<u32>,
    ) -> CommentVec<'_> {
        // The previous param's anchor-line run, the same split its trailing arm claims
        // ([`Printer::gap_anchor_line_end`] — it follows a multi-line block to its closing
        // `*/` line, so a comment glued past that `*/` still trails the previous param).
        let anchor_line_end = self.gap_anchor_line_end(start, param_render_start);
        comments_to_emit_in_range(self.comments, start, param_render_start)
            .filter(|c| {
                // A comment already pulled onto the opening `(` line (first param)
                // must not be re-emitted as a leading comment here.
                if let Some(dpos) = skip_delim
                    && self.comment_on_delimiter_line(dpos, c)
                {
                    return false;
                }
                let Some(comma) = prev_comma_pos else {
                    return true; // First param - keep all comments
                };
                // A stranded after-comma block (on the comma's line, newline before
                // this param) trails the comma — emitted by the loop's
                // `push_stranded_after_comma_blocks`, not led here.
                if c.span.start >= comma
                    && self.is_stranded_after_comma_block(c, comma, param_render_start)
                {
                    return false;
                }
                // A line comment trailing the previous param — on its line, or on the
                // line of the comma that closes it — is emitted by the loop's
                // trailing-line arm, so it never leads this one. The comma reading is
                // what the item reading cannot give: an author who pushed the comma onto
                // its own line (`a⏎, // c⏎ b`) wrote the comment against the comma.
                if self.param_trailing_line_comment(c, start, anchor_line_end, Some(comma)) {
                    return false;
                }
                // Past the prev param's anchor-line run - definitely a leading comment
                if c.span.start >= anchor_line_end {
                    return true;
                }
                // On the prev param's line: only keep block comments after the comma
                // (block comments before the comma are trailing)
                c.is_block && c.span.start >= comma
            })
            .collect()
    }

    /// Whether a line comment in a param's `[param_end, next_start)` gap **trails that
    /// param** — it sits inside the gap's anchor-line run, or on the line of the comma that
    /// closes it. The one statement of that claim, asked by the trailing-line emitter and by
    /// `leading_param_comments`'s exclusion: the two must PARTITION the gap, so a spelling
    /// that drifts either drops the comment or prints it twice.
    ///
    /// `anchor_line_end` is that run's end ([`Printer::gap_anchor_line_end`]), passed in so
    /// both callers hand it the same split the *block* arms partition on. It is not
    /// `is_same_line(param_end, …)`: the run follows a multi-line block to its closing `*/`
    /// line, and a `//` written after that `*/` (`aaaa /* x⏎y */ /* c */ // d⏎,`) is on the
    /// param's rendered line even though it is two source lines below the param's end.
    /// Answering with the bare anchor left it claimed by NEITHER arm — the block arm skips
    /// it for its kind, the leading arm for its line — which is a DROP.
    ///
    /// The **comma** half is what an item-anchored reading cannot give. The comma is
    /// re-emitted structure — the printer pulls it back onto the param's line whatever the
    /// author did with it — so a `//` written after a comma the author pushed onto a line
    /// of its own (`a⏎, // c⏎ b`) belongs to that comma, and reading only the param's line
    /// hands it to the next param's leading run, re-binding it to a param it was never
    /// written against. Block comments are not on this rule: they ask the hug question
    /// instead ([`Printer::is_stranded_after_comma_block`]).
    ///
    /// ⚠️ **At most ONE can trail**, which is why the two anchors alone do not answer it:
    /// the run ends at the first line comment in the gap, and the rest leads the next param
    /// on its own line, as it does in prettier. That stop condition is
    /// [`Printer::gap_emitted_line_comment_before`], shared with the type-side comma
    /// emitter so the two can't disagree about a gap they answer the same way.
    ///
    /// `comma_pos` is the comma the printer **emits**, so this is a NON-LAST param's
    /// predicate only: under `trailingComma: 'none'` a last param's source comma is deleted,
    /// and the anchor's whole argument is that the printer pulls the comma back onto the
    /// param's line. That region belongs to the shared last-item→closer walk
    /// ([`Printer::build_trailing_gap_comments_ext`]), whose
    /// [`Printer::closer_trailing_comment_run`] asks the same question of a `//` (the param's
    /// line, since the comma is not in the output for it to be written against) inside one
    /// ordered pass that also places the block comments.
    fn param_trailing_line_comment(
        &self,
        comment: &internal::Comment,
        param_end: u32,
        anchor_line_end: u32,
        comma_pos: Option<u32>,
    ) -> bool {
        if comment.is_block {
            return false;
        }
        let anchored = comment.span.start < anchor_line_end
            || comma_pos.is_some_and(|comma| {
                comment.span.start >= comma && self.comment_on_comma_line(comma, comment)
            });
        anchored && !self.gap_emitted_line_comment_before(param_end, comment.span.start)
    }

    /// **on page**: where the param at `param_render_start`'s printed content begins in
    /// source — the position a blank-line scan over the gap before it must stop at, so that
    /// a blank line the author left ahead of that content is inside the measured range.
    ///
    /// Three answers, in order: a comment this gap leads with; else the comment the param
    /// OWNS, which prints from inside the param's own doc and so is invisible to the **to
    /// emit** axis above; else the param itself.
    ///
    /// Bounding at the first comment *in source* instead is the subtle wrong answer: that
    /// comment may be the **previous** param's trailing one (`a, // x`), which ends the
    /// previous param's line rather than starting this one's — leaving any blank line
    /// after it inside no one's range, and silently dropped.
    fn param_content_start(
        &self,
        start: u32,
        param_render_start: u32,
        prev_comma_pos: Option<u32>,
        skip_delim: Option<u32>,
        param: &internal::Expression<'_>,
    ) -> u32 {
        self.leading_param_comments(start, param_render_start, prev_comma_pos, skip_delim)
            .first()
            .map(|c| c.span.start)
            .or_else(|| {
                // Guarded to this gap: the lookup is keyed on the param's own span start,
                // which a leading decorator run sits ahead of, so a hit outside
                // `[start, param_render_start)` is not this gap's to measure against.
                self.owned_leading_comment_start(param)
                    .filter(|&p| p >= start && p < param_render_start)
            })
            .unwrap_or(param_render_start)
    }

    /// Build doc for leading comments before a parameter, through the shared
    /// leading-comment emitter ([`Printer::push_leading_comment_run`]) — so every
    /// separator here is prettier's `printLeadingComment`, read of the source around
    /// *that* comment: a space when nothing follows the `*/` on its line, a **soft
    /// `line`** when something precedes the `/*` but not follows it, a hardline only
    /// when the author isolated the comment on a line of its own.
    ///
    /// ⚠️ **The soft `line` is load-bearing and this site once lacked it.** Collapsing
    /// prettier's three separators into space-or-hardline gives one document two fixed
    /// points: a glued run the author gave its own line (`f(⏎/* c1 */ /* c2 */⏎a)`) took
    /// the hardline and forced the list open, while the same run written on the param's
    /// line stayed flat — and in a list broken on WIDTH the space glued the pair to a
    /// param prettier drops below it. One `line` answers both, because it is the only
    /// separator whose rendering follows the list's own group.
    ///
    /// `prev_comma_pos`: if Some, filter out trailing comments for the previous param
    ///
    /// `param_render_start` is where the param's rendered form begins — its first
    /// decorator when it carries parameter decorators, else the binding itself. It
    /// bounds the collection on **both** ends of the concern: only comments *before*
    /// the first decorator are leading param comments (anything interleaved with the
    /// decorators is emitted in place by `with_param_decorators`), and the final
    /// own-line/blank decision measures against it so an own-line decorator between
    /// the last comment and the binding isn't miscounted as an author blank line.
    /// Same decorator-aware anchor as `has_blank_line_between_params`.
    fn build_leading_param_comments(
        &self,
        start: u32,
        param_render_start: u32,
        prev_comma_pos: Option<u32>,
        skip_delim: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let comments =
            self.leading_param_comments(start, param_render_start, prev_comma_pos, skip_delim);
        if comments.is_empty() {
            return d.empty();
        }

        let mut parts: DocBuf = DocBuf::new();
        self.push_leading_comment_run(
            &mut parts,
            comments.iter().copied(),
            param_render_start,
            LeadingGlue::Adjacent,
            d.empty(),
        );
        d.concat(&parts)
    }

    /// Build a Doc for a class expression (`class …`, named or anonymous).
    pub(in crate::printer) fn build_class_expression_doc(
        &self,
        class_expr: &internal::ClassExpression<'_>,
    ) -> DocId {
        let d = self.d();

        // With a decorated class expression (`@dec class {}`), span.start points
        // at the first decorator's `@`, so derive the `class` keyword position
        // from after the decorators (falls back to span.start when undecorated).
        let class_keyword_start = self.find_keyword_after_decorators(
            class_expr.decorators,
            "class",
            class_expr.span.start,
        );

        // Compute heritage positions once (shared with the class-declaration printer).
        let positions = self.class_heritage_positions(
            class_keyword_start,
            class_expr.id.as_ref(),
            class_expr.type_parameters.as_ref(),
            class_expr.super_class,
            class_expr.super_type_parameters.as_ref(),
            class_expr.implements,
        );

        // Heritage layout (shared with the class-declaration printer).
        let layout = self.class_header_layout(
            &positions,
            class_expr.super_class,
            class_expr.super_type_parameters.as_ref(),
            class_expr.implements,
        );

        let mut parts = DocBuf::new();

        // Leading decorators (`@dec class {}`), each on its own line.
        if let Some(dec_doc) = self.build_decorators_doc(class_expr.decorators, class_keyword_start)
        {
            parts.push(dec_doc);
        }

        // 'class' keyword
        parts.push(d.text("class"));

        // Optional class name
        if let Some(id) = &class_expr.id {
            // Comments between `class` keyword and name
            parts.push(self.build_keyword_to_name_comments(class_keyword_start, id.span.start));
            parts.push(self.build_identifier_doc(id));
        }
        // The name→body and `class`→body gaps are NOT emitted here: `header_end` already
        // falls back to the name's end (or the `class` keyword's) when there is no heritage
        // and no type params, so `build_class_header_doc` resolves every class shape's
        // header→`{` gap through the one seam. Emitting them here instead is what made the
        // bare-name and anonymous forms answer that gap differently from their siblings.

        // Type parameters (`class<T>`) and the gap before them — shared with the
        // declaration printer, which is what keeps the anonymous arm's open gap a
        // single hole.
        self.push_class_type_params(
            &mut parts,
            class_expr.type_parameters.as_ref(),
            class_expr
                .id
                .as_ref()
                .map_or(ClassTypeParamsGap::Keyword(class_keyword_start), |id| {
                    ClassTypeParamsGap::Name(id.span.end)
                }),
        );

        // Build heritage docs (shared with the class-declaration printer).
        let extends_doc = self.build_class_extends_doc(
            class_expr.super_class,
            class_expr.super_type_parameters.as_ref(),
            positions.extends_keyword_start,
        );
        let implements_doc = self.build_class_implements_doc(
            class_expr.implements,
            layout.is_group(),
            positions.implements_keyword_start,
        );

        // Assemble the header (group-wrapped); the body is appended outside the
        // group so its hardlines don't affect the header's fit check.
        // Resolved once for the header placement and the body emission, as in the
        // declaration printer.
        let frozen_body = self.gap_frozen_span(positions.header_end, class_expr.body.span);
        let header_doc = self.build_class_header_doc(
            parts,
            &positions,
            extends_doc,
            implements_doc,
            class_expr.implements,
            ClassHeaderOptions {
                body_is_empty: class_expr.body.body.is_empty(),
                body_start: class_expr.body.span.start,
                layout,
                body_frozen: frozen_body.is_some(),
            },
        );

        d.concat(&[
            header_doc,
            self.build_class_body_doc(&class_expr.body, frozen_body),
        ])
    }
}
