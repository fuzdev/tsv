// Cover-grammar conversion: turn an already-parsed `Expression` into an
// assignable pattern (`{a, b} = obj`, `[x] = arr`, arrow params). Pure AST
// rewriting — no token consumption.

use crate::ast::internal::{
    ArrayPattern, AssignmentPattern, Expression, ObjectPattern, ObjectPatternProperty,
    ObjectProperty, Property, RestElement,
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
                let mut properties = self.bvec();
                for prop in obj.properties {
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
                let mut elements = self.bvec();
                for elem in arr.elements {
                    elements.push(match elem {
                        Some(e) => Some(self.to_assignable(e.clone())?),
                        None => None,
                    });
                }

                Ok(Expression::ArrayPattern(ArrayPattern {
                    elements: elements.into_bump_slice(),
                    type_annotation: None,
                    span: arr.span,
                }))
            }

            // Convert SpreadElement to RestElement
            Expression::SpreadElement(spread) => {
                let argument = self.to_assignable(spread.argument.clone())?;
                Ok(Expression::RestElement(RestElement {
                    argument: self.alloc(argument),
                    type_annotation: None,
                    span: spread.span,
                }))
            }

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
            ObjectProperty::SpreadElement(spread) => {
                // Convert spread to rest element
                let argument = self.to_assignable(spread.argument.clone())?;
                Ok(ObjectPatternProperty::RestElement(RestElement {
                    argument: self.alloc(argument),
                    type_annotation: None,
                    span: spread.span,
                }))
            }
        }
    }
}
