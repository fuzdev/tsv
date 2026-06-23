// Cover-grammar conversion: turn an already-parsed `Expression` into an
// assignable pattern (`{a, b} = obj`, `[x] = arr`, arrow params). Pure AST
// rewriting — no token consumption.

use crate::ast::internal::{
    ArrayPattern, AssignmentPattern, Expression, ObjectPattern, ObjectPatternProperty,
    ObjectProperty, Property, RestElement,
};
use tsv_lang::ParseError;

use super::Parser;

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
    /// - BinaryExpression with = (shorthand default) → AssignmentPattern
    /// - Identifier, MemberExpression → unchanged (valid assignment targets)
    pub(super) fn to_assignable(&self, expr: Expression) -> Result<Expression, ParseError> {
        match expr {
            // Identifier is already a valid assignment target
            Expression::Identifier(_) => Ok(expr),

            // Member expression is a valid assignment target
            Expression::MemberExpression(_) => Ok(expr),

            // Convert ObjectExpression to ObjectPattern
            Expression::ObjectExpression(obj) => {
                let properties = obj
                    .properties
                    .into_iter()
                    .map(|prop| self.object_property_to_pattern(prop))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Expression::ObjectPattern(ObjectPattern {
                    properties,
                    type_annotation: None,
                    span: obj.span,
                }))
            }

            // Convert ArrayExpression to ArrayPattern
            Expression::ArrayExpression(arr) => {
                let elements = arr
                    .elements
                    .into_iter()
                    .map(|elem| elem.map(|e| self.to_assignable(e)).transpose())
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Expression::ArrayPattern(ArrayPattern {
                    elements,
                    type_annotation: None,
                    span: arr.span,
                }))
            }

            // Convert SpreadElement to RestElement
            Expression::SpreadElement(spread) => {
                let argument = self.to_assignable(*spread.argument)?;
                Ok(Expression::RestElement(RestElement {
                    argument: Box::new(argument),
                    type_annotation: None,
                    span: spread.span,
                }))
            }

            // AssignmentExpression in pattern context becomes AssignmentPattern
            // This handles default values like `{a = 1}` which was parsed as shorthand
            Expression::AssignmentExpression(assign) => {
                let left = self.to_assignable(*assign.left)?;
                Ok(Expression::AssignmentPattern(AssignmentPattern {
                    left: Box::new(left),
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
        prop: ObjectProperty,
    ) -> Result<ObjectPatternProperty, ParseError> {
        match prop {
            ObjectProperty::Property(p) => {
                // Convert the value to a pattern
                let value = self.to_assignable(p.value)?;

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
                let argument = self.to_assignable(*spread.argument)?;
                Ok(ObjectPatternProperty::RestElement(RestElement {
                    argument: Box::new(argument),
                    type_annotation: None,
                    span: spread.span,
                }))
            }
        }
    }
}
