// Cover-grammar conversion: turn an already-parsed `Expression` into an
// assignable pattern (`{a, b} = obj`, `[x] = arr`, arrow params). Pure AST
// rewriting — no token consumption.

use crate::ast::{
    ArrayPattern, AssignmentOperator, AssignmentPattern, AstView, ExprId, Expression,
    ExpressionKind, ObjProp, ObjectPattern, ObjectPatternProperty, ObjectProperty, Property,
    PropertyKind, RestElement, Run,
};
use tsv_lang::{Error, Span};

use super::{ExprRet, Parser, flush_run};

/// Which assignable position `refine_assignable` is converting for — the axis that
/// decides whether a type-assertion or JSDoc-cast target is allowed, and what a
/// non-simple target does. `Assignment` and `ForHead` (a no-declaration for-in/of
/// head) both accept an assertion or a JSDoc cast over a simple target
/// (`(x as T) = …`, `for (x! of …)`, `/** @type {T} */ (x) = …`) — tsc's
/// `checkReferenceExpression` reads the two positions with one rule — and differ
/// only on a non-simple target, which `Assignment` defers and `ForHead` rejects;
/// `Binding` (function params, destructuring bindings) rejects every wrapper, and a
/// member or parenthesized target too; `SveltePattern` (a Svelte block or `{@const}`
/// pattern) rejects the wrappers but takes those two. acorn-typescript's `isBinding`
/// split puts the for-head on the binding side; tsv does not follow it there.
#[derive(Clone, Copy)]
pub(in crate::parser) enum AssignableContext {
    /// `… = rhs` — a type-assertion wrapping a *simple* target is itself a valid target.
    Assignment,
    /// A no-declaration for-in/of head (`for ((x) of …)`) — an
    /// `AssignmentTargetType`/`LeftHandSideExpression` position. Reads a target the
    /// way `Assignment` does — a type assertion over a simple target is itself a
    /// target (`for (x! of …)`, `for (x as T of …)`), as is a JSDoc
    /// `/** @type {T} */ (x)` cast, which is *transparent grouping* (no node in
    /// acorn's AST) — but keeps rejecting a non-simple target (`for (f() of …)`),
    /// which `Assignment` defers.
    ForHead,
    /// Binding position — a `BindingElement` / `BindingRestElement`, which ecma262
    /// spells as a `BindingIdentifier` or a nested `BindingPattern` and nothing else. A
    /// type-assertion or (parenthesized) JSDoc-cast target is rejected, and so are a
    /// member expression (`let [a.b] = x`) and a parenthesized target
    /// (`function f([(x)])`, read off [`Parser::grouping_parens`]): the grammar has no
    /// production for either, so each is a syntax error rather than a deferred
    /// early error, and the paren-free reprint would change the program.
    Binding,
    /// A Svelte block or tag pattern — an `{#each}` context, a `{:then}` / `{:catch}`
    /// value, a `{@const}` id. Svelte reads each as the LEFT side of an assignment
    /// (`read_pattern` parses `${pattern} = 1`; `{@const}` parses `id = init`), so a
    /// member expression and a parenthesized target are the
    /// `DestructuringAssignmentTarget`s it admits, and tsv accepts both to match its
    /// AST. Every other target is graded as `Binding` grades it.
    SveltePattern,
}

/// What the cover-grammar conversion reads of the node it converts, as `Copy` pieces, so the
/// walk holds no borrow of the stores across its own pushes. The node may be a stored row or a
/// value the parse never placed; [`CoverHead::of`] reads both.
#[derive(Clone, Copy)]
struct CoverHead {
    span: Span,
    /// Whether a for-in/of head's optional-chain target rejects here (answered once).
    for_head_chain: bool,
    shape: CoverShape,
}

/// The node kinds the conversion distinguishes; see [`Parser::refine_assignable`].
#[derive(Clone, Copy)]
enum CoverShape {
    Identifier,
    Member,
    Object {
        properties: Run<ObjProp>,
        spread_trailing_comma: bool,
    },
    Array {
        elements: Run<Option<ExprId>>,
        spread_trailing_comma: bool,
    },
    Spread {
        argument: ExprId,
        span: Span,
    },
    Assignment {
        left: ExprId,
        right: ExprId,
        operator: AssignmentOperator,
    },
    AlreadyPattern,
    /// An `as` / `satisfies` / `!` / `<T>` or JSDoc-cast wrapper, whether it wraps a simple
    /// target.
    Wrapper {
        simple_target: bool,
    },
    Other,
}

