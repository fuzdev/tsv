// Cover-grammar conversion: turn an already-parsed `Expression` into an
// assignable pattern (`{a, b} = obj`, `[x] = arr`, arrow params). Pure AST
// rewriting — no token consumption.

use crate::ast::internal::{
    ArrayPattern, AssignmentPattern, Expression, ObjectPattern, ObjectPatternProperty,
    ObjectProperty, Property, RestElement, SpreadElement,
};
use tsv_lang::ParseError;

use super::Parser;

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
    pub(super) fn to_assignable(
        &self,
        expr: Expression<'arena>,
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
                    properties.push(self.object_property_to_pattern(prop.clone())?);
                }

                Ok(Expression::ObjectPattern(ObjectPattern {
                    properties: properties.into_bump_slice(),
                    type_annotation: None,
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
                            Some(self.to_assignable(e.clone())?)
                        }
                        None => None,
                    };
                    elements.push(converted);
                }

                Ok(Expression::ArrayPattern(ArrayPattern {
                    elements: elements.into_bump_slice(),
                    type_annotation: None,
                    span: arr.span,
                }))
            }

            // Convert SpreadElement to RestElement
            Expression::SpreadElement(spread) => Ok(Expression::RestElement(
                self.spread_to_rest_element(&spread)?,
            )),

            // AssignmentExpression in pattern context becomes AssignmentPattern
            // This handles default values like `{a = 1}` which was parsed as shorthand
            Expression::AssignmentExpression(assign) => {
                let left = self.to_assignable(assign.left.clone())?;
                Ok(Expression::AssignmentPattern(AssignmentPattern {
                    left: self.alloc(left),
                    right: assign.right,
                    span: assign.span,
                }))
            }

            // Already a pattern (can happen with nested patterns)
            Expression::ObjectPattern(_)
            | Expression::ArrayPattern(_)
            | Expression::AssignmentPattern(_)
            | Expression::RestElement(_) => Ok(expr),

            // Invalid assignment target
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
    ) -> Result<RestElement<'arena>, ParseError> {
        let argument = self.to_assignable(spread.argument.clone())?;
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
    ) -> Result<ObjectPatternProperty<'arena>, ParseError> {
        match prop {
            ObjectProperty::Property(p) => {
                // Convert the value to a pattern
                let value = self.to_assignable(p.value.clone())?;

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
                self.spread_to_rest_element(&spread)?,
            )),
        }
    }
}
