// Function/method parameter and destructuring-pattern parsing.

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use crate::parser::expression_assignable::AssignableContext;
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

        // Check for optional marker: param? — a bare `?` extends the span.
        let (optional, param_end) = self.eat_optional_marker(param_end);

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
        let extra = type_annotation.map(|ta| self.typed_extra(ta));

        let mut param = Expression::Identifier(Identifier {
            name: symbol,
            optional,
            extra,
            span: Span::new(param_start as u32, id_end as u32),
        });

        // Check for default value: param = default
        if self.eat(TokenKind::Equals) {
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

    /// Parse a bare array/object binding pattern (no trailing `: Type`).
    ///
    /// Current token must be `[` or `{`. The type annotation is left for the
    /// caller: a destructured *parameter* / *declarator* binds the type on the
    /// pattern (`parse_destructured_binding`), whereas a rest element binds it on
    /// the enclosing `RestElement` (`...[a, b]: T`), matching acorn.
    pub(super) fn parse_binding_pattern(&mut self) -> Result<Expression<'arena>, ParseError> {
        let expr = if self.check(&TokenKind::BracketOpen) {
            self.parse_array_expression()?
        } else {
            self.parse_object_expression()?
        };
        // Binding context: a type assertion is not a valid binding target
        // (`let [x as T] = …` / `function f([x as T])` reject, matching acorn).
        self.to_assignable(expr, AssignableContext::Binding)
    }

    /// Parse a destructuring binding (`[a, b]` / `{a, b}`) with an optional
    /// type annotation attached to the resulting pattern.
    ///
    /// Current token must be `[` or `{`. Shared by parameter lists and
    /// variable declarators; default values are handled by the caller.
    pub(super) fn parse_destructured_binding(&mut self) -> Result<Expression<'arena>, ParseError> {
        let mut pattern = self.parse_binding_pattern()?;

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

    /// Consume the contextual keyword `kw` as a parameter-property modifier iff
    /// the current token is the identifier `kw` and the next token begins a
    /// parameter binding — otherwise `kw` is itself the parameter name
    /// (`constructor(readonly)`, `f(override: T)`, `constructor(public readonly)`).
    /// The parameter-list analog of `class.rs`'s `eat_modifier_keyword` (which
    /// keys on a class-member-name lookahead). Returns whether it was consumed.
    fn eat_param_modifier_keyword(&mut self, kw: &str) -> Result<bool, ParseError> {
        if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == kw
            && self.peek_starts_parameter_binding()
        {
            self.advance()?; // consume the modifier keyword
            Ok(true)
        } else {
            Ok(false)
        }
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

                        // Check for parameter property modifiers: public, private, protected, readonly.
                        // Accessibility keywords are strict-mode reserved words, so they cannot be
                        // ordinary parameter names (acorn rejects bare `public`/`private`/`protected`
                        // here too) — eating them greedily is correct. `override` and `readonly`,
                        // by contrast, are contextual keywords that ARE valid parameter names, so
                        // each is treated as a modifier only when a binding follows it (see below).
                        let accessibility = if self.eat_contextual_keyword("public") {
                            Some(Accessibility::Public)
                        } else if self.eat_contextual_keyword("private") {
                            Some(Accessibility::Private)
                        } else if self.eat_contextual_keyword("protected") {
                            Some(Accessibility::Protected)
                        } else {
                            None
                        };

                        // Check for `override` then `readonly` modifiers (TS order:
                        // accessibility → override → readonly). Both are contextual
                        // keywords and valid parameter names, so each is a modifier
                        // only when a binding follows it (see `eat_param_modifier_keyword`).
                        let is_override = self.eat_param_modifier_keyword("override")?;
                        let readonly = self.eat_param_modifier_keyword("readonly")?;

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
                    // `await` as a parameter name — valid only at Script goal in a
                    // `[~Await]` context (`parse_simple_param` interns it via
                    // `try_intern_param_name`).
                    TokenKind::Keyword(KeywordKind::Await) if self.await_is_identifier() => {
                        self.parse_simple_param()?
                    }
                    TokenKind::BracketOpen | TokenKind::BraceOpen => {
                        // Destructuring pattern: [a, b] / {a, b}, with optional
                        // type annotation and default value
                        let pattern = self.parse_destructured_binding()?;

                        // Check for default value
                        if self.eat(TokenKind::Equals) {
                            let pattern_start = pattern.span().start;
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
                        // Rest parameter: ...args, ...args: type, or a
                        // destructuring pattern (...[a, b] / ...{ a }). Per
                        // `BindingRestElement : ... BindingPattern`, the rest
                        // argument may itself be an array/object pattern.
                        let rest_start = self.current_pos().0;
                        self.advance()?; // consume '...'

                        if matches!(
                            self.current_kind(),
                            TokenKind::BracketOpen | TokenKind::BraceOpen
                        ) {
                            // ...[a, b] / ...{ a } — the rest argument is itself a
                            // binding pattern. A trailing `: Type` annotates the
                            // *rest element* (`...[a, b]: T`), not the inner pattern,
                            // so parse the bare pattern and bind the type here
                            // (matching acorn; `parse_destructured_binding` would
                            // wrongly attach it to the pattern).
                            let pattern = self.parse_binding_pattern()?;
                            let (type_annotation, arg_end) = if self.check(&TokenKind::Colon) {
                                let ta = self.parse_type_annotation()?;
                                let end = ta.span.end;
                                (Some(ta), end)
                            } else {
                                (None, pattern.span().end)
                            };
                            Expression::RestElement(RestElement {
                                argument: self.alloc(pattern),
                                type_annotation,
                                span: Span::new(rest_start as u32, arg_end),
                            })
                        } else {
                            // ...args or ...args: type
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

                            let argument = Expression::Identifier(Identifier::simple(
                                symbol,
                                Span::new(id_start as u32, id_end as u32),
                            ));

                            Expression::RestElement(RestElement {
                                argument: self.alloc(argument),
                                type_annotation,
                                span: Span::new(rest_start as u32, arg_end as u32),
                            })
                        }
                    }
                    _ => {
                        return Err(
                            self.error_expected_found("parameter name or destructuring pattern")
                        );
                    }
                };

                let is_rest = matches!(&param, Expression::RestElement(_));
                params.push(param);

                // A rest parameter must be the last in the list. Per the grammar
                // (`FormalParameters : FormalParameterList `,` FunctionRestParameter`)
                // nothing — not even a trailing comma — may follow it. acorn:
                // "Comma is not permitted after the rest element".
                if is_rest {
                    if self.check(&TokenKind::Comma) {
                        return Err(
                            self.error_msg("A rest parameter must be last in a parameter list")
                        );
                    }
                    break;
                }

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