impl CoverHead {
    /// Reads `expr` into its pieces, asking the two questions that walk the node's own
    /// subtree here, while the read is free.
    fn of(view: AstView<'_>, expr: &Expression, context: AssignableContext) -> Self {
        let for_head_chain = matches!(context, AssignableContext::ForHead)
            && expr.skip_type_assertions(view).has_optional_in_chain(view);
        let shape = match &expr.kind {
            ExpressionKind::Identifier(_) => CoverShape::Identifier,
            ExpressionKind::MemberExpression(_) => CoverShape::Member,
            ExpressionKind::ObjectExpression(obj) => CoverShape::Object {
                properties: obj.properties,
                spread_trailing_comma: obj.spread_trailing_comma,
            },
            ExpressionKind::ArrayExpression(arr) => CoverShape::Array {
                elements: arr.elements,
                spread_trailing_comma: arr.spread_trailing_comma,
            },
            ExpressionKind::SpreadElement(spread) => CoverShape::Spread {
                argument: spread.argument,
                span: spread.span,
            },
            ExpressionKind::AssignmentExpression(assign) => CoverShape::Assignment {
                left: assign.left,
                right: assign.right,
                operator: assign.operator,
            },
            ExpressionKind::ObjectPattern(_)
            | ExpressionKind::ArrayPattern(_)
            | ExpressionKind::AssignmentPattern(_)
            | ExpressionKind::RestElement(_) => CoverShape::AlreadyPattern,
            ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::TSNonNullExpression(_)
            | ExpressionKind::TSTypeAssertion(_)
            | ExpressionKind::CommentBoundPair(_) => CoverShape::Wrapper {
                simple_target: matches!(
                    context,
                    AssignableContext::Assignment | AssignableContext::ForHead
                ) && matches!(
                    expr.skip_type_assertions(view).kind,
                    ExpressionKind::Identifier(_) | ExpressionKind::MemberExpression(_)
                ),
            },
            _ => CoverShape::Other,
        };
        Self {
            span: expr.span(),
            for_head_chain,
            shape,
        }
    }
}

