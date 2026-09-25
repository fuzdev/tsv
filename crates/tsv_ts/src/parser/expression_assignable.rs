// Cover-grammar conversion: turn an already-parsed `Expression` into an
// assignable pattern (`{a, b} = obj`, `[x] = arr`, arrow params). Pure AST
// rewriting — no token consumption.

use crate::ast::internal::{
    ArrayPattern, AssignmentOperator, AssignmentPattern, Expression, ExpressionKind, ObjectPattern,
    ObjectPatternProperty, ObjectProperty, Property, RestElement, SpreadElement,
};
use tsv_lang::{ParseError, Span};

use super::Parser;

/// Which assignable position `to_assignable` is converting for — the axis that
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
    /// (`read_pattern` parses `(${pattern} = 1)`; `{@const}` parses `id = init`), so a
    /// member expression and a parenthesized target are the
    /// `DestructuringAssignmentTarget`s it admits, and tsv accepts both to match its
    /// AST. Every other target is graded as `Binding` grades it.
    SveltePattern,
}

impl<'a, 'arena> Parser<'a, 'arena> {
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
    ///   convert to one at all (`to_whole_assignable`)
    /// - Identifier, MemberExpression → unchanged (valid assignment targets)
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
    pub(super) fn to_assignable(
        &self,
        expr: Expression<'arena>,
        context: AssignableContext,
    ) -> Result<Expression<'arena>, ParseError> {
        // A parenthesized target is no binding target: `BindingElement` and
        // `BindingRestElement` derive a `BindingIdentifier` or a nested pattern, never a
        // parenthesized one, so `let [(a)] = x` is a syntax error (tsc TS1181, acorn
        // "Parenthesized pattern"), and the paren-free reprint would be a different, valid
        // program. Asked of every node the conversion reaches, ahead of the kind arms, so a
        // grouped identifier, pattern, member or default (`[(a = 1)]`) rejects alike. A
        // default's right side is never converted, so `[a = (1)]` stays an expression's
        // own grouping.
        if matches!(context, AssignableContext::Binding)
            && let Some(open) = self.grouping_paren_around(expr.span())
        {
            return Err(self.error_msg_at(
                "A binding pattern cannot contain a parenthesized target",
                open as usize,
            ));
        }

