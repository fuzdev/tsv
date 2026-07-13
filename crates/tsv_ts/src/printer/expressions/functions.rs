// Function expression printing for TypeScript
//
// This module handles function-related expressions:
// - Arrow function expressions (async, parameters, return types, body)
// - Function expressions (parameters, return types, body)
//
// Note: Block statements are in blocks.rs as a reusable utility

use crate::ast::internal;
use crate::printer::ArrowChainContext;
use crate::printer::calls::arg_predicates::arrow_has_trailing_param_comments;
use crate::printer::layout::hang_after_operator;
use crate::printer::needs_parens::leftmost_no_lookahead;
use crate::printer::types::helpers::is_huggable_type;
use crate::printer::{CommentFilter, CommentSpacing, LeadingGlue};
use crate::printer::{
    CommentVec, ParenContext, Printer, has_newline_before_position,
    is_multiline_template_expression, unwrap_parenthesized,
};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;
use tsv_lang::doc::{DocBuf, GroupId};
use tsv_lang::source_scan::find_char_skipping_comments;

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

        // Calculate signature end position (after `)` or return type).
        // This is where comments BEFORE `=>` start.
        let sig_end = if let Some(rt) = &arrow.return_type {
            rt.span.end
        } else if let Some(params_start) = arrow.params_start {
            // Find closing `)` to get accurate boundary
            self.find_closing_paren(params_start, arrow.body.span().start)
                .unwrap_or_else(|| arrow.body.span().start)
        } else {
            // No parens (single param arrow like `x => x`) - use param end
            arrow
                .params
                .last()
                .map_or(arrow.span.start, |p| p.span().end)
        };

        // The `=>` token position (parser-recorded) distinguishes:
        // - Comments between sig_end and `=>` → print BEFORE `=>`
        // - Comments between `=>` and body → print AFTER `=>`
        let arrow_pos = arrow.arrow_token;
        let arrow_end = arrow_pos + "=>".len() as u32;

        // Build the signature (async + type params + params + return type) via the
        // shared builder, then append any comment between the signature and `=>`
        // (`(x) /* c */ =>`). The common (no-comment) path uses the signature doc
        // directly — no extra Vec.
        let sig_inner = self.build_arrow_signature_doc(arrow);
        let sig_doc = if self.has_comments_between(sig_end, arrow_pos) {
            let mut sig_parts: DocBuf = smallvec![sig_inner];
            for comment in comments_in_range(self.comments, sig_end, arrow_pos) {
                sig_parts.push(d.text(" "));
                sig_parts.push(self.build_comment_doc(comment));
            }
            d.concat(&sig_parts)
        } else {
            sig_inner
        };

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
            if self.has_comments_between(arrow_end, body_start) {
                for comment in comments_in_range(self.comments, arrow_end, body_start) {
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
        let has_post_arrow_comments = self.has_comments_between(arrow_end, body_start);

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
        let body_arrow_has_trailing_param_comments =
            if let internal::Expression::ArrowFunctionExpression(body_arrow) = expr {
                let arrow_token = body_arrow.arrow_token;
                arrow_has_trailing_param_comments(body_arrow, arrow_token, |start, end| {
                    self.has_comments_between(start, end)
                })
            } else {
                false
            };

        // Inline block comments don't prevent hugging — only own-line comments do.
        // `() => /* comment */ ({...})` hugs (inline block comment)
        // `() =>\n  /* comment */\n  ({...})` breaks (own-line comment)
        let should_hug = !has_own_line_comment
            && (should_hug_arrow_body(expr) || is_template_on_same_line(self.source, expr))
            && !chain_has_return_type
            && !body_arrow_has_trailing_param_comments;

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
            let body_doc = self.build_with_in_curried(true, || self.build_arrow_body_doc(expr));
            parts.push(d.concat(&[d.hardline(), body_doc]));
        } else if is_arrow_body && body_arrow_has_trailing_param_comments {
            // Nested arrow with trailing param comments - first level gets indent,
            // subsequent levels align (use curried pattern)
            // (a, // c) => (b, // c) => {}
            // becomes:
            // (a, // c) =>
            //     (b, // c) =>
            //     (c, // c) => {}
            let body_doc = self.build_with_in_curried(true, || self.build_arrow_body_doc(expr));
            parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
        } else if self.in_curried_typed_arrow.get() {
            // Innermost arrow in curried chain - body is NOT another arrow.
            // This needs indent since it's the final expression.
            // Reset flag so arrows inside the body (e.g. callback args) aren't
            // treated as part of the curried chain; restore to `true` (its value on
            // entry, since this arm is reached only when the flag is set) afterward.
            let body_doc = self.build_with_in_curried(false, || self.build_arrow_body_doc(expr));
            parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
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
            if d.will_break(body_doc) {
                // Body has hardlines (multiline template in ternary, etc.)
                // Use normal break layout — no parens needed
                parts.push(hang_after_operator(d, body_doc));
            } else {
                parts.push(d.text(" "));
                parts.push(d.group(d.concat(&[
                    d.if_break(d.empty(), d.text("(")),
                    d.indent(d.concat(&[d.softline(), body_doc])),
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
        let has_post_arrow_comments = self.has_comments_between(arrow_end, body_start);

        if has_post_arrow_comments {
            // Build comments doc
            let mut comment_parts: DocBuf = DocBuf::new();
            for comment in comments_in_range(self.comments, arrow_end, body_start) {
                comment_parts.push(d.text(" "));
                comment_parts.push(self.build_comment_doc(comment));
            }
            parts.push(d.concat(&comment_parts));
        }

        parts.push(d.text(" "));
        // A block body terminates any curried-arrow chain — arrows nested
        // inside it (callbacks, object-property arrows) are NOT part of the
        // chain, so clear the flag so they aren't force-broken after `=>`.
        // Mirrors the innermost expression-body case above.
        let block_doc = self.build_with_in_curried(false, || self.build_block_statement_doc(block));
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
        !self.has_comments_between(arrow.span.start, arrow.span.end)
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
        let terminal: &internal::ArrowFunctionBody<'_> = loop {
            // Each signature is its own group so its params break independently of
            // the chain (prettier wraps each `printArrowFunctionSignature` in a
            // group): when the heads break onto separate lines, the params stay
            // flat unless a single signature genuinely overflows.
            sig_docs.push(d.group(self.build_arrow_signature_doc(current)));
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
            d.indent_if_break(body_part, GroupId::ArrowChain, false),
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
        // Preserve a block comment between `)` and the return type `:`
        // (`(x) /* c */ : T => ...`); prettier adds a space before `:`.
        let comment_prefix =
            self.build_paren_to_return_type_comments(params_start, annotation.span.start);

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

    /// Build doc for arrow function type params.
    ///
    /// The brackets are always wrapped in their own group so they break
    /// independently of the rest of the signature — matching Prettier's
    /// `printTypeParameters`, which always returns a `group([...])` (or an inline
    /// form). This is what keeps `<T>` inline while only the return type expands,
    /// regardless of whether the arrow has parameters.
    fn build_type_params_doc_for_arrow(
        &self,
        decl: &internal::TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        if decl.params.is_empty() {
            return d.text("<>");
        }

        // Expanding comments (e.g. a line comment trailing `<`) force the shared
        // multiline layout — without this the arrow's own type-param printer drops
        // the comment (content loss). The disambiguation trailing comma is moot here
        // since the multiline form always emits one.
        if self.has_expanding_comments_in_type_param_declaration(decl) {
            let inner = self.build_type_parameter_declaration_doc_with_line_comments(decl);
            return d.group(inner);
        }

        let param_docs: DocBuf = decl
            .params
            .iter()
            .map(|param| self.build_type_parameter_doc(param))
            .collect();

        // A bare `<T>` is the canonical form everywhere. Prettier forces a trailing
        // comma (`<T,>`) on single-unconstrained arrow type params to stay valid as
        // TSX, but tsv never emits TSX and Svelte's parser accepts the bare form in
        // every TS position, so the disambiguation is moot — see the
        // single_type_param_prettier_divergence fixture. The trailing comma added
        // here only appears when the group breaks across lines.
        let inner_parts = d.join_doc(param_docs, d.comma_line());

        let brackets_doc = d.concat(&[
            d.text("<"),
            d.indent_softline(inner_parts),
            d.softline(),
            d.text(">"),
        ]);

        d.group(brackets_doc)
    }

    /// Build doc for arrow params NOT in their own group (outer signature group controls breaking)
    ///
    /// Structure matches prettier's function-parameters.js:
    /// `[typeParams, "(", indent([softline, ...params]), ifBreak(","), softline, ")"]`
    fn build_arrow_params_doc_ungrouped(
        &self,
        arrow: &internal::ArrowFunctionExpression<'_>,
    ) -> DocId {
        let params_start = arrow.params_start;

        // Compute trailing comments boundary for params
        // IMPORTANT: Stop at `)` not at return type or body start
        // Comments between `)` and `=>` are handled separately by the arrow printer
        let trailing_comments_end = if let Some(ps) = params_start {
            // Find the closing `)` position
            let body_start = arrow.body.span().start;
            self.find_closing_paren(ps, body_start)
        } else {
            // No parens - use param end as boundary
            arrow.params.last().map(|p| p.span().end)
        };

        // Delegate to shared implementation
        self.build_params_doc_with_comments(arrow.params, params_start, trailing_comments_end)
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

        // Async keyword if present
        if arrow.r#async {
            parts.push(d.text("async "));
        }

        // Type parameters: always their own group so they break independently of
        // the rest of the signature (Prettier's printTypeParameters semantics).
        if let Some(tp) = &arrow.type_parameters {
            parts.push(self.build_type_params_doc_for_arrow(tp));

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

    /// Check if any param has a trailing line comment or own-line block comment
    fn has_trailing_line_comment_in_params(
        &self,
        params: &[internal::Expression<'_>],
        trailing_comments_end: Option<u32>,
    ) -> bool {
        params.iter().enumerate().any(|(i, param)| {
            let trailing_end = self.param_trailing_end(params, i, trailing_comments_end);
            if self.has_line_comments_between(param.span().end, trailing_end) {
                return true;
            }
            // For the last param, also check for own-line block comments before `)`
            if i == params.len() - 1 {
                comments_in_range(self.comments, param.span().end, trailing_end)
                    .any(|c| c.is_block && !self.is_same_line(param.span().end, c.span.start))
            } else {
                false
            }
        })
    }

    fn param_trailing_end(
        &self,
        params: &[internal::Expression<'_>],
        index: usize,
        trailing_comments_end: Option<u32>,
    ) -> u32 {
        if index + 1 < params.len() {
            params[index + 1].span().start
        } else {
            trailing_comments_end.unwrap_or_else(|| params[index].span().end)
        }
    }

    /// Build doc for arrow function body expression.
    fn build_arrow_body_doc(&self, expr: &internal::Expression<'_>) -> DocId {
        // Expand-last-arg body reuse: when the enclosing call/new expand-last path has
        // pre-built this exact body (to also compose the break-body state), reuse that
        // DocId instead of rebuilding — rebuilding here *and* separately recurses into
        // itself for `f(lead, x => f(lead, y => …))`, making the doc-node count
        // O(2^depth). See the `arrow_body_inject` field.
        if let Some((span, doc)) = self.arrow_body_inject.get()
            && span == expr.span().start
        {
            return doc;
        }
        let d = self.d();
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
            return doc;
        }

        // Conditional expressions: parens when inline, none when on own line.
        // The primary ternary path is shouldAddParensIfNotBreak in
        // build_arrow_doc_wrapping. This branch handles ternaries reached
        // via other callers (curried innermost arrow, post-arrow comments)
        // where the body always breaks via hardline — if_break selects
        // the break variant (no parens).
        if matches!(expr, internal::Expression::ConditionalExpression(_)) {
            let body_doc = self.build_expression_doc(expr);
            // If body contains hardlines (will definitely break), no parens
            if d.will_break(body_doc) {
                return body_doc;
            }
            // Otherwise, use if_break to check enclosing group
            return d.if_break(body_doc, d.parens(body_doc));
        }

        // Standard cases: objects and assignments always need parens
        if self.needs_parens(expr, ParenContext::ArrowBody) {
            d.parens(self.build_expression_doc(expr))
        } else {
            self.build_expression_doc(expr)
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
            comments_in_range(self.comments, sig_end, body_start),
            body_start,
            LeadingGlue::Adjacent,
        );

        // Add the body expression
        parts.push(self.build_arrow_body_doc(expr));

        d.concat(&parts)
    }

    /// Check if any comment between `=>` and body is on its own line.
    ///
    /// Matches Prettier's `hasLeadingOwnLineComment` which checks `hasNewline(text, locEnd(comment))`
    /// — whether there's a newline after each comment. Inline block comments like
    /// `=> /* c */ expr` have no newline after them (returns false). Own-line comments
    /// and line comments have a newline after (returns true).
    pub(crate) fn has_own_line_post_arrow_comment(&self, sig_end: u32, body_start: u32) -> bool {
        for comment in comments_in_range(self.comments, sig_end, body_start) {
            // A line comment, a multiline block, or a block that starts on its own
            // line (a newline precedes it) forces the body onto its own line. A
            // single-line block glued to `=>` keeps the body hugged even when the
            // body follows on the next source line (`=> /* c */⏎expr` → `=> /* c */ expr`).
            if self.comment_forces_own_line(comment) {
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
        for comment in comments_in_range(self.comments, sig_end, body_start) {
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

        // Async keyword if present
        if func.r#async {
            parts.push(d.text("async "));
        }

        // Function keyword
        parts.push(d.text("function"));

        // Generator asterisk
        if func.generator {
            parts.push(d.text("*"));
        }

        // Optional function name
        if let Some(id) = &func.id {
            // Comments between keywords and the name (same as FunctionDeclaration)
            parts.push(self.build_keyword_to_name_comments(func.span.start, id.span.start));
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
            parts.push(self.build_keyword_to_name_comments(func.span.start, next_start));
        }

        // Type params, params, return type, and body (signature_doc handles type params)
        parts.push(self.build_function_doc_body(func));

        d.concat(&parts)
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
        // [window_start, window_end], and `comments_in_range` only yields comments
        // fully inside its range — so when no comment lies inside the window, every
        // sub-query is provably empty/false. Skip them all, including the per-gap
        // `find_comma_after` trivia scans, whose results feed only comment placement.
        let comments_present = {
            let window_start = params_start.unwrap_or_else(|| params[0].span().start);
            let last_end = params[params.len() - 1].span().end;
            let window_end = trailing_comments_end.map_or(last_end, |end| end.max(last_end));
            self.has_comments_between(window_start, window_end)
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
            || !self.has_comments_between(
                params_start.unwrap_or_else(|| params[0].span().start),
                params[0].span().start,
            );
        let no_trailing_comments = !comments_present
            || trailing_comments_end
                .is_none_or(|end| !self.has_comments_between(params[0].span().end, end));
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
        // below. See conformance_prettier.md §Comment relocation and
        // open_paren_line_comment_prettier_divergence.
        let (paren_prefix, paren_pull_pos) = match params_start {
            Some(open) if comments_present => {
                self.delimiter_line_comment_prefix(open, params[0].span().start)
            }
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
        let force_break = has_trailing_line_comment
            || has_leading_own_line_comment
            || should_break_for_param_properties
            || has_blank_line_between_params
            || paren_pull_pos.is_some();

        let mut inner_parts = d.pooled_docbuf();
        // Block comment trailing the last param after its source comma — emitted past
        // where the comma was, after the loop (no trailing comma; trailingComma: 'none').
        let mut last_after_comma_docs: DocBuf = DocBuf::new();
        for (i, param) in params.iter().enumerate() {
            let param_start = param.span().start;
            let is_last = i == params.len() - 1;

            // Check for leading comments before this param
            let search_start = if i == 0 {
                // First param: search from after '(' (position + 1)
                params_start.map_or(param_start, |pos| pos + 1)
            } else {
                // Subsequent params: search from after the previous param
                params[i - 1].span().end
            };

            // Add separator before non-first params
            if i > 0 {
                // Use hardline when forcing break (trailing line comments or param properties)
                if force_break {
                    // Preserve a blank line the author left before this param's
                    // leading comment (or the param itself) — prettier keeps one blank
                    // line in the expanded list. `search_start` is the previous param's
                    // end, so the gap spans the comma too.
                    let check_pos = if comments_present {
                        comments_in_range(self.comments, search_start, param_start)
                            .next()
                            .map_or(param_start, |c| c.span.start)
                    } else {
                        param_start
                    };
                    self.push_blank_preserving_hardline(&mut inner_parts, search_start, check_pos);
                } else {
                    inner_parts.push(d.line());
                }
            }

            // Add leading comments for this param
            // Use proper line breaks for line comments on their own line
            // For non-first params, find comma position to filter properly
            if comments_present {
                let prev_comma_pos = if i > 0 {
                    self.find_comma_after(params[i - 1].span().end)
                } else {
                    None
                };
                // The first param excludes any comment already pulled onto the `(`
                // line by `delimiter_line_comment_prefix`, so it isn't emitted twice.
                let skip_delim = if i == 0 { paren_pull_pos } else { None };
                inner_parts.push(self.build_leading_param_comments(
                    search_start,
                    self.param_start_with_decorators(param),
                    prev_comma_pos,
                    skip_delim,
                ));
            }

            // Use FunctionParameter context for object patterns
            inner_parts.push(self.build_function_parameter_doc(param));

            // Handle trailing same-line comments
            let search_end = if is_last {
                self.param_trailing_end(params, i, trailing_comments_end)
            } else {
                params[i + 1].span().start
            };

            // Find comma position. For the last param, locate a source trailing
            // comma (within the trailing range) so an after-comma block comment is
            // preserved after the comma rather than relocated before it.
            // Consumed only by comment placement, so the zero-comment gate skips the scan.
            let comma_pos = if !comments_present {
                None
            } else if !is_last {
                self.find_comma_after(param.span().end)
            } else {
                self.find_comma_after(param.span().end)
                    .filter(|cp| *cp < search_end)
            };

            // Collect same-line comments
            let same_line_comments: CommentVec<'_> = if comments_present {
                comments_in_range(self.comments, param.span().end, search_end)
                    .filter(|c| self.is_same_line(param.span().end, c.span.start))
                    .collect()
            } else {
                CommentVec::new()
            };

            // Block comments BEFORE comma go before comma
            for comment in same_line_comments
                .iter()
                .filter(|c| c.is_block && comma_pos.is_none_or(|pos| c.span.start < pos))
            {
                inner_parts.push(d.text(" "));
                inner_parts.push(self.build_comment_doc(comment));
            }

            // Add inter-param separator comma (only between params; the last param
            // gets no trailing comma — trailingComma: 'none').
            let needs_comma = !is_last;
            if needs_comma {
                inner_parts.push(d.text(","));
                // A stranded after-comma block (on the comma's line, but a newline
                // before the next param) trails the comma — preserving the author's
                // placement, matching call args / declarators (prettier relocates it
                // before the comma). A block hugging the next param leads it instead
                // (as a leading comment). See conformance_prettier.md §Comment relocation.
                if let Some(cp) = comma_pos {
                    let next_start = self.param_start_with_decorators(&params[i + 1]);
                    self.push_stranded_after_comma_blocks(&mut inner_parts, cp, next_start);
                }
            }

            // Block comments AFTER the comma on the last param: preserve their position.
            // The last param has no trailing comma, so an after-comma block is deferred to
            // last_after_comma_docs (emitted after the loop, past where the comma was).
            if is_last {
                let after: CommentVec<'_> = same_line_comments
                    .iter()
                    .filter(|c| c.is_block && comma_pos.is_some_and(|pos| c.span.start > pos))
                    .copied()
                    .collect();
                for comment in after {
                    last_after_comma_docs.push(d.text(" "));
                    last_after_comma_docs.push(self.build_comment_doc(comment));
                }
            }

            // Line comments (same-line) go after comma (excluded from width)
            // Block comments AFTER comma are handled as leading for next param
            for comment in same_line_comments.iter().filter(|c| !c.is_block) {
                inner_parts.push(self.build_trailing_line_comment_doc(comment));
            }

            // Own-line comments (on their own line after last param, before `)`)
            // Only for the last param - non-last param comments are handled as leading for next param
            if is_last && comments_present {
                let mut prev_own = param.span().end;
                for comment in comments_in_range(self.comments, param.span().end, search_end)
                    .filter(|c| !self.is_same_line(param.span().end, c.span.start))
                {
                    // Preserve an author blank line before the own-line trailing comment.
                    self.push_blank_preserving_hardline(
                        &mut inner_parts,
                        prev_own,
                        comment.span.start,
                    );
                    inner_parts.push(self.build_comment_doc(comment));
                    prev_own = comment.span.end;
                }
            }
        }

        // No group - outer signature group controls breaking
        let mut result: DocBuf = smallvec![d.text("(")];
        // A pulled `( // c` comment renders on the `(` line before the break.
        result.extend(paren_prefix);

        if force_break {
            // When forcing break (trailing comments or param properties), use hardlines.
            // No trailing comma (trailingComma: 'none'); a preserved after-comma block
            // comment on the last param still lands past where the comma was.
            inner_parts.append(&mut last_after_comma_docs);
            result.push(d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])));
            result.push(d.hardline());
        } else {
            result.push(d.indent_softline(d.concat(&inner_parts)));
            // No trailing comma (trailingComma: 'none').
            // Preserved after-comma block comment(s) on the last param
            result.append(&mut last_after_comma_docs);
            result.push(d.softline());
        }

        result.push(d.text(")"));

        d.concat(&result)
    }

    /// Check if any param has a leading line comment on its own line
    fn has_leading_own_line_comment_in_params(
        &self,
        params: &[internal::Expression<'_>],
        params_start: Option<u32>,
    ) -> bool {
        for (i, param) in params.iter().enumerate() {
            let search_start = if i == 0 {
                params_start.map_or_else(|| param.span().start, |pos| pos + 1)
            } else {
                params[i - 1].span().end
            };

            // Check if there's a line comment on its own line before this param
            if self.has_own_line_comment_between(search_start, param.span().start) {
                return true;
            }
        }
        false
    }

    /// Whether a comment between two params forces the param list to expand.
    ///
    /// A **line** comment always forces it (it runs to end-of-line, so the
    /// following param can't share the line). A **block** comment forces it only
    /// when it sits on its OWN line — isolated from *both* neighbors: not
    /// inline-adjacent to the previous param at `start` (`a /* c */,`), nor to the
    /// following one at `end` (`/* c */ b`). Either adjacency stays inline, matching
    /// prettier, which collapses `a,⏎/* c */ b` and `a /* c */,⏎b` both back to
    /// `a, /* c */ b`. (Same isolated-from-both rule as the intersection member
    /// gate; keying only on the following param over-expanded a block that trailed
    /// the previous one before its comma.)
    fn has_own_line_comment_between(&self, start: u32, end: u32) -> bool {
        comments_in_range(self.comments, start, end)
            .any(|c| self.comment_isolated_from_neighbors(start, c, end))
    }

    /// Whether the author left a blank line between any two consecutive params.
    /// A bare blank is authorial intent (like one around a comment), so it forces
    /// the param list to expand and the separator emission preserves it — matching
    /// prettier. Shared by regular function params and the type-level param lists
    /// (function/constructor types, method/call/construct signatures).
    pub(in crate::printer) fn has_blank_line_between_params(
        &self,
        params: &[internal::Expression<'_>],
    ) -> bool {
        params.windows(2).any(|pair| {
            // Measure to the next param's first decorator, not its binding — a
            // decorator written on its own line sits between the two bindings but
            // is not an author blank line.
            self.has_blank_line_between(
                pair[0].span().end,
                self.param_start_with_decorators(&pair[1]),
            )
        })
    }

    /// Build doc for leading comments before a parameter
    /// Handles line comments on their own line with proper hardlines
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
        let comments: CommentVec<'_> = comments_in_range(self.comments, start, param_render_start)
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
                if c.is_block
                    && c.span.start >= comma
                    && self.is_stranded_after_comma_block(c, comma, param_render_start)
                {
                    return false;
                }
                // Different line from prev param - definitely a leading comment
                if !self.is_same_line(start, c.span.start) {
                    return true;
                }
                // Same line as prev param: only keep block comments after the comma
                // (line comments go in line_suffix, block comments before comma are trailing)
                c.is_block && c.span.start >= comma
            })
            .collect();
        if comments.is_empty() {
            return d.empty();
        }

        // Neighbor bounds for the comment at index `i`: its predecessor is the `(`
        // (== `start`) for the first comment, else the previous comment's end; its
        // successor is the next comment's start, else the param's rendered start.
        let prev_of = |i: usize| {
            if i == 0 {
                start
            } else {
                comments[i - 1].span.end
            }
        };
        let next_of = |i: usize| {
            comments
                .get(i + 1)
                .map_or(param_render_start, |c| c.span.start)
        };

        // For the first param, prettier collapses leading block comment(s) inline
        // (`(/* c */ x)`) UNLESS one is isolated on its own line — a line break on both
        // sides — or is a line comment (the same isolated-from-neighbors rule the outer
        // `has_leading_own_line_comment` gate uses). When nothing is isolated, every
        // separator is a space so the group stays flat; an isolated/line comment
        // re-expands the run with hardlines tracking the source newlines. A non-first
        // param keeps the legacy own-line behavior — its comma-relocation cases differ.
        let first_param = prev_comma_pos.is_none();
        let force_expand = first_param
            && comments
                .iter()
                .enumerate()
                .any(|(i, c)| self.comment_isolated_from_neighbors(prev_of(i), c, next_of(i)));

        let mut parts: DocBuf = DocBuf::new();

        for (i, comment) in comments.iter().enumerate() {
            let prev_pos = prev_of(i);
            let on_own_line = !self.is_same_line(prev_pos, comment.span.start);

            if i > 0 {
                if first_param {
                    // Collapse mode uses a space; expand mode preserves the source
                    // newline (blank-aware). Two comments sharing a line stay spaced.
                    if force_expand && on_own_line {
                        self.push_blank_preserving_hardline(
                            &mut parts,
                            prev_pos,
                            comment.span.start,
                        );
                    } else {
                        parts.push(d.text(" "));
                    }
                } else if on_own_line {
                    // Non-first param (legacy): own-line comment gets a hardline,
                    // preserving a blank line the author left between the two comments.
                    self.push_blank_preserving_hardline(&mut parts, prev_pos, comment.span.start);
                }
            }
            parts.push(self.build_comment_doc(comment));
        }

        // Separator between the last leading comment and the param. Measure to the
        // param's rendered start (its first own-line decorator, if any) so a decorator
        // between the comment and the binding isn't miscounted as an author blank line.
        let last_comment_end = comments.last().map_or(start, |c| c.span.end);
        let param_on_own_line = !self.is_same_line(last_comment_end, param_render_start);

        // First param collapses inline unless an isolated/line comment forced expansion;
        // non-first stays inline only when the param shares the last comment's line.
        let collapse_to_param = if first_param {
            !force_expand || !param_on_own_line
        } else {
            !param_on_own_line
        };

        if collapse_to_param {
            // Inline - add space after comment
            parts.push(d.text(" "));
        } else {
            // Preserve a blank line between the last leading comment and the param.
            self.push_blank_preserving_hardline(&mut parts, last_comment_end, param_render_start);
        }

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

        // Determine group mode: structural reasons OR heritage comments
        let has_heritage_comments = positions
            .first_heritage_start
            .is_some_and(|hs| self.has_comments_between(positions.pre_heritage_end, hs))
            || positions.extends_clause_end.is_some_and(|ext_end| {
                !class_expr.implements.is_empty()
                    && self.has_comments_between(ext_end, class_expr.implements[0].span.start)
            });
        let group_mode = self.should_class_group_mode(
            class_expr.super_class,
            class_expr.super_type_parameters.as_ref(),
            class_expr.implements,
        ) || has_heritage_comments;

        let has_heritage_line_comments = positions
            .first_heritage_start
            .is_some_and(|hs| self.has_line_comments_between(positions.pre_heritage_end, hs))
            || positions.extends_clause_end.is_some_and(|ext_end| {
                !class_expr.implements.is_empty()
                    && self.has_line_comments_between(ext_end, class_expr.implements[0].span.start)
            });

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

            // Comments between name and type params: `class A/* c */ <T> {}`
            // Line comments get a hardline to prevent absorbing type params as comment text
            if let Some(type_params) = &class_expr.type_parameters {
                self.push_name_to_type_params_comments(
                    &mut parts,
                    id.span.end,
                    type_params.span.start,
                    CommentSpacing::Trailing,
                );
            } else if positions.first_heritage_start.is_none() {
                // No type params, no heritage: comments between name and body `class A /* c */ {}`
                // Heritage path handles name→heritage comments when heritage exists
                self.push_name_to_type_params_comments(
                    &mut parts,
                    id.span.end,
                    class_expr.body.span.start,
                    CommentSpacing::Leading,
                );
            }
        } else if class_expr.type_parameters.is_none()
            && class_expr.super_class.is_none()
            && class_expr.implements.is_empty()
        {
            // Anonymous class without heritage: extract comments between `class` and body
            // `class /* c */ {}` — heritage comment handling covers the heritage case
            if self.has_line_comments_between(class_keyword_start, class_expr.body.span.start) {
                // Line comment: hardline after, body on new line without extra space
                // `class // c\n{}` — no heritage/type params, so return early
                parts.push(self.build_name_to_type_params_comments(
                    class_keyword_start,
                    class_expr.body.span.start,
                    CommentSpacing::Leading,
                ));
                parts.push(self.build_class_body_doc(&class_expr.body, false));
                return d.concat(&parts);
            }
            if let Some(comment_doc) = self.build_comments_between_filtered_opt(
                class_keyword_start,
                class_expr.body.span.start,
                CommentSpacing::Leading,
                CommentFilter::All,
            ) {
                parts.push(comment_doc);
            }
        }

        // Type parameters (TypeScript generics): class<T>
        // Use _wrapping version for width-based line breaking
        if let Some(type_params) = &class_expr.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Build heritage docs (shared with the class-declaration printer).
        let extends_doc = self.build_class_extends_doc(
            class_expr.super_class,
            class_expr.super_type_parameters.as_ref(),
            positions.extends_keyword_start,
        );
        let implements_doc = self.build_class_implements_doc(
            class_expr.implements,
            group_mode,
            positions.implements_keyword_start,
        );

        // The bare name→body / anonymous→body comments are emitted above, so only
        // scan for header→body comments here when heritage or type params exist.
        let emit_pre_body_comments =
            positions.first_heritage_start.is_some() || class_expr.type_parameters.is_some();

        // Assemble the header (group-wrapped); the body is appended outside the
        // group so its hardlines don't affect the header's fit check.
        let header_doc = self.build_class_header_doc(
            parts,
            &positions,
            extends_doc,
            implements_doc,
            class_expr.implements,
            class_expr.body.body.is_empty(),
            class_expr.body.span.start,
            group_mode,
            has_heritage_line_comments,
            emit_pre_body_comments,
        );

        d.concat(&[
            header_doc,
            self.build_class_body_doc(&class_expr.body, false),
        ])
    }
}
