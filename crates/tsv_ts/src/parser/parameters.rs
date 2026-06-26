// Function/method parameter and destructuring-pattern parsing.

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::Parser;

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Parse a simple parameter: identifier with optional `?`, type annotation, and default value.
    ///
    /// Accepts both regular identifiers and contextual keywords (`from`, `as`, etc.).
    /// Does NOT handle parameter property modifiers (`public`, `private`, `readonly`).
    fn parse_simple_param(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (param_start, param_end) = self.current_pos();
        let symbol = self
            .try_intern_param_name()
            .ok_or_else(|| self.error_expected("parameter name or destructuring pattern"))?;
        self.advance()?;

        // Check for optional marker: param?
        let optional = self.eat(TokenKind::Question);
        // The `?` extends the identifier span when no type annotation follows
        let param_end = if optional {
            self.prev_token_end()
        } else {
            param_end
        };

        // Check for type annotation: param: type
        let (type_annotation, id_end) = if self.check(&TokenKind::Colon) {
            let ta = self.parse_type_annotation()?;
            let end = ta.span.end;
            (Some(ta), end as usize)
        } else {
            (None, param_end)
        };

        // Build binding extra only when a type annotation is present (decorators,
        // if any, are attached later by the parameter-list caller).
        let extra = type_annotation.map(|ta| {
            self.alloc(IdentifierParamExtra {
                type_annotation: Some(ta),
                decorators: None,
            })
        });

        let mut param = Expression::Identifier(Identifier {
            name: symbol,
            optional,
            extra,
            span: Span::new(param_start as u32, id_end as u32),
        });

        // Check for default value: param = default
        if self.check(&TokenKind::Equals) {
            self.advance()?; // consume '='
            let default_value = self.parse_assignment_expression()?;
            // prev_token_end covers a parenthesized default's closing `)`
            let assign_end = self.prev_token_end() as u32;
            param = Expression::AssignmentPattern(AssignmentPattern {
                left: self.alloc(param),
                right: self.alloc(default_value),
                span: Span::new(param_start as u32, assign_end),
            });
        }
        Ok(param)
    }

    /// Attach parameter decorators to the binding identifier of a freshly-parsed
    /// parameter, merging them into the identifier's `extra` (preserving any type
    /// annotation already present). The identifier may be the parameter directly
    /// or the `left` of an `AssignmentPattern` default (`@dec x = 1`).
    fn attach_param_decorators(
        &self,
        mut param: Expression<'arena>,
        decorators: &'arena [Decorator<'arena>],
    ) -> Expression<'arena> {
        let arena = self.arena;
        match &mut param {
            Expression::Identifier(id) => {
                let type_annotation = id.type_annotation().cloned();
                id.extra = Some(arena.alloc(IdentifierParamExtra {
                    type_annotation,
                    decorators: Some(decorators),
                }));
                param
            }
            Expression::AssignmentPattern(ap) => {
                if let Expression::Identifier(id) = ap.left {
                    let mut new_id = id.clone();
                    new_id.extra = Some(arena.alloc(IdentifierParamExtra {
                        type_annotation: id.type_annotation().cloned(),
                        decorators: Some(decorators),
                    }));
                    ap.left = arena.alloc(Expression::Identifier(new_id));
                }
                param
            }
            _ => param,
        }
    }

    /// Parse a destructuring binding (`[a, b]` / `{a, b}`) with an optional
    /// type annotation attached to the resulting pattern.
    ///
    /// Current token must be `[` or `{`. Shared by parameter lists and
    /// variable declarators; default values are handled by the caller.
    pub(super) fn parse_destructured_binding(&mut self) -> Result<Expression<'arena>, ParseError> {
        let expr = if self.check(&TokenKind::BracketOpen) {
            self.parse_array_expression()?
        } else {
            self.parse_object_expression()?
        };
        let mut pattern = self.to_assignable(expr)?;

        // Check for type annotation: [a, b]: Type or {a, b}: Type
        if self.check(&TokenKind::Colon) {
            let type_annotation = self.parse_type_annotation()?;
            let end = type_annotation.span.end;
            match &mut pattern {
                Expression::ArrayPattern(p) => {
                    p.type_annotation = Some(type_annotation);
                    p.span.end = end;
                }
                Expression::ObjectPattern(p) => {
                    p.type_annotation = Some(type_annotation);
                    p.span.end = end;
                }
                _ => {}
            }
        }
        Ok(pattern)
    }

    /// Parse a parenthesized parameter list: `(a, b, c)`
    ///
    /// Used by function declarations, method shorthand, and arrow functions.
    /// Consumes both opening and closing parentheses.
    pub(super) fn parse_parameter_list(
        &mut self,
    ) -> Result<bumpalo::collections::Vec<'arena, Expression<'arena>>, ParseError> {
        self.expect(&TokenKind::ParenOpen)?;

        let mut params = self.bvec();
        if !self.check(&TokenKind::ParenClose) {
            loop {
                // Parse parameter: identifier, array pattern, or object pattern
                let param = match self.current_kind() {
                    // Parameter decorators: @dec1 @dec2 identifier
                    TokenKind::At => {
                        // Parse decorators
                        let mut decorators = self.bvec();
                        while self.check(&TokenKind::At) {
                            let start = self.current_pos().0;
                            self.advance()?; // consume '@'
                            let expression = self.parse_assignment_expression()?;
                            // Covers a parenthesized expression's closing `)` (`@(expr)`)
                            let end = self.prev_token_end();
                            decorators.push(Decorator {
                                expression,
                                span: Span::new(start as u32, end as u32),
                            });
                        }
                        let decorators: &'arena [Decorator<'arena>] = decorators.into_bump_slice();
                        // After decorators, parse parameter and attach decorators to the
                        // identifier (possibly inside an AssignmentPattern default).
                        let param = self.parse_simple_param()?;
                        self.attach_param_decorators(param, decorators)
                    }
                    TokenKind::Identifier => {
                        let param_start = self.current_pos().0;

                        // Check for parameter property modifiers: public, private, protected, readonly
                        // TODO: `readonly` is eaten greedily below, so `constructor(readonly)` /
                        // `constructor(readonly, x)` (readonly as a plain param name) fail where
                        // acorn accepts them — a pre-existing divergence. Fixing needs the same
                        // `peek_starts_parameter_binding` lookahead `override` uses, but acorn's
                        // per-keyword rules differ (it rejects bare `public` here), so it warrants
                        // its own fixture-first pass.
                        let accessibility = if self.eat_contextual_keyword("public") {
                            Some(Accessibility::Public)
                        } else if self.eat_contextual_keyword("private") {
                            Some(Accessibility::Private)
                        } else if self.eat_contextual_keyword("protected") {
                            Some(Accessibility::Protected)
                        } else {
                            None
                        };

                        // Check for `override` modifier (TS order: accessibility →
                        // override → readonly). Unlike accessibility/readonly,
                        // `override` is commonly a real identifier, so only treat it
                        // as a modifier when the next token starts a binding;
                        // otherwise it is itself the parameter name
                        // (`constructor(override)`, `constructor(override: T)`).
                        let is_override = matches!(self.current_kind(), TokenKind::Identifier)
                            && self.current_value() == "override"
                            && self.peek_starts_parameter_binding();
                        if is_override {
                            self.advance()?; // consume 'override'
                        }

                        // Check for readonly modifier (can appear alone or after accessibility)
                        let readonly = self.eat_contextual_keyword("readonly");

                        // If we have modifiers, this is a parameter property
                        if accessibility.is_some() || is_override || readonly {
                            let parameter = self.parse_simple_param()?;
                            let param_end = parameter.span().end;
                            Expression::TSParameterProperty(TSParameterProperty {
                                accessibility,
                                readonly,
                                r#override: is_override,
                                parameter: self.alloc(parameter),
                                span: Span::new(param_start as u32, param_end),
                            })
                        } else {
                            // Simple identifier parameter (no modifiers)
                            self.parse_simple_param()?
                        }
                    }
                    // Keywords that can be used as parameter names (contextual keywords like `from`, `async`)
                    // Note: `await`, `yield`, `let` are NOT allowed as parameter names
                    TokenKind::Keyword(kw) if kw.can_be_binding_name() => {
                        self.parse_simple_param()?
                    }
                    // TypeScript `this` parameter: `function f(this: T) {}`
                    TokenKind::Keyword(KeywordKind::This) => self.parse_simple_param()?,
                    TokenKind::BracketOpen | TokenKind::BraceOpen => {
                        // Destructuring pattern: [a, b] / {a, b}, with optional
                        // type annotation and default value
                        let pattern = self.parse_destructured_binding()?;

                        // Check for default value
                        if self.check(&TokenKind::Equals) {
                            let pattern_start = pattern.span().start;
                            self.advance()?; // consume '='
                            let default_value = self.parse_assignment_expression()?;
                            // prev_token_end covers a parenthesized default's closing `)`
                            let assign_end = self.prev_token_end() as u32;
                            Expression::AssignmentPattern(AssignmentPattern {
                                left: self.alloc(pattern),
                                right: self.alloc(default_value),
                                span: Span::new(pattern_start, assign_end),
                            })
                        } else {
                            pattern
                        }
                    }
                    TokenKind::DotDotDot => {
                        // Rest parameter: ...args or ...args: type
                        let rest_start = self.current_pos().0;
                        self.advance()?; // consume '...'

                        // Parse the identifier
                        let (id_start, id_end) = self.current_pos();
                        let symbol = self.intern_identifier();
                        self.expect(&TokenKind::Identifier)?;

                        // Check for type annotation: ...args: type
                        let (type_annotation, arg_end) = if self.check(&TokenKind::Colon) {
                            let ta = self.parse_type_annotation()?;
                            let end = ta.span.end;
                            (Some(ta), end as usize)
                        } else {
                            (None, id_end)
                        };

                        let argument = Expression::Identifier(Identifier {
                            name: symbol,
                            optional: false,
                            extra: None,
                            span: Span::new(id_start as u32, id_end as u32),
                        });

                        Expression::RestElement(RestElement {
                            argument: self.alloc(argument),
                            type_annotation,
                            span: Span::new(rest_start as u32, arg_end as u32),
                        })
                    }
                    _ => {
                        return Err(
                            self.error_expected_found("parameter name or destructuring pattern")
                        );
                    }
                };

                params.push(param);

                // Check for comma or closing paren
                if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::ParenClose)? {
                    break;
                }
            }
        }

        self.expect(&TokenKind::ParenClose)?;
        Ok(params)
    }
}
