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
/// decides whether a type-assertion or JSDoc-cast target is allowed. `Assignment`
/// accepts both (`(x as T) = …`, `/** @type {T} */ (x) = …`); `ForHead` (a
/// no-declaration for-in/of head) rejects type assertions but accepts a JSDoc cast
/// over a simple target; `Binding` (function params, destructuring bindings, Svelte
/// `{:then}`/`{:catch}`) rejects both. Mirrors acorn-typescript's `isBinding` split,
/// with the for-head carved out as its own case.
#[derive(Clone, Copy)]
pub(in crate::parser) enum AssignableContext {
    /// `… = rhs` — a type-assertion wrapping a *simple* target is itself a valid target.
    Assignment,
    /// A no-declaration for-in/of head (`for ((x) of …)`) — an
    /// `AssignmentTargetType`/`LeftHandSideExpression` position. A type-assertion
    /// target is rejected (acorn: `for ((x as T) of …)` is a syntax error), but a
    /// JSDoc `/** @type {T} */ (x)` cast is *transparent grouping* (no node in
    /// acorn's AST), so it is accepted when it wraps a bare Identifier/MemberExpression.
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
    /// `AssignableContext` selects which simple-target wrappers are legal.
    /// `Assignment` accepts a type-assertion-family expression (`as`, `satisfies`,
    /// non-null `!`, `<T>`) or a JSDoc cast wrapping a *simple* target
    /// (`(x as T) = …`, `/** @type {T} */ (x) = …`); `ForHead` (a no-declaration
    /// for-in/of head) rejects type assertions but accepts a JSDoc cast over a simple
    /// target; `Binding` (function params, destructuring bindings) rejects both —
    /// matching acorn-typescript's `isBinding` split. The assertion/cast node is kept
    /// here; the public AST unwraps it at the convert boundary (acorn drops the
    /// cast/assertion from a simple `=` left and from an `AssignmentPattern` left).
    pub(super) fn to_assignable(
        &self,
        expr: Expression<'arena>,
        context: AssignableContext,
    ) -> Result<Expression<'arena>, ParseError> {
        self.to_assignable_impl(expr, context, false)
    }

    /// The recursive core of `to_assignable`. `nested` is true when converting a
    /// pattern *child* (a property value, array element, or rest argument) rather
    /// than the whole assignment left. It gates the parenthesized-cast rejection:
    /// acorn accepts a grouping-parenthesized cast as the whole `=` left or as an
    /// `AssignmentPattern` left (`({ a: (b as T) = 1 } = x)` — the inner-`=`
    /// conversion unwraps it), but rejects it as a bare nested target
    /// (`({ a: (b as T) } = x)` → "Assigning to rvalue"), while a bare
    /// *unparenthesized* nested cast is kept (`({ a: b as T } = x)`).
    fn to_assignable_impl(
        &self,
        expr: Expression<'arena>,
        context: AssignableContext,
        nested: bool,
    ) -> Result<Expression<'arena>, ParseError> {
        match expr {
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
                            Some(self.to_assignable_impl(e.clone(), context, true)?)
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
            // The left converts as non-nested: it is an `=` left in its own right, so a
            // parenthesized cast is a valid target there (acorn's inner-`=` conversion
            // unwraps it; the convert layer mirrors that unwrap at emission). That
            // inner-`=` conversion runs at expression-parse time in acorn — before the
            // enclosing construct is known — so it applies in a for-head too
            // (`for ([(a as T) = 1] of x)` is accepted); ForHead therefore converts the
            // left under Assignment rules. Binding stays Binding: params reject the
            // cast ("unexpected type cast in parameter position").
            Expression::AssignmentExpression(assign) => {
                let left_context = match context {
                    AssignableContext::ForHead => AssignableContext::Assignment,
                    c => c,
                };
                let left = self.to_assignable_impl(assign.left.clone(), left_context, false)?;
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

            // A JSDoc `/** @type {T} */ (expr)` cast is *transparent grouping* — acorn
            // carries no node for it — so it is a valid target wherever a parenthesized
            // simple target is: an assignment (`/** @type {T} */ (x.y) += …`) and a
            // no-declaration for-in/of head (`for (/** @type {T} */ (x) of …)`). True
            // binding positions reject it (even bare parens are illegal there). The
            // valid inner is Identifier/MemberExpression — but only an *assignment* may
            // wrap an assertion inside the cast (`/** @type {T} */ (x as U) = …`); a
            // for-head must wrap a bare Identifier/MemberExpression (assertions are
            // illegal there). The node is kept (the formatter preserves the parens);
            // convert unwraps it unconditionally.
            // Nested (a bare pattern child), only a plain inner target is legal —
            // acorn sees the JSDoc parens as ordinary grouping, which it accepts
            // around a simple target but rejects around a cast.
            Expression::JsdocCast(ref cast)
                if matches!(
                    context,
                    AssignableContext::Assignment | AssignableContext::ForHead
                ) && matches!(
                    match (context, nested) {
                        (AssignableContext::Assignment, false) => cast.inner.skip_type_assertions(),
                        _ => cast.inner,
                    },
                    Expression::Identifier(_) | Expression::MemberExpression(_)
                ) =>
            {
                Ok(expr)
            }

            // A type-assertion-family expression (`as` / `satisfies` / non-null `!` /
            // `<T>`) is a valid assignment target — in assignment context only — when it
            // wraps a *simple* target (Identifier/MemberExpression): acorn accepts
            // `(x as T) = …` / `(x.y! as U) = …` but rejects an assertion wrapping a
            // destructuring pattern (`([a, b] as T) = …`), and rejects it in a for-head
            // (`for ((x as T) of …)`) / binding position (acorn's `isBinding` split).
            // Nested (a bare pattern child), the cast must additionally be
            // *unparenthesized* — `({ a: b as T } = x)` is kept, but
            // `({ a: (b as T) } = x)` is acorn's "Assigning to rvalue". The node is
            // kept (the formatter reproduces prettier's `(x as T) = …`); convert
            // unwraps it for a simple `=` left.
            Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSNonNullExpression(_)
            | Expression::TSTypeAssertion(_)
                if matches!(context, AssignableContext::Assignment)
                    && matches!(
                        expr.skip_type_assertions(),
                        Expression::Identifier(_) | Expression::MemberExpression(_)
                    )
                    && !(nested && self.preceded_by_open_paren(expr.span().start_usize())) =>
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

    /// Whether the previous non-trivia byte before `start` (a full-file offset)
    /// is `(` — i.e. the expression starting at `start` sits directly inside
    /// grouping parens. Walks back over whitespace; a comment ending where the
    /// walk stops is hopped via the lexer-recorded spans in `self.comments`
    /// (byte-scanning backwards can't delimit comments reliably — same technique
    /// as `paren_preceded_by_jsdoc_cast_comment`). Used by the nested
    /// parenthesized-cast rejection in `to_assignable_impl`, so it only runs on
    /// the rare cast-in-pattern path.
    fn preceded_by_open_paren(&self, start: usize) -> bool {
        let bytes = self.source.as_bytes();
        let mut i = start - self.base_offset;
        loop {
            while i > 0 && bytes[i - 1].is_ascii_whitespace() {
                i -= 1;
            }
            // If the walk stopped inside (or at the end of) a recorded comment,
            // hop to the comment's start and keep walking.
            let pos = (i + self.base_offset) as u32;
            match self
                .comments
                .iter()
                .rev()
                .find(|c| c.span.start < pos && pos <= c.span.end)
            {
                Some(c) => i = c.span.start as usize - self.base_offset,
                None => break,
            }
        }
        i > 0 && bytes[i - 1] == b'('
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
        let argument = self.to_assignable_impl(spread.argument.clone(), context, true)?;
        if matches!(argument, Expression::AssignmentPattern(_)) {
            return Err(self.error_msg_at(
                "A rest element cannot have a default value",
                spread.span.start_usize(),
            ));
        }
        Ok(RestElement {
            argument: self.alloc(argument),
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
                let value = self.to_assignable_impl(p.value.clone(), context, true)?;

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
                self.spread_to_rest_element(&spread, context)?,
            )),
        }
    }
}