        match &expr.kind {
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
            _ if matches!(context, AssignableContext::ForHead)
                && expr.skip_type_assertions().has_optional_in_chain() =>
            {
                Err(self.error_msg_at(
                    "Optional chaining cannot appear in left-hand side",
                    expr.span().start_usize(),
                ))
            }

            // Identifier is already a valid assignment target
            ExpressionKind::Identifier(_) => Ok(expr),

            // A member expression is an assignment target but no binding target: the
            // binding grammar names only a `BindingIdentifier` or a nested pattern
            // (tsc TS1005, acorn "Binding member expression").
            ExpressionKind::MemberExpression(_)
                if matches!(context, AssignableContext::Binding) =>
            {
                Err(self.error_msg_at(
                    "A binding pattern cannot contain a member expression",
                    expr.span().start_usize(),
                ))
            }

            // Member expression is a valid assignment target
            ExpressionKind::MemberExpression(_) => Ok(expr),

            // Convert ObjectExpression to ObjectPattern
            ExpressionKind::ObjectExpression(obj) => {
                // `{...a,}` — trailing comma after the rest property.
                if obj.spread_trailing_comma {
                    let span = obj
                        .properties
                        .last()
                        .map_or(expr.span, ObjectProperty::span);
                    return Err(self.rest_trailing_comma_error(span.start_usize()));
                }

                let mut properties = self.bvec();
                let last_index = obj.properties.len().saturating_sub(1);
                for (i, prop) in obj.properties.iter().enumerate() {
                    // A rest property must be the last property in an object
                    // destructuring pattern (`ObjectBindingPattern` /
                    // `ObjectAssignmentPattern` place the rest last, with no
                    // trailing comma allowed after it).
                    if let ObjectProperty::SpreadElement(spread) = prop
                        && i != last_index
                    {
                        return Err(self.error_msg_at(
                            "A rest element must be last in a destructuring pattern",
                            spread.span.start_usize(),
                        ));
                    }
                    properties.push(self.object_property_to_pattern(prop.clone(), context)?);
                }

                Ok(Expression {
                    span: expr.span,
                    kind: ExpressionKind::ObjectPattern(ObjectPattern {
                        properties: properties.into_bump_slice(),
                        optional: false,
                        type_annotation: None,
                        decorators: None,
                    }),
                })
            }

            // Convert ArrayExpression to ArrayPattern
            ExpressionKind::ArrayExpression(arr) => {
                // `[...a,]` — trailing comma after the rest element.
                // Element-after-rest (`[...a, b]`) and rest-with-default
                // (`[...a = 1]`) are caught in the loop below.
                if arr.spread_trailing_comma {
                    let span = arr
                        .elements
                        .last()
                        .and_then(|e| *e)
                        .map_or(expr.span, Expression::span);
                    return Err(self.rest_trailing_comma_error(span.start_usize()));
                }

                let mut elements = self.bvec();
                let last_index = arr.elements.len().saturating_sub(1);
                for (i, elem) in arr.elements.iter().enumerate() {
                    let converted = match elem {
                        Some(e) => {
                            // A rest element must be the last element in an array
                            // destructuring pattern (`ArrayBindingPattern` /
                            // `ArrayAssignmentPattern` place the rest last). acorn:
                            // "Comma is not permitted after the rest element".
                            if matches!(e.kind, ExpressionKind::SpreadElement(_)) && i != last_index
                            {
                                return Err(self.error_msg_at(
                                    "A rest element must be last in a destructuring pattern",
                                    e.span().start_usize(),
                                ));
                            }
                            Some(self.to_assignable((*e).clone(), context)?)
                        }
                        None => None,
                    };
                    elements.push(converted);
                }

                Ok(Expression {
                    span: expr.span,
                    kind: ExpressionKind::ArrayPattern(ArrayPattern {
                        elements: elements.into_bump_slice(),
                        optional: false,
                        type_annotation: None,
                        decorators: None,
                    }),
                })
            }

            // Convert SpreadElement to RestElement
            ExpressionKind::SpreadElement(spread) => Ok(Expression::from_rest_element(
                self.spread_to_rest_element(spread, context)?,
            )),

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
            ExpressionKind::AssignmentExpression(assign) => {
                if assign.operator != AssignmentOperator::Assign {
                    return Err(self.error_msg_at(
                        "Only '=' operator can be used for specifying default value",
                        expr.span.start_usize(),
                    ));
                }
                let left_context = match context {
                    AssignableContext::ForHead => AssignableContext::Assignment,
                    c => c,
                };
                let left = self.to_assignable(assign.left.clone(), left_context)?;
                Ok(Expression {
                    span: expr.span,
                    kind: ExpressionKind::AssignmentPattern(AssignmentPattern {
                        left: self.alloc(left),
                        right: assign.right,
                        decorators: None,
                    }),
                })
            }

            // Already a pattern (can happen with nested patterns)
            ExpressionKind::ObjectPattern(_)
            | ExpressionKind::ArrayPattern(_)
            | ExpressionKind::AssignmentPattern(_)
            | ExpressionKind::RestElement(_) => Ok(expr),

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
            ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::TSNonNullExpression(_)
            | ExpressionKind::TSTypeAssertion(_)
            | ExpressionKind::JsdocCast(_)
                if matches!(
                    context,
                    AssignableContext::Assignment | AssignableContext::ForHead
                ) && matches!(
                    expr.skip_type_assertions().kind,
                    ExpressionKind::Identifier(_) | ExpressionKind::MemberExpression(_)
                ) =>
            {
                Ok(expr)
            }

            // A non-simple target in *assignment* context (a call `foo() = …`, a
            // literal `1 >>= …`, `this = …`, `new C() = …`, …) is not a valid
            // `LeftHandSideExpression`, but "not a valid assignment target" is a
            // static-semantic early-error, not a syntax error — the assignment
            // grammar parses it and the assignability refinement is layered on top.
            // Per the permissive stance (`crates/tsv_ts/CLAUDE.md` §Sources of truth)
            // the parser defers it: the target is kept as-is so the formatter keeps
            // formatting well-formed input (prettier formats all of these). Only
            // `Assignment` defers — `Binding` (params/destructuring bindings) and
            // `ForHead` (a no-declaration for-in/of head) still reject below, which
            // is why `for (foo() in b)` stays a parse error (prettier rejects it too).
            // TODO: the invalid-target early-error belongs in the diagnostics layer.
            _ if matches!(context, AssignableContext::Assignment) => Ok(expr),

            // Invalid assignment target (binding / for-head position)
            _ => Err(self.error_msg_at("Invalid assignment target", expr.span().start_usize())),
        }
    }

    /// Convert the WHOLE target of an `=` or of a no-declaration for-in/of head —
    /// the one place a converted target may not be an `AssignmentPattern`.
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
    pub(super) fn to_whole_assignable(
        &self,
        expr: Expression<'arena>,
        context: AssignableContext,
    ) -> Result<Expression<'arena>, ParseError> {
        let start = expr.span().start_usize();
        let target = self.to_assignable(expr, context)?;
        if matches!(target.kind, ExpressionKind::AssignmentPattern(_)) {
            return Err(self.error_msg_at("Invalid assignment target", start));
        }
        Ok(target)
    }

    /// Reject an assignment left that no `LeftHandSideExpression` derives — the
    /// GRAMMAR error, as opposed to the deferred `AssignmentTargetType` early error
    /// `to_assignable` lets through.
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
    /// A bare type assertion (`a as T = 1`, `<T>a = 1`) is no `LeftHandSideExpression`
    /// either, and tsc's parser refuses it; tsv follows acorn-typescript, which accepts
    /// one over a SIMPLE operand (an identifier or a member, through further assertions)
    /// — the printer repairs it to `(a as T) = 1`, the spelling every parser reads alike.
    /// Over any other operand (`-a as T = 1`, `fn() as T = 1`, `[a, b] as T = c`) both
    /// oracles reject, and so does this.
    ///
    /// "Unparenthesized" is read off the node's extent against the expression's: a
    /// grouped operand starts after the `(` the parse began at (`expr_start`) and ends
    /// before the `)` last consumed (`prev_token_end`), whatever comments or line breaks
    /// sit inside the pair. A node whose operator came first (`-(a)`) or last (`(a)++`),
    /// or that a binary built from the pair outward (`(a) + b`), fails one side.
    pub(super) fn check_bare_assignment_left(
        &self,
        left: &Expression<'arena>,
        expr_start: usize,
    ) -> Result<(), ParseError> {
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
            | ExpressionKind::BinaryExpression(_) => true,
            ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::TSTypeAssertion(_) => !matches!(
                left.skip_type_assertions().kind,
                ExpressionKind::Identifier(_) | ExpressionKind::MemberExpression(_)
            ),
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
    fn rest_trailing_comma_error(&self, pos: usize) -> ParseError {
        self.error_msg_at(
            "A trailing comma is not permitted after a rest element",
            pos,
        )
    }

    /// Convert a `...expr` spread into a `RestElement` — the shared core of both
    /// pattern arms (array/assignment-target spreads in `to_assignable`, object
    /// rest properties in `object_property_to_pattern`). A rest element binds its
    /// target directly: the grammar's `BindingRestElement` / `AssignmentRestElement`
    /// / `BindingRestProperty` / `AssignmentRestProperty` carry no `Initializer`, so
    /// a default (`[...a = 1]`, `{...a = 1}`) is a syntax error.
    fn spread_to_rest_element(
        &self,
        spread: &SpreadElement<'arena>,
        context: AssignableContext,
    ) -> Result<RestElement<'arena>, ParseError> {
        let argument = self.to_assignable(spread.argument.clone(), context)?;
        if matches!(argument.kind, ExpressionKind::AssignmentPattern(_)) {
            return Err(self.error_msg_at(
                "A rest element cannot have a default value",
                spread.span.start_usize(),
            ));
        }
        Ok(RestElement {
            argument: self.alloc(argument),
            optional: false,
            type_annotation: None,
            span: spread.span,
        })
    }

    /// Convert an object property to a pattern property
    fn object_property_to_pattern(
        &self,
        prop: ObjectProperty<'arena>,
        context: AssignableContext,
    ) -> Result<ObjectPatternProperty<'arena>, ParseError> {
        match prop {
            ObjectProperty::Property(p) => {
                // Convert the value to a pattern
                let value = self.to_assignable(p.value.clone(), context)?;

                Ok(ObjectPatternProperty::Property(Property {
                    key: p.key,
                    value: self.alloc(value),
                    method: p.method,
                    shorthand: p.shorthand,
                    computed: p.computed,
                    kind: p.kind,
                    span: p.span,
                }))
            }
            ObjectProperty::SpreadElement(spread) => Ok(ObjectPatternProperty::RestElement(
                self.spread_to_rest_element(&spread, context)?,
            )),
        }
    }
}