impl<'a> Parser<'a> {
    /// Convert an expression to an assignable pattern (cover grammar)
    ///
    /// This implements the ECMAScript "cover grammar" for assignment targets.
    /// When we parse `{a, b} = obj`, we first parse `{a, b}` as an ObjectExpression,
    /// then convert it to an ObjectPattern when we see the `=`.
    ///
    /// Conversions:
    /// - ObjectExpression → ObjectPattern
    /// - ArrayExpression → ArrayPattern
    /// - SpreadElement → RestElement
    /// - AssignmentExpression with a plain `=` (a default) → AssignmentPattern; a
    ///   compound operator is a syntax error (it names no default, and the pattern
    ///   node has no slot for it), and the WHOLE target of an `=` / for-head may not
    ///   convert to one at all (`Parser::reject_whole_assignment_pattern`)
    /// - Identifier, MemberExpression → unchanged (valid assignment targets)
    ///
    /// Returns `None` when the node is already the target (the identifier, member and
    /// already-a-pattern arms, the accepted wrappers and the deferred non-simple targets),
    /// and the node a conversion built otherwise. The entry points say what the unchanged
    /// case hands back: [`Parser::refine_placed_assignable`] the id it was given,
    /// [`Parser::refine_owned_assignable`] the value it was given.
    ///
    /// `AssignableContext` selects which simple-target wrappers are legal and what a
    /// non-simple target does. `Assignment` and `ForHead` (a no-declaration for-in/of
    /// head) both accept a type-assertion-family expression (`as`, `satisfies`,
    /// non-null `!`, `<T>`) or a JSDoc cast wrapping a *simple* target
    /// (`(x as T) = …`, `for (x! of …)`, `/** @type {T} */ (x) = …`), as the whole
    /// target or as a pattern child; a non-simple target `Assignment` defers and
    /// `ForHead` rejects. `Binding` (function params, destructuring bindings) rejects
    /// every wrapper. The assertion node is kept, and the public AST keeps it too
    /// (acorn preserves it on a simple `=` left and on an `AssignmentPattern` left);
    /// only the internal-only JSDoc cast unwraps at convert.
    ///
    /// It takes the parser mutably because a conversion can owe the depth limit a printer
    /// pair (a default that is itself a bare assignment, [`Parser::printer_pair`]).
    fn refine_assignable(
        &mut self,
        head: CoverHead,
        context: AssignableContext,
    ) -> Result<Option<Expression>, Error> {
        let span = head.span;
        // A parenthesized target is no binding target: `BindingElement` and
        // `BindingRestElement` derive a `BindingIdentifier` or a nested pattern, never a
        // parenthesized one, so `let [(a)] = x` is a syntax error (tsc TS1181, acorn
        // "Parenthesized pattern"), and the paren-free reprint would be a different, valid
        // program. Asked of every node the conversion reaches, ahead of the kind arms, so a
        // grouped identifier, pattern, member or default (`[(a = 1)]`) rejects alike. A
        // default's right side is never converted, so `[a = (1)]` stays an expression's
        // own grouping.
        if matches!(context, AssignableContext::Binding)
            && let Some(open) = self.grouping_paren_around(span)
        {
            return Err(self.error_msg_at(
                "A binding pattern cannot contain a parenthesized target",
                open as usize,
            ));
        }

        // A target on an optional chain (`a?.b`, `a?.b!`, `a?.[i].c`) rejects in a
        // for-head. ecma262 gives an `OptionalExpression` the invalid
        // `AssignmentTargetType`, a static-semantic early error that tsc's parser
        // does not raise (its checker does, TS2780 / TS2781). tsv rejects it here
        // for the reason every non-simple for-head target rejects: a no-declaration
        // for-in/of head is a `LeftHandSideExpression` position but not an
        // assignment context, so the assignment deferral does not reach it (see
        // docs/conformance_svelte.md §TypeScript Corrections, the non-simple
        // assignment target entry). acorn rejects it too ("Optional chaining
        // cannot appear in left-hand side"), and so does prettier. Read through
        // the assertions and the JSDoc cast first, since `a?.b!` is the same chain
        // one node up. Sits ahead of the accepting arms so no simple-target arm
        // admits the chain. An assignment (`a?.b = 1`, and a destructuring element,
        // `[a?.b] = xs`) keeps deferring it with the rest of the non-simple targets
        // below (TS2779 is checker-raised, and an `AssignmentExpression.left`
        // carries any expression). A parenthesized chain is sealed: `(a?.b).c` is
        // an ordinary member target.
        if head.for_head_chain {
            return Err(self.error_msg_at(
                "Optional chaining cannot appear in left-hand side",
                span.start_usize(),
            ));
        }

        match head.shape {
            // Identifier is already a valid assignment target
            CoverShape::Identifier => Ok(None),

            // A member expression is an assignment target but no binding target: the
            // binding grammar names only a `BindingIdentifier` or a nested pattern
            // (tsc TS1005, acorn "Binding member expression").
            CoverShape::Member if matches!(context, AssignableContext::Binding) => Err(self
                .error_msg_at(
                    "A binding pattern cannot contain a member expression",
                    span.start_usize(),
                )),

            // Member expression is a valid assignment target
            CoverShape::Member => Ok(None),

            // Convert ObjectExpression to ObjectPattern
            CoverShape::Object {
                properties: source_properties,
                spread_trailing_comma,
            } => {
                #[cfg(feature = "_internal_ast_census")]
                tsv_lang::ast_census_probe::dead::<ObjectProperty>(
                    tsv_lang::ast_census_probe::Dead::Superseded,
                    source_properties.len(),
                );
                // `{...a,}` — trailing comma after the rest property.
                if spread_trailing_comma {
                    let error_span = self
                        .stores
                        .view()
                        .obj_props(source_properties)
                        .last()
                        .map_or(span, ObjectProperty::span);
                    return Err(self.rest_trailing_comma_error(error_span.start_usize()));
                }

                // Copied out one at a time, an id-only row: each conversion pushes, so no read
                // through the view can span one.
                let property_at = |parser: &Self, i: usize| {
                    parser
                        .stores
                        .view()
                        .obj_props(source_properties)
                        .get(i)
                        .cloned()
                };
                let mark = self.stores.runs.pat_prop_scratch.mark();
                let last_index = source_properties.len().saturating_sub(1);
                let mut i = 0;
                while let Some(prop) = property_at(self, i) {
                    // A rest property must be the last property in an object
                    // destructuring pattern (`ObjectBindingPattern` /
                    // `ObjectAssignmentPattern` place the rest last, with no
                    // trailing comma allowed after it).
                    if let ObjectProperty::SpreadElement(spread) = &prop
                        && i != last_index
                    {
                        return Err(self.error_msg_at(
                            "A rest element must be last in a destructuring pattern",
                            spread.span.start_usize(),
                        ));
                    }
                    let property = self.object_property_to_pattern(prop, context)?;
                    self.stores.runs.pat_prop_scratch.push(property);
                    i += 1;
                }
                let properties = flush_run(
                    &mut self.stores.runs.pat_prop_scratch,
                    &mut self.stores.runs.pat_props,
                    mark,
                );

                Ok(Some(Expression {
                    span,
                    kind: ExpressionKind::ObjectPattern(ObjectPattern {
                        properties,
                        optional: false,
                        type_annotation: None,
                        decorators: None,
                    }),
                }))
            }

            // Convert ArrayExpression to ArrayPattern
            CoverShape::Array {
                elements,
                spread_trailing_comma,
            } => {
                #[cfg(feature = "_internal_ast_census")]
                tsv_lang::ast_census_probe::dead::<Option<ExprId>>(
                    tsv_lang::ast_census_probe::Dead::Superseded,
                    elements.len(),
                );
                let element_at = |parser: &Self, i: usize| {
                    parser
                        .stores
                        .view()
                        .opt_list(elements)
                        .get(i)
                        .copied()
                        .flatten()
                };
                let count = elements.len();
                // `[...a,]` — trailing comma after the rest element.
                // Element-after-rest (`[...a, b]`) and rest-with-default
                // (`[...a = 1]`) are caught in the loop below.
                if spread_trailing_comma {
                    let error_span = count
                        .checked_sub(1)
                        .and_then(|last| element_at(self, last))
                        .map_or(span, |e| self.expr(e).span());
                    return Err(self.rest_trailing_comma_error(error_span.start_usize()));
                }

                let mark = self.stores.expr_opt_scratch.mark();
                let last_index = count.saturating_sub(1);
                for i in 0..count {
                    let converted = match element_at(self, i) {
                        Some(e) => {
                            // A rest element must be the last element in an array
                            // destructuring pattern (`ArrayBindingPattern` /
                            // `ArrayAssignmentPattern` place the rest last). acorn:
                            // "Comma is not permitted after the rest element".
                            if matches!(self.expr(e).kind, ExpressionKind::SpreadElement(_))
                                && i != last_index
                            {
                                return Err(self.error_msg_at(
                                    "A rest element must be last in a destructuring pattern",
                                    self.expr(e).span().start_usize(),
                                ));
                            }
                            Some(self.refine_placed_assignable(e, context)?.id())
                        }
                        None => None,
                    };
                    self.stores.expr_opt_scratch.push(converted);
                }

                Ok(Some(Expression {
                    span,
                    kind: ExpressionKind::ArrayPattern(ArrayPattern {
                        elements: self.flush_opt_expr_list(mark),
                        optional: false,
                        type_annotation: None,
                        decorators: None,
                    }),
                }))
            }

            // Convert SpreadElement to RestElement
            CoverShape::Spread {
                argument,
                span: spread_span,
            } => Ok(Some(Expression::from_rest_element(
                self.spread_to_rest_element(argument, spread_span, context)?,
            ))),

            // AssignmentExpression in pattern context becomes AssignmentPattern
            // This handles default values like `{a = 1}` which was parsed as shorthand.
            // The left is an `=` left in its own right, so a parenthesized cast is a
            // valid target there. That inner-`=` conversion runs
            // at expression-parse time — before the enclosing construct is known — so it
            // applies in a for-head too; ForHead therefore converts the left under
            // Assignment rules. tsc accepts `for ([(a as T) = 1] of x)` and prettier
            // formats it; acorn-typescript rejects it, because preserving the assertion
            // node leaves it in the way of the for-head's second, binding-mode pass —
            // cataloged as `cast_target_destructure_default_for_head_svelte_divergence`.
            // Binding stays Binding: params reject the cast ("unexpected type cast in
            // parameter position").
            //
            // Only a plain `=` covers an `Initializer` (ecma262 §13.15.5,
            // `AssignmentExpression : LeftHandSideExpression = AssignmentExpression` is
            // the one production `AssignmentPattern` / `BindingElement` cover): a
            // compound `[a += b] = xs` names no default, and converting it anyway would
            // DELETE the operator (the pattern node has no slot for it) — the output
            // `[a = b] = xs` is a different program, so this is the faithful-reprint
            // floor, not a deferrable early error. Rejected in every context; acorn
            // spells it the same way.
            CoverShape::Assignment {
                left: assign_left,
                right: assign_right,
                operator,
            } => {
                if operator != AssignmentOperator::Assign {
                    return Err(self.error_msg_at(
                        "Only '=' operator can be used for specifying default value",
                        span.start_usize(),
                    ));
                }
                let left_context = match context {
                    AssignableContext::ForHead => AssignableContext::Assignment,
                    c => c,
                };
                let left = self
                    .refine_placed_assignable(assign_left, left_context)?
                    .id();
                // the pair the printer writes around a default that is itself a bare
                // assignment (`[a = (b = c)] = x`): parsed as an assignment's right side,
                // the one position that takes none, it counted none — a level of the
                // output's nesting (`Parser::self_pair`)
                let right_node = self.expr(assign_right);
                if matches!(right_node.kind, ExpressionKind::AssignmentExpression(_))
                    && self.authored_pairs(
                        self.expr(assign_left).span().end as usize,
                        right_node.span().start as usize,
                    ) == 0
                {
                    self.printer_pair()?;
                }
                Ok(Some(Expression {
                    span,
                    kind: ExpressionKind::AssignmentPattern(AssignmentPattern {
                        left,
                        right: assign_right,
                        decorators: None,
                    }),
                }))
            }

            // Already a pattern (can happen with nested patterns)
            CoverShape::AlreadyPattern => Ok(None),

            // A type-assertion-family expression (`as` / `satisfies` / non-null `!` /
            // `<T>`) or a JSDoc `/** @type {T} */ (expr)` cast is a valid target — as an
            // assignment left and as a no-declaration for-in/of head alike, whole or as
            // a pattern child — when what it wraps, through any further assertions, is
            // a *simple* target (Identifier/MemberExpression), never a destructuring
            // pattern (`([a, b] as T) = …`). That is tsc's line: its
            // `checkReferenceExpression` skips assertions and parens together and asks
            // only that an identifier or an access expression remain, for `=` and for
            // the for-head the same, and its own suite asserts
            // `for ((g satisfies number) of [10])` with a clean baseline; ecma262 makes
            // the for-head's target rule a static-semantic early error, the class the
            // parser defers. acorn-typescript instead converts a for-head in *binding*
            // mode, where every assertion kind raises "Unexpected type cast in
            // parameter position", and reads the JSDoc parens as grouping, rejecting a
            // nested cast under them — shape-oracle verdicts tsv does not follow,
            // cataloged as `cast_target_for_head_svelte_divergence`. A binding position
            // still rejects every wrapper (params, declarations; even bare parens are
            // illegal there). The node is kept (the formatter reproduces prettier's
            // `(x as T) = …` and its paren-free `for (x as T of …)`, and preserves the
            // JSDoc cast's parens); convert emits the assertion and unwraps the cast.
            // Whether a *nested* assertion (a bare pattern child) was reached through
            // grouping parens does not matter: acorn reads those parens as grouping, so
            // `({ a: b as T } = x)` and `({ a: (b as T) } = x)` are the same target,
            // both accepted — pinned by `cast_target_destructure` and
            // `cast_target_destructure_paren`.
            CoverShape::Wrapper {
                simple_target: true,
            } => Ok(None),

            // A non-simple target in *assignment* context (a call `foo() = …`, a
            // literal `1 >>= …`, `this = …`, `new C() = …`, …) is not a valid
            // `LeftHandSideExpression`, but "not a valid assignment target" is a
            // static-semantic early-error, not a syntax error — the assignment
            // grammar parses it and the assignability refinement is layered on top.
            // Per the permissive stance (`crates/tsv_ts/CLAUDE.md` §Sources of truth)
            // the parser defers it: the target is kept as-is so the formatter keeps
            // formatting well-formed input (prettier formats all of these). Only an
            // assignment defers, plain or compound — `Binding` (params/destructuring
            // bindings) and `ForHead` (a no-declaration for-in/of head) still reject
            // below, which is why `for (foo() in b)` stays a parse error (prettier
            // rejects it too).
            // TODO: the invalid-target early-error belongs in the diagnostics layer.
            CoverShape::Wrapper { .. } | CoverShape::Other
                if matches!(context, AssignableContext::Assignment) =>
            {
                Ok(None)
            }

            // Invalid assignment target (binding / for-head position)
            CoverShape::Wrapper { .. } | CoverShape::Other => {
                Err(self.error_msg_at("Invalid assignment target", span.start_usize()))
            }
        }
    }

