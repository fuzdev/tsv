// Cover-grammar conversion: turn an already-parsed `Expression` into an
// assignable pattern (`{a, b} = obj`, `[x] = arr`, arrow params). Pure AST
// rewriting — no token consumption.

use crate::ast::internal::{
    ArrayPattern, AssignmentPattern, Expression, ObjectPattern, ObjectPatternProperty,
    ObjectProperty, Property, RestElement, SpreadElement,
};
use tsv_lang::ParseError;

use super::Parser;

/// Which assignable position `to_assignable` is converting for — the axis that
/// decides whether a type-assertion or JSDoc-cast target is allowed, and what a
/// non-simple target does. `Assignment` and `ForHead` (a no-declaration for-in/of
/// head) both accept an assertion or a JSDoc cast over a simple target
/// (`(x as T) = …`, `for (x! of …)`, `/** @type {T} */ (x) = …`) — tsc's
/// `checkReferenceExpression` reads the two positions with one rule — and differ
/// only on a non-simple target, which `Assignment` defers and `ForHead` rejects;
/// `Binding` (function params, destructuring bindings, Svelte `{:then}`/`{:catch}`)
/// rejects every wrapper. acorn-typescript's `isBinding` split puts the for-head on
/// the binding side; tsv does not follow it there.
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
    /// Binding position — a type-assertion *or* a (parenthesized) JSDoc-cast target
    /// is rejected (even bare parens are illegal: `function f((x))`).
    Binding,
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
    /// - BinaryExpression with = (shorthand default) → AssignmentPattern
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
        match expr {
            // A target on an optional chain (`a?.b`, `a?.b!`, `a?.[i].c`) is no target
            // at all in a for-head: ecma262 gives an `OptionalExpression` the invalid
            // `AssignmentTargetType`, and there is no wire shape for it — acorn wraps
            // the chain in a `ChainExpression`, which is not a `Pattern`, and rejects
            // ("Optional chaining cannot appear in left-hand side"); tsc's checker
            // rejects it too (TS2780 / TS2781, the `OptionalChain` flag on the
            // reference it skipped to), and so does prettier. Read through the assertions and the
            // JSDoc cast first, since `a?.b!` is the same chain one node up. Sits
            // ahead of the accepting arms so no simple-target arm admits the chain. An
            // assignment (`a?.b = 1`) keeps deferring it with the rest of the
            // non-simple targets below (TS2779 is checker-raised, and an
            // `AssignmentExpression.left` carries any expression). A parenthesized
            // chain is sealed: `(a?.b).c` is an ordinary member target.
            _ if matches!(context, AssignableContext::ForHead)
                && expr.skip_type_assertions().has_optional_in_chain() =>
            {
                Err(self.error_msg_at(
                    "Optional chaining cannot appear in left-hand side",
                    expr.span().start_usize(),
                ))
            }

            // Identifier is already a valid assignment target
            Expression::Identifier(_) => Ok(expr),

            // Member expression is a valid assignment target
            Expression::MemberExpression(_) => Ok(expr),

            // Convert ObjectExpression to ObjectPattern
            Expression::ObjectExpression(obj) => {
                // `{...a,}` — trailing comma after the rest property.
                if obj.spread_trailing_comma {
                    let span = obj.properties.last().map_or(obj.span, ObjectProperty::span);
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

                Ok(Expression::ObjectPattern(ObjectPattern {
                    properties: properties.into_bump_slice(),
                    optional: false,
                    type_annotation: None,
                    decorators: None,
                    span: obj.span,
                }))
            }

            // Convert ArrayExpression to ArrayPattern
            Expression::ArrayExpression(arr) => {
                // `[...a,]` — trailing comma after the rest element.
                // Element-after-rest (`[...a, b]`) and rest-with-default
                // (`[...a = 1]`) are caught in the loop below.
                if arr.spread_trailing_comma {
                    let span = arr
                        .elements
                        .last()
                        .and_then(|e| e.as_ref())
                        .map_or(arr.span, Expression::span);
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
                            if matches!(e, Expression::SpreadElement(_)) && i != last_index {
                                return Err(self.error_msg_at(
                                    "A rest element must be last in a destructuring pattern",
                                    e.span().start_usize(),
                                ));
                            }
                            Some(self.to_assignable(e.clone(), context)?)
                        }
                        None => None,
                    };
                    elements.push(converted);
                }

                Ok(Expression::ArrayPattern(ArrayPattern {
                    elements: elements.into_bump_slice(),
                    optional: false,
                    type_annotation: None,
                    decorators: None,
                    span: arr.span,
                }))
            }

            // Convert SpreadElement to RestElement
            Expression::SpreadElement(spread) => Ok(Expression::RestElement(
                self.spread_to_rest_element(&spread, context)?,
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
            Expression::AssignmentExpression(assign) => {
                let left_context = match context {
                    AssignableContext::ForHead => AssignableContext::Assignment,
                    c => c,
                };
                let left = self.to_assignable(assign.left.clone(), left_context)?;
                Ok(Expression::AssignmentPattern(AssignmentPattern {
                    left: self.alloc(left),
                    right: assign.right,
                    decorators: None,
                    span: assign.span,
                }))
            }

            // Already a pattern (can happen with nested patterns)
            Expression::ObjectPattern(_)
            | Expression::ArrayPattern(_)
            | Expression::AssignmentPattern(_)
            | Expression::RestElement(_) => Ok(expr),

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
            Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSNonNullExpression(_)
            | Expression::TSTypeAssertion(_)
            | Expression::JsdocCast(_)
                if matches!(
                    context,
                    AssignableContext::Assignment | AssignableContext::ForHead
                ) && matches!(
                    expr.skip_type_assertions(),
                    Expression::Identifier(_) | Expression::MemberExpression(_)
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
        if matches!(argument, Expression::AssignmentPattern(_)) {
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
