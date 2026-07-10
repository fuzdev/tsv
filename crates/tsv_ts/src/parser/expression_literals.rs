// Object and array literal parsing (`{ ... }` and `[ ... ]`), plus the method
// body shared by object-literal method shorthand and getters/setters.

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::Parser;

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Parse object literal: `{ prop: value, ... }`
    ///
    /// Supports all JS/TypeScript object literal features:
    /// - Simple properties: `{ prop: value }`
    /// - Shorthand properties: `{ prop }` (key equals value)
    /// - Computed property names: `{ [expr]: value }`
    /// - Method shorthand: `{ foo() {} }`, `{ async foo() {} }`
    /// - Getter/setter: `{ get foo() {}, set foo(v) {} }`
    /// - Spread properties: `{ ...obj }`
    /// - String/number literal keys: `{ "key": value, 123: value }`
    /// - Trailing commas: `{ a: 1, }`
    /// - Empty objects: `{}`
    pub(super) fn parse_object_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let arena = self.arena;
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BraceOpen)?; // consume '{'
        self.grouping_depth += 1;

        let mut properties = self.bvec();
        // Set when a trailing comma follows a final spread property (`{...a,}`):
        // legal in a literal, rejected by `to_assignable` in pattern context.
        let mut spread_trailing_comma = false;

        // Handle empty object: `{}`
        if self.check(&TokenKind::BraceClose) {
            let (_, end) = self.current_pos();
            self.advance()?; // consume '}'
            self.grouping_depth -= 1;
            return Ok(Expression::ObjectExpression(ObjectExpression {
                properties: properties.into_bump_slice(),
                spread_trailing_comma: false,
                span: Span::new(start as u32, end as u32),
            }));
        }

        // Parse properties
        loop {
            let prop_start = self.current_pos().0;

            // Check for spread: { ...obj }
            if self.eat(TokenKind::DotDotDot) {
                // Use assignment_expression because comma separates properties
                let argument = self.parse_assignment_expression_ref()?;
                // Use prev_token_end() to include the closing paren when the argument
                // is parenthesized (`{...(a && b)}`), matching the array-spread and
                // object-value paths (acorn includes the `)` in the SpreadElement span).
                let prop_end = self.prev_token_end();
                properties.push(ObjectProperty::SpreadElement(SpreadElement {
                    argument,
                    span: Span::new(prop_start as u32, prop_end as u32),
                }));

                // Check for comma or closing brace. A trailing comma right
                // after this spread (`{...a,}`) is the rest-trailing-comma case:
                // the separator is consumed before the `}`, so record it for
                // `to_assignable` (the discarded comma leaves no other trace).
                let trailing_comma = self.check(&TokenKind::Comma);
                if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::BraceClose)? {
                    spread_trailing_comma = trailing_comma;
                    break;
                }
                continue;
            }

            // Check for async method: `async foo() {}` or `async *gen() {}`
            // async is tokenized as a keyword, and is treated as method when followed by a
            // property name or `*` on the same line. A line break makes `async` an ordinary
            // shorthand property (ECMAScript `async [no LineTerminator here] MethodDefinition`);
            // `{ async⏎m() {} }` is then a syntax error (a stray `m` with no separator), matching
            // acorn/prettier — not a silently-accepted async method.
            let is_async_method =
                if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Async))
                    && (self.peek_is_property_name() || self.peek_is(&TokenKind::Star))
                    && !self.peek_preceded_by_line_terminator()
                {
                    self.advance()?; // consume 'async'
                    true
                } else {
                    false
                };

            // Check for generator method: `*gen() {}` or `async *gen() {}`
            let is_generator = self.eat(TokenKind::Star);

            // Check for getter/setter: `get x() {}` or `set x(v) {}`
            // These are contextual keywords - only treated as get/set when followed by a property name
            // Note: async getters/setters and generator getters/setters are not valid in JS
            let accessor_kind = if !is_async_method
                && !is_generator
                && *self.current_kind() == TokenKind::Identifier
            {
                let is_get = self.current_value() == "get";
                let is_set = self.current_value() == "set";
                if (is_get || is_set) && self.peek_is_property_name() {
                    let kind = if is_get {
                        PropertyKind::Get
                    } else {
                        PropertyKind::Set
                    };
                    self.advance()?; // consume 'get' or 'set'
                    Some(kind)
                } else {
                    None
                }
            } else {
                None
            };

            // Parse property key
            // Supports: identifiers, keywords (as identifiers), string literals, number literals, computed keys
            // Track if key is a restricted keyword (can't be used in shorthand)
            let (key, computed, is_restricted_keyword) = match self.current_kind() {
                // Computed property: { [expr]: value }
                TokenKind::BracketOpen => {
                    self.advance()?; // consume '['
                    (self.parse_computed_member_key()?, true, false)
                }
                // Both identifiers and keywords can be property keys: { foo: 1, object: 2, in: 3 }
                TokenKind::Identifier => {
                    let (key_start, key_end) = self.current_pos();
                    let name = self.current_ident_name();
                    self.advance()?;
                    (
                        Expression::Identifier(Identifier::simple(
                            name,
                            Span::new(key_start as u32, key_end as u32),
                        )),
                        false,
                        false,
                    )
                }
                TokenKind::Keyword(kw) => {
                    let (key_start, key_end) = self.current_pos();
                    let name = self.current_raw_ident_name();
                    // Track if this keyword cannot be used as identifier reference
                    // in shorthand. `await` is allowed at Script `[~Await]` (a
                    // valid `IdentifierReference`), like any other identifier.
                    let restricted = match kw {
                        KeywordKind::Await => !self.await_is_identifier(),
                        KeywordKind::Yield | KeywordKind::Let => true,
                        _ => false,
                    };
                    self.advance()?;
                    (
                        Expression::Identifier(Identifier::simple(
                            name,
                            Span::new(key_start as u32, key_end as u32),
                        )),
                        false,
                        restricted,
                    )
                }
                TokenKind::String => {
                    // String literal key: {"prop-name": value}
                    let (key_start, key_end) = self.current_pos();
                    let cooked = self.extract_string_cooked();
                    self.advance()?;
                    (
                        Expression::Literal(Literal {
                            value: LiteralValue::String(cooked),
                            span: Span::new(key_start as u32, key_end as u32),
                        }),
                        false,
                        false,
                    )
                }
                TokenKind::Number => {
                    // Number literal key: {0: value, 0xb_b: value, 1n: value} —
                    // shares the full numeric decode (radix, separators, bigint)
                    let literal = self.parse_number_or_bigint_literal()?;
                    self.advance()?;
                    (Expression::Literal(literal), false, false)
                }
                _ => {
                    return Err(self.error_expected_found_at("property key", prop_start));
                }
            };

            // Determine property kind, value, shorthand, and method flags
            let (kind, value, shorthand, method) = if let Some(accessor) = accessor_kind {
                // Getter/setter: `get x() {}` or `set x(v) {}`
                // Note: getters/setters cannot be async
                let func_expr = self.parse_method_body(false, false)?;
                // Enforce accessor arity (getter: no params; setter: exactly one
                // non-rest param) — acorn rejects a violation at parse, tsv matches
                // (see `check_accessor_param_arity`). An object-literal accessor
                // excludes a leading `this` param from the count (`allow_this_param`).
                self.check_accessor_param_arity(
                    accessor == PropertyKind::Get,
                    func_expr.params,
                    true,
                )?;
                (
                    accessor,
                    Expression::FunctionExpression(func_expr),
                    false,
                    false,
                )
            } else if self.check(&TokenKind::ParenOpen)
                || self.check(&TokenKind::LessThan)
                || is_async_method
                || is_generator
            {
                // Method shorthand: `{ foo() {} }`, `{ foo<T>() {} }`, `{ async foo() {} }`, or `{ *gen() {} }`
                let func_expr = self.parse_method_body(is_async_method, is_generator)?;
                (
                    PropertyKind::Init,
                    Expression::FunctionExpression(func_expr),
                    false,
                    true,
                )
            } else if self.eat(TokenKind::Colon) {
                // Use assignment_expression because comma separates properties
                (
                    PropertyKind::Init,
                    self.parse_assignment_expression()?,
                    false,
                    false,
                )
            } else if self.check(&TokenKind::Equals) && !computed {
                // Shorthand with default value: `{a = 1}` (only for simple identifiers)
                // This parses as an AssignmentExpression, which gets converted to
                // AssignmentPattern by to_assignable() when used in destructuring context
                // Restricted keywords (await, yield, let) can't be used as shorthand identifiers
                if is_restricted_keyword {
                    return Err(self.error_msg_at(
                        "Cannot use restricted keyword as shorthand property",
                        key.span().start_usize(),
                    ));
                }
                self.advance()?; // consume '='
                let default_value = self.parse_assignment_expression()?;
                // prev_token_end covers a parenthesized default's closing `)`
                let assign_end = self.prev_token_end() as u32;
                (
                    PropertyKind::Init,
                    Expression::AssignmentExpression(AssignmentExpression {
                        left: arena.alloc(key.clone()),
                        operator: AssignmentOperator::Assign,
                        right: arena.alloc(default_value),
                        span: Span::new(key.span().start, assign_end),
                    }),
                    true,
                    false,
                )
            } else {
                // Shorthand: key is duplicated as value
                // Restricted keywords (await, yield, let) can't be used as shorthand identifiers
                if is_restricted_keyword {
                    return Err(self.error_msg_at(
                        "Cannot use restricted keyword as shorthand property",
                        key.span().start_usize(),
                    ));
                }
                (PropertyKind::Init, key.clone(), true, false)
            };

            // Use prev_token_end() to include closing paren when value is parenthesized
            let prop_end = self.prev_token_end();
            properties.push(ObjectProperty::Property(Property {
                key,
                value,
                kind,
                shorthand,
                computed,
                method,
                span: Span::new(prop_start as u32, prop_end as u32),
            }));

            // Check for comma or closing brace (with trailing comma support)
            if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::BraceClose)? {
                break;
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?; // consume '}'
        self.grouping_depth -= 1;

        Ok(Expression::ObjectExpression(ObjectExpression {
            properties: properties.into_bump_slice(),
            spread_trailing_comma,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse array literal: `[elem, ...]`
    ///
    /// Supports all JS/TypeScript array literal features:
    /// - All expression types as elements
    /// - Spread elements: `[...arr]`
    /// - Elision (holes/sparse arrays): `[, a]`, `[1,,3]`, `[, , a]`
    /// - Trailing commas: `[1, 2, 3,]`
    /// - Empty arrays: `[]`
    pub(super) fn parse_array_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BracketOpen)?; // consume '['
        self.grouping_depth += 1;

        let mut elements = self.bvec();
        // Set when a trailing comma follows a final spread element (`[...a,]`):
        // legal in a literal, rejected by `to_assignable` in pattern context.
        let mut spread_trailing_comma = false;

        // Handle empty array: `[]`
        if self.check(&TokenKind::BracketClose) {
            let (_, end) = self.current_pos();
            self.advance()?; // consume ']'
            self.grouping_depth -= 1;
            return Ok(Expression::ArrayExpression(ArrayExpression {
                elements: elements.into_bump_slice(),
                spread_trailing_comma: false,
                span: Span::new(start as u32, end as u32),
            }));
        }

        // Parse elements (including elision/holes)
        loop {
            // Check for elision (hole): leading comma means empty slot
            if self.eat(TokenKind::Comma) {
                elements.push(None); // hole
                // Check if we hit the closing bracket (trailing comma after hole)
                if self.check(&TokenKind::BracketClose) {
                    break;
                }
                continue;
            }

            // Check for closing bracket (end of array)
            if self.check(&TokenKind::BracketClose) {
                break;
            }

            // Parse element expression (use assignment_expression because comma separates elements)
            let elem = self.parse_assignment_expression()?;
            elements.push(Some(elem));

            // Check for comma or closing bracket
            if self.eat(TokenKind::Comma) {
                // Check for trailing comma
                if self.check(&TokenKind::BracketClose) {
                    // A trailing comma after a final spread (`[...a,]`) is the
                    // rest-trailing-comma case; record it for `to_assignable`
                    // (the discarded comma leaves no other trace).
                    if matches!(elements.last(), Some(Some(Expression::SpreadElement(_)))) {
                        spread_trailing_comma = true;
                    }
                    break;
                }
            } else if self.check(&TokenKind::BracketClose) {
                break;
            } else {
                return Err(ParseError::InvalidExpression {
                    found: format!("'{}'", self.current_kind()),
                    position: self.current_pos().0,
                    context: None,
                });
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BracketClose)?; // consume ']'
        self.grouping_depth -= 1;

        Ok(Expression::ArrayExpression(ArrayExpression {
            elements: elements.into_bump_slice(),
            spread_trailing_comma,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse method body for method shorthand: `foo() { return 1; }`
    ///
    /// This parses the parameter list and block body for a method definition.
    /// The key has already been parsed by the caller.
    fn parse_method_body(
        &mut self,
        is_async: bool,
        is_generator: bool,
    ) -> Result<FunctionExpression<'arena>, ParseError> {
        // Parse optional type parameters: <T, U>
        let type_parameters = self.parse_optional_type_parameters()?;

        // Capture paren position before parsing params (for comment detection)
        let (params_start, _) = self.current_pos();
        // Params + body in the method's `[Await]` context (async → `[+Await]`).
        let params = self
            .with_in_await(is_async, Self::parse_parameter_list)?
            .into_bump_slice();

        // Check for return type annotation: (): type or type predicate
        let return_type = self.parse_optional_return_type()?;

        let body = self.with_in_await(is_async, Self::parse_function_body)?;
        let end = body.span.end;

        Ok(FunctionExpression {
            id: None, // Method shorthand has no function name
            type_parameters,
            params,
            return_type,
            body,
            generator: is_generator,
            r#async: is_async,
            params_start: params_start as u32,
            span: Span::new(params_start as u32, end),
        })
    }
}