    /// [`Parser::refine_assignable`] over a stored node. An unchanged target is the id it
    /// was given, with no read and no push; only a conversion pushes a row, superseding the
    /// one it read.
    pub(super) fn refine_placed_assignable(
        &mut self,
        expr: ExprId,
        context: AssignableContext,
    ) -> Result<ExprRet, Error> {
        // the two targets nearly every call holds, answered without entering the body,
        // whose arms for them this restates
        let node = self.expr(expr);
        let unchanged = match node.kind {
            ExpressionKind::Identifier(_) => {
                !matches!(context, AssignableContext::Binding)
                    || self.grouping_paren_around(node.span()).is_none()
            }
            ExpressionKind::MemberExpression(_) => matches!(
                context,
                AssignableContext::Assignment | AssignableContext::SveltePattern
            ),
            _ => false,
        };
        if unchanged {
            #[expect(
                clippy::debug_assert_with_mut_call,
                reason = "the arms this restates mutate nothing"
            )]
            {
                let head = CoverHead::of(self.stores.view(), self.expr(expr), context);
                debug_assert!(matches!(self.refine_assignable(head, context), Ok(None)));
            }
            return Ok(ExprRet::wrap(expr));
        }
        let head = CoverHead::of(self.stores.view(), self.expr(expr), context);
        match self.refine_assignable(head, context)? {
            None => Ok(ExprRet::wrap(expr)),
            Some(converted) => {
                #[cfg(feature = "_internal_ast_census")]
                tsv_lang::ast_census_probe::dead::<Expression>(
                    tsv_lang::ast_census_probe::Dead::Superseded,
                    1,
                );
                Ok(self.push_ret(converted))
            }
        }
    }

    /// [`Parser::refine_assignable`] over a value the parse has not placed: a binding pattern
    /// read from a literal.
    pub(super) fn refine_owned_assignable(
        &mut self,
        expr: Expression,
        context: AssignableContext,
    ) -> Result<Expression, Error> {
        let head = CoverHead::of(self.stores.view(), &expr, context);
        Ok(self.refine_assignable(head, context)?.unwrap_or(expr))
    }

    /// Convert the WHOLE target of an `=` (cover grammar), which may not be an
    /// `AssignmentPattern` ([`Parser::reject_whole_assignment_pattern`]).
    pub(super) fn refine_assignment_target(&mut self, expr: ExprId) -> Result<ExprRet, Error> {
        let target = self
            .refine_placed_assignable(expr, AssignableContext::Assignment)?
            .id();
        self.reject_whole_assignment_pattern(target)?;
        Ok(ExprRet::wrap(target))
    }

    /// Convert the WHOLE target of a no-declaration for-in/of head, which may not be an
    /// `AssignmentPattern` ([`Parser::reject_whole_assignment_pattern`]).
    pub(super) fn refine_for_head_target(&mut self, expr: ExprId) -> Result<ExprRet, Error> {
        let target = self
            .refine_placed_assignable(expr, AssignableContext::ForHead)?
            .id();
        self.reject_whole_assignment_pattern(target)?;
        Ok(ExprRet::wrap(target))
    }

    /// Reject a converted WHOLE target of an `=` or of a no-declaration for-in/of head that
    /// is an `AssignmentPattern`.
    ///
    /// An `AssignmentPattern` exists only as a pattern CHILD (an array element, a
    /// property value, a parameter): `AssignmentExpression.left` and the for-head's
    /// `left` derive `LeftHandSideExpression` / `AssignmentPattern`-proper, and neither
    /// covers an `Initializer` of its own. So a parenthesized assignment there —
    /// `(a = b) = 1`, `for ((a = b) of xs)` — has no wire shape: conversion turns the
    /// parenthesized `=` into an `AssignmentPattern`, which acorn never emits at either
    /// position (it rejects, "Assigning to rvalue"), and no assignment is left in the
    /// tree for a kept pair to wrap. The representability floor says reject, ahead of
    /// the non-simple-target deferral. A parenthesized conditional, arrow or `yield` is
    /// never converted, so it survives as itself and defers with the rest. Only the
    /// parenthesized spelling reaches this: an unparenthesized `a = b = 1` parses
    /// right-associative, so its left is never an assignment.
    fn reject_whole_assignment_pattern(&self, target: ExprId) -> Result<(), Error> {
        let target = self.expr(target);
        if matches!(target.kind, ExpressionKind::AssignmentPattern(_)) {
            return Err(self.error_msg_at("Invalid assignment target", target.span().start_usize()));
        }
        Ok(())
    }

    /// Reject an assignment left that no `LeftHandSideExpression` derives — the
    /// GRAMMAR error, as opposed to the deferred `AssignmentTargetType` early error
    /// `refine_assignable` lets through.
    ///
    /// `AssignmentExpression : LeftHandSideExpression AssignmentOperator
    /// AssignmentExpression` (ecma262 §13.15, and the same left for `=` and the logical
    /// assignments) takes a `LeftHandSideExpression`, which an unparenthesized operator
    /// expression is not: a unary (`-a`, `typeof a`), an update (`++a`, `a++`), an
    /// `await`, a binary or a logical expression. tsc's parser rejects every one
    /// (TS1005), and so do acorn (`Assigning to rvalue`) and prettier. Parenthesized, the
    /// same operand is a `ParenthesizedExpression` — a `PrimaryExpression` — and only the
    /// early error refuses it, so `(-a) = 1` defers like `foo() = bar` (tsc's parser
    /// accepts it) and the printer keeps the pair (`ParenContext::AssignmentTarget`).
    ///
    /// A bare type assertion (`a as T = 1`, `a satisfies T += 1`, `<T>a = 1`) is no
    /// `LeftHandSideExpression` either, over any operand: tsc's parser rejects every one
    /// (TS1005), and so does prettier. acorn-typescript alone accepts one over a simple
    /// operand (an identifier or a member, through further assertions), but the
    /// production derives none, and the only reprint every parser reads — the
    /// parenthesized `(a as T) = 1` — would be a repair into a different, valid program,
    /// not the input. Parenthesized, the assertion is the deferred class like any other
    /// operand (`(a as T) = 1`). A destructuring element's own target (`[a as T] = c`)
    /// and a for-in/of head never reach here; an element default's does
    /// (`[a as T = 1] = c`), since the element is an assignment before it is a pattern.
    ///
    /// "Unparenthesized" is read off the node's extent against the expression's: a
    /// grouped operand starts after the `(` the parse began at (`expr_start`) and ends
    /// before the `)` last consumed (`prev_token_end`), whatever comments or line breaks
    /// sit inside the pair. A node whose operator came first (`-(a)`) or last (`(a)++`),
    /// or that a binary built from the pair outward (`(a) + b`), fails one side.
    pub(super) fn check_bare_assignment_left(
        &self,
        left: ExprId,
        expr_start: usize,
    ) -> Result<(), Error> {
        let left = self.expr(left);
        let span = left.span();
        let parenthesized =
            span.start_usize() > expr_start && (span.end as usize) < self.prev_token_end();
        if parenthesized {
            return Ok(());
        }
        let refused = match &left.kind {
            ExpressionKind::UnaryExpression(_)
            | ExpressionKind::UpdateExpression(_)
            | ExpressionKind::AwaitExpression(_)
            // a logical expression too: the internal AST folds `&&` / `||` / `??` into
            // `BinaryExpression`
            | ExpressionKind::BinaryExpression(_)
            | ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::TSTypeAssertion(_) => true,
            _ => false,
        };
        if refused {
            return Err(
                self.error_msg_at("Invalid left-hand side in assignment", span.start_usize())
            );
        }
        Ok(())
    }

    /// The `(` of the grouping paren pair wrapping exactly `span`, if one was recorded
    /// while the enclosing binding pattern was read (the outermost pair, for `((a))`).
    fn grouping_paren_around(&self, span: Span) -> Option<u32> {
        self.grouping_parens
            .iter()
            .rev()
            .find(|paren| paren.inner == span)
            .map(|paren| paren.open)
    }

    /// Build the "trailing comma after a rest element" syntax error (`[...a,]` /
    /// `{...a,}`). The literal parser records the discarded comma on
    /// `spread_trailing_comma`; both the array and object pattern arms surface
    /// it. acorn: "Comma is not permitted after the rest element".
    fn rest_trailing_comma_error(&self, pos: usize) -> Error {
        self.error_msg_at(
            "A trailing comma is not permitted after a rest element",
            pos,
        )
    }

    /// Convert a `...expr` spread into a `RestElement` — the shared core of both
    /// pattern arms (array/assignment-target spreads in `refine_assignable`, object
    /// rest properties in `object_property_to_pattern`). A rest element binds its
    /// target directly: the grammar's `BindingRestElement` / `AssignmentRestElement`
    /// / `BindingRestProperty` / `AssignmentRestProperty` carry no `Initializer`, so
    /// a default (`[...a = 1]`, `{...a = 1}`) is a syntax error.
    fn spread_to_rest_element(
        &mut self,
        argument: ExprId,
        spread_span: Span,
        context: AssignableContext,
    ) -> Result<RestElement, Error> {
        let argument = self.refine_placed_assignable(argument, context)?.id();
        if matches!(
            self.expr(argument).kind,
            ExpressionKind::AssignmentPattern(_)
        ) {
            return Err(self.error_msg_at(
                "A rest element cannot have a default value",
                spread_span.start_usize(),
            ));
        }
        Ok(RestElement {
            argument,
            optional: false,
            type_annotation: None,
            span: spread_span,
        })
    }

    /// Convert an object property to a pattern property
    fn object_property_to_pattern(
        &mut self,
        prop: ObjectProperty,
        context: AssignableContext,
    ) -> Result<ObjectPatternProperty, Error> {
        match prop {
            ObjectProperty::Property(p) => {
                // A method, accessor or generator property has no pattern form: the wire
                // has no `ObjectPattern` property carrying one, and converting the value
                // reprints a different program (`({ get f() {} } = x)` as
                // `({ f: function () {} } = x)`). The representability floor says reject,
                // as acorn does and as tsc's grammar check does (TS1136), with the message
                // and position a binding or for-head target already gives the method's
                // function value through the non-simple arm.
                if p.method || p.kind != PropertyKind::Init {
                    return Err(self.error_msg_at(
                        "Invalid assignment target",
                        self.expr(p.value).span().start_usize(),
                    ));
                }
                // Convert the value to a pattern
                let value = self.refine_placed_assignable(p.value, context)?.id();

                Ok(ObjectPatternProperty::Property(Property {
                    key: p.key,
                    value,
                    method: p.method,
                    shorthand: p.shorthand,
                    computed: p.computed,
                    kind: p.kind,
                    span: p.span,
                }))
            }
            ObjectProperty::SpreadElement(spread) => Ok(ObjectPatternProperty::RestElement(
                self.spread_to_rest_element(spread.argument, spread.span, context)?,
            )),
        }
    }
}
